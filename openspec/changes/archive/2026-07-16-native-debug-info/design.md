## Context

The native backend emits textual LLVM IR compiled and linked by `clang`.
Debug info must therefore be emitted as textual DI metadata (named metadata
`!llvm.dbg.cu`, `DIFile`/`DICompileUnit`/`DISubprogram` nodes, and `!dbg`
attachments), not through a builder API. `clang` then lowers it to DWARF on
POSIX and CodeView on Windows targets. The umbrella froze the enablement
policy (D1) and v1 surface (D2) in
`openspec/changes/mainstream-adoption-gap-closure/design.md`.

## Decisions

### D-A1 Flag and policy

- `sgc build -g` / `sgc run -g` (long form `--debug-info`) emit debug
  metadata at any `-O` level; `-g` plus `-O2` is allowed and documented as
  optimized-code debugging with possibly merged lines.
- Without `-g`, emitted IR is byte-identical to today. No implicit O0
  default in this change.
- `sgc` passes `-g` to the `clang` compile/link invocations when the flag is
  set so the final artifact carries DWARF/CodeView.

### D-A2 Metadata emission strategy

- One `DICompileUnit` per emitted module, `DIFile` per Sengoo source path
  (absolute path + filename split per LLVM convention).
- One `DISubprogram` per emitted function, attached via `!dbg` on the
  function definition, carrying source name, linkage name, file, and line.
- Statement-level `!dbg` locations attached to calls, branches, returns,
  stores from assignments, and loop back-edges; locations come from AST
  spans already carried through HIR/MIR (`Span` line/column); MIR
  instructions missing spans inherit the enclosing statement span rather
  than dropping location coverage.
- Lambdas/closures use their synthesized names (`$__lambdaN`) with the
  lambda expression's source location.
- Version pinning: emit `"Debug Info Version", i32 3` module flag and the
  DWARF version module flag expected by the pinned clang contract from
  `codegen-ir-correctness-and-gate`.

### D-A3 v1 surface and stretch

| Area | Commitment |
| --- | --- |
| Functions, files, lines | Required |
| Breakpoint bind + hit (lldb DWARF, WinDbg/cdb CodeView) | Required |
| `step`/`next` follow Sengoo lines | Required |
| Function parameters as `DILocalVariable` + `llvm.dbg.declare` | Stretch; ships only with tests, otherwise matrix-deferred |
| Locals, struct fields, enum payloads | Out of scope, matrix-deferred |

### D-A4 Cache and reproducibility

- The artifact-cache key gains a debug-mode dimension; `-g` and non-`-g`
  artifacts never alias, covered by cache tests next to the existing
  `runtime.c` fingerprint tests.
- Debug metadata must not change program semantics: conformance examples
  run under `-g` with unchanged results in the gate added by
  `codegen-ir-correctness-and-gate` (which must be archived first so the
  gate drives the real CLI).

### D-A5 Verification approach

- IR-level tests: compile fixtures with `-g` and assert presence/shape of
  `DICompileUnit`, `DISubprogram` for every user function, and `!dbg` on
  representative statements; assert byte-identical IR without `-g`.
- The no-`-g` byte-identity baselines live under
  `compiler/tests/fixtures/debug-info-baselines/` and cover exactly three
  representative entrypoints before implementation starts: scalar control
  flow, struct/method dispatch, and async `main`. Updating those baselines
  without first explaining a non-debug codegen change is out of scope for
  this change.
- Debugger transcripts: scripted lldb batch (Linux CI or manual checklist)
  and cdb script (Windows) setting a source-line breakpoint, running,
  asserting the stop location file:line, stepping one line, continuing to
  exit; transcripts committed as
  `docs/debugging-native-linux-lldb.transcript` and
  `docs/debugging-native-windows-cdb.transcript` and linked from
  `docs/debugging-native.md`.

### D-A6 Explicit deferrals for this change

- `sgpm build`/profile forwarding to `sgc -g` is deferred. This change only
  adds `-g`/`--debug-info` on direct `sgc build` and `sgc run` entrypoints.
- Debug Adapter Protocol support, Sengoo-aware expression evaluation,
  pretty-printers, local variable inspection beyond the optional parameter
  stretch, and IDE launch configuration are deferred.
- If span plumbing cannot locate a source statement, the implementation must
  inherit the nearest enclosing statement location; it must not introduce
  synthetic line `0` locations or omit `!dbg` from required statement kinds.

## Risks / Trade-offs

- Textual DI is verbose and ordering-sensitive; mitigated by emitting only
  under `-g` and asserting non-`-g` byte identity.
- CodeView fidelity through clang may lag DWARF (line merging under
  optimization); v1 accepts line-level parity and records gaps in the
  matrix.
- Span gaps in MIR could yield unlocatable statements; inheriting the
  enclosing statement span keeps stepping monotonic instead of jumping to
  line 0.

## Migration Plan

Additive flag; no migration. Existing build scripts keep current behavior.

## Open Questions

- None for the v1 surface. New debug surfaces require a follow-up OpenSpec
  update before implementation.
