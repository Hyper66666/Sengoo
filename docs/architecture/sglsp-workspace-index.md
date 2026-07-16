# ADR: snapshot-based sglsp workspace index

## Status

Accepted for the smart-completion foundation.

## Decision

`sglsp` builds workspace roots and resolved direct path-dependency roots once
during initialization. Each source file is represented by an indexed entry
containing its text, origin, symbols, members, signatures, import facts,
documentation, and generation. Standard-library metadata has an independent
revision marker in the same immutable snapshot.

Open documents are exact-version overlays over disk entries. Open, incremental
change, save, close, and watched-file events parse only the affected document
and publish a cloned snapshot after parsing. Publication rejects stale document
versions. Closing an overlay restores its latest disk entry. A broken overlay
keeps the bounded last-good semantic facts for that URI while retaining the
new text and version for diagnostics and recovery.

Requests query indexed symbols, scopes/containers, signatures, documentation,
import facts, and resolved standard-library metadata directly; they never
reconstruct a text map and reparse it. Instrumentation counts recursive scans,
disk reads, parsed documents, core queries, and published snapshots. Regression
tests run 100 warm completion/navigation/hover/signature query cycles and
require scan/read/parse counts to remain unchanged.

Canonical roots retain the normalized manifest package name. A module identity
combines that package with the source-relative path below `src/`; completion,
imported-symbol classification, selective exports, and resolve auto-import all
consume this identity instead of guessing from a file stem.
Namespace and selective completion resolve top-level exports by this identity.
Member completion qualifies the declaring module and owner type; ambiguous bare
types return no members, while explicit/constructor/field/call return types can
propagate safely through a chain.

Full builds and document parsing run behind a handler-lifetime cancellation
guard. Dropping the async adapter cancels its blocking worker before
publication. A monotonically increasing build generation cancels older builds
and prevents them from swapping a stale snapshot. Read/UTF-8/permission
failures are retained per URI; create/change failures keep last-good data and
only explicit delete events remove it.

Every document publication is a two-phase operation. Overlay operations capture
the target URI's overlay epoch and revision; disk operations capture its disk
epoch. They perform IO/parse outside the write lock, recheck cancellation, then
compare-and-swap only the owned epoch while merging into the newest snapshot.
Different URI updates therefore do not invalidate one another, and save or
watched-file publication cannot make a parsed higher-revision overlay stale.
Delete increments the disk epoch; close increments the overlay epoch and
restores the latest disk snapshot when one remains.

The snapshot retains canonical workspace and direct-dependency roots. Origin
for newly opened or watched files uses the longest containing canonical root,
so files created after initial indexing keep dependency provenance.

## Syntax authority

Completion recovery may tolerate incomplete text, but it does not redefine the
language. Import facts are constrained by compiler-parser and `sgfmt` fixtures.
The attribute catalog cites a stable executable compiler, derive, FFI, or
`sgc test` test ID for every advertised entry. A manifest is bijective with
catalog IDs, and the evidence gate actually executes every owner Cargo test,
rejecting missing, ignored, or failed tests. Completion resolve refuses
metadata unless URI, revision, symbol ID, and origin all still match.

## Consequences

- Initial indexing performs bounded filesystem IO once.
- Normal editor events allocate a new snapshot but do not hold the publication
  lock while parsing.
- Per-file failures are isolated; one last-good semantic entry per URI bounds
  recovery memory.
- The old recursive workspace reader remains test-only as a parity oracle and
  is not reachable from production request paths.
