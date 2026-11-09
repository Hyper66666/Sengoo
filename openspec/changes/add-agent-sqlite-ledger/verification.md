# Verification

Windows x64, DESKTOP-FUAQMFB, 2026-11-09; baseline
`ea803f1913f9d08f6393d412b0eb4b1de3055b3e` plus working changes.
The final tracked driver `packages/sgagent/tests/verify_sqlite.py` passes all
nine recorded build/format/native/cross-process steps. Exact inputs and command
outputs are in `target/agent-sqlite-ledger/final/evidence.json`.

- Compiler `target/agent-parameter-compiler/debug/sgc.exe` SHA256
  `69a42135586f5fa56debe10f35e51e4423a926463e88c3f3bfa5f5e1f596ddce`.
- Formatter SHA256
  `d075a1a0fd51a8e2434650ea542f5f252f526198ff1bf0ba27a7fce3ea7ba814`.
- Test executable SHA256
  `cb6d6f659cd070fd719c2b7f358b48fd5cc9d66c949a6ee55ce1ddbde216061a`.
- Core lib input SHA256
  `396c91ca2d94b4679d40cebda39aa3728b300c6c363b220162bdc85949870cee`.
- Store input SHA256
  `bc0fe99d52c9fe8e692fee2a5d4b8ccf0ea50e3aefd5910bde377b76cff7d317`.

The native suite uses actual SQLite files and actual kernel actions. It passes:
exact current-byte duplicate acknowledgement; different-byte/current, stale and
skipped revision refusal; outer/inner schema/run/fence/ID conflicts; NUL in paths,
run IDs, action types and nested keys; capacity refusal without a partial save;
nondecreasing cumulative counters; close/reopen and new-fence handoff; forced
durable fencing; altered snapshot/digest and noninteger receipt identity refusal;
metadata version refusal; first receipt INSERT failure and later receipt UPDATE
failure after the head write, both with real rollback to the old complete state.

A real claimed-tool checkpoint restores paused without rerunning the tool.
Trusted-host confirmation of quiescence is persisted, then the store closes and
reopens without clearing the unresolved call. Only exact, separately verified
outcome resolution advances that claim; budget counters remain unchanged.
Twenty additional rounds include success, duplicate-open failure, empty load,
invalid action, commit/load and explicit close. After every round, native String,
Buffer, SQLite connections/statements/transactions, FileLocks, bounded JsonDocs
and reservation bytes exactly match baseline.

An independent holder process proves same-run contention and canonical-alias
refusal while a different run remains admissible. Refusal completed in 16 ms
on this host and did not advance the fence. The driver kills and reaps only its
own holder, which launched no tool process. The database's cleanup guard still
refuses normal open after the OS lock is released. Explicit synthetic host proof
then admits recovery, which remains paused with unknown outcome; the checkpoint,
head and receipt digest agree. Its temporary inventory is removed exactly.
The store still does not itself prove arbitrary tool-tree quiescence.

Earlier overall failed evidence is retained at
`target/agent-sqlite-ledger/crossprocess-with-known-buffer-defect.json` and
`resource-diagnostic.log`: the older compiler leaked four hash Buffer handles
per round. The new generic parameter-ownership correction fixes that path;
the store still calls the public hash API with no private FFI or manual field
free workaround. The final gate is a fresh build, not a relabelled old binary.

Dedicated strict validation and current formatting pass. These local synthetic
checks establish storage/locking behavior and scoped resource evidence. They do
not establish arbitrary external tool quiescence, installer VM, public release,
other-platform or the whole Agent ecosystem acceptance. Read-only viewing
without executor admission remains a documented API gap.

Reproduce with explicit paths:

```text
python packages/sgagent/tests/verify_sqlite.py --compiler path/to/sgc.exe --clang path/to/clang.exe --formatter path/to/sgfmt.exe --output target/agent-store/evidence.json
```
