## Decisions

### D1: Integrate the existing smart-completion protocol

M0 checkpoints the current `enhance-sglsp-smart-completion` change. M2 either
lands and archives that child first or imports its requirements/evidence and
marks it superseded. No implemented protocol field, stale-revision protection,
syntax provenance test, or p95 threshold may disappear during reconciliation.

### D2: One workspace snapshot per document revision

Completion, definition, references, rename, signature help, hover, diagnostics,
and code actions consume the same immutable workspace-index snapshot and exact
document revision. Edits are refused when the revision is stale. Parsing occurs
outside publication locks and last-good state is bounded.

### D3: Compiler and formatter own syntax

The LSP's tolerant context classifier may recover incomplete text, but every
import, attribute, pattern, and declaration form used for edits must have a
compiler parser or `sgfmt` fixture. The editor cannot make unsupported syntax
appear valid.

### D4: Rename is conservative and package-aware

Rename covers local variables, parameters, private fields/methods, and
workspace-owned public symbols only when identity is unique in the indexed
package graph. Dependency and stdlib source is read-only. Ambiguous, generated,
macro-expanded, stale, or incomplete identities are rejected without edits.

### D5: Debugging reuses native tools

The VS Code extension launches the installed artifact with the documented CDB
or LLDB adapter/configuration. `native-debug-info` owns metadata fidelity and
transcripts. M2 owns path discovery, launch configuration, source mapping, and
the editor workflow test.

### D6: Performance and compatibility are protocol requirements

- Warm completion p95: <= 80 ms on the checked-in representative workspace.
- Warm hover/signature/definition p95: <= 100 ms.
- A one-file edit must not recursively rescan the workspace.
- Completion data schema v1 and standard LSP fallback remain compatible.
- Formatter pass two must be byte-identical to pass one.

## Failure behavior

Tool crashes, stale edits, malformed protocol payloads, unavailable debugger
adapters, and missing dependency sources produce bounded actionable diagnostics.
They must not silently use another binary found on PATH when an absolute tool
path was configured.
