# SQLite ledger design

The kernel's persist action and AgentCommitReceipt use the exact v1 contract in
`add-embeddable-agent-core/design.md`. This change owns only
`packages/sgagent/src/sqlite_store.sg`, its independent test and this specification.

## Ownership and admission
`agent_sqlite_open(&String path, &String run_id, i64 maximum_snapshot_bytes)`
rejects NUL using exact owned bytes before string narrowing. SQLite creates a
new ordinary file if necessary; its canonical path is then resolved before the
per-run lock name is derived as `<canonical-db>.sgagent-<sha256(run_id)>.lock`.
The OS lock is nonblocking. No database fence changes occur until it is held.
The file path is not a URI or memory database. Local cooperative hosts must use
the same physical path; hard-link aliases and hostile filesystem replacement
are outside this initial contract.

The adapter is single-thread owned and !Send/!Sync. FileLock, SqliteDb,
statements and transactions are ordinary owned fields and locals. Borrowed
`close` refuses unresolved cleanup, otherwise closes execution admission;
remaining database ownership is released by automatic Drop at scope exit.
Close and Drop do not kill processes. The host must retain execution authority
until its process tree is quiescent and known results are committed.

## Metadata and transactions
A dedicated metadata table has one exact integer schema_version=1 row. An
unrelated database or newer metadata fails closed. New metadata/tables and the
first run row are created transactionally. Each admitted open increments that
run's positive fence in the same transaction, with checked integer overflow.
A run's current head and last receipt occupy separate tables. SQLite synchronous
FULL is selected; successful SQL COMMIT precedes the returned receipt.

Every read and commit checks the durable current fence, not a cached token alone.
New commits require saved revision+1. The current revision is idempotent only
when the whole serialized checkpoint has exactly the same bytes; stale, skipped,
identity-conflicting and different-byte duplicate actions return status 12.
Both outer action and inner checkpoint schema/run/fence/revision are checked,
as is the stable `<run>:commit:<revision>` action ID. Counters are nonnegative;
requests, tokens and sequence cannot decrease across new commits. No hidden
request cap or budget reset is introduced.

The checkpoint bytes and SHA-256 digest are saved before a separate receipt
upsert inside one transaction. An error, including an injected receipt failure,
rolls the earlier checkpoint write back through the transaction owner's Drop.
Load checks digest, receipt, snapshot identity and schema in the same read
transaction and returns a new owned complete snapshot. Missing snapshot is
status 5. Capacity violations are status 13 and never truncate; the host chooses
1..4 MiB and the JSON node/depth contracts remain explicit. SQL byte length is
checked before materializing snapshot text. NUL in decoded keys/values is rejected
before key lookup or comparisons.

## Unknown outcomes and recovery
A persisted `cleanup_required=true` or phase5 claim blocks default reopening,
even after process exit releases its OS lock. Only
`agent_sqlite_open_after_quiescence(..., confirmed_previous_quiescent=true)`
allows the trusted host to assert prior executor and tool-tree quiescence.
That assertion does not resolve an uncertain side effect or erase its ledger.
The unchanged checkpoint goes to `agent_kernel_resume`, which pauses claimed
unknown tools and interrupted model requests before any dispatch. The new fence
must be persisted through the next revision. Late old-token commits are refused.

Explicit close similarly refuses unresolved cleanup and retains its OS lock.
Drop or a crashed process can still release the lock: the durable flag is the
cross-crash guard, not a claim that destructors prove external process state.
No lock timeout or heartbeat authorizes takeover. A future truly read-only
viewer is separate work; this adapter does not fake one using read/write open.

## Verification
The actual SQLite tests drive the actual Agent kernel and data contracts. They
check exact duplicate/conflicting/stale/skipped revisions; outer/inner identity;
transaction rollback after head write and before receipt; budget preservation;
close/reopen fencing; a forced durable newer fence; digest corruption; metadata
version refusal; bounded snapshots and NUL. Independent processes test lock
contention, forced host exit and guarded unknown-effect recovery. Twenty resource
rounds verify SQLite connections/statements/transactions, OS locks and bounded
JSON counters return to baseline. Synthetic fixtures do not execute recorded
tool side effects. Final evidence records compiler, source and executable hashes.
