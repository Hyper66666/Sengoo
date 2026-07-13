## 1. Prerequisites and baseline

- [x] 1.1 Run `openspec validate native-debug-info --strict`.
- [x] 1.2 Confirm `codegen-ir-correctness-and-gate` is archived (conformance
  gate drives the real `sgc` CLI); otherwise record it as the active blocker
  and stop before §3.
- [x] 1.3 Record the no-`-g` baseline: pick three fixtures (scalar control
  flow, struct/method, async main) and check in their emitted IR hashes to
  prove later byte-identity without `-g`.
  - `compiler/tests/fixtures/debug-info-baselines/` contains source, LLVM IR,
    and FNV64 records for all three shapes. The integration test pins one
    reference triple so byte identity is reproducible across build hosts.
- [x] 1.4 Pin explicit deferrals: no `sgpm build` debug-profile forwarding,
  no DAP/IDE debug UI, no pretty-printers, and no full local-variable
  inspection in v1.

## 2. Span plumbing audit

- [x] 2.1 Audit HIR→MIR lowering for statements whose spans are dropped;
  thread or inherit spans so every MIR instruction maps to a source line.
  - AST statement byte offsets now survive as zero-codegen HIR source markers,
    MIR records them beside instructions and terminators, and synthetic MIR
    without a direct site inherits the active statement location in codegen.
    Loop/match joins, Drop exit rewrites, postcondition CFG rewrites, and async
    poll synthesis preserve the originating site; coverage registration
    prologue instructions are explicitly hidden from user stepping.
- [x] 2.2 Add a compiler unit test asserting representative MIR functions
  have line coverage for calls, branches, returns, and assignments.
  - `debug_span_tests::mir_preserves_statement_sites_for_debuggable_operations`
    checks call, Store assignment, If, and two explicit Return paths against
    their exact source lines. Companion tests lock loop back-edges, Drop and
    contract exit rewrites, async poll transformation, and debug+coverage
    behavior.

## 3. DI emission

- [x] 3.1 Emit `DIFile`/`DICompileUnit` named metadata and module flags
  (`Debug Info Version`, DWARF version) under `-g`.
- [x] 3.2 Emit `DISubprogram` per function with `!dbg` attachment, including
  synthesized lambda names.
- [x] 3.3 Attach statement `!dbg` locations per design D-A2.
  - `debug_span_tests::llvm_debug_locations_follow_mir_statement_sites`
    proves that call/store/branch/return LLVM instructions use the metadata id
    for their own MIR source site. The older source scanner is retained only
    for function entry and local-declaration naming, not statement assignment;
    async poll IR retains each pre-transform statement line as well.
- [x] 3.4 IR tests: DI presence/shape under `-g`; byte-identical IR without
  `-g` against §1.3 baselines.

  - `tools/sgc/tests/debug_info_baselines.rs` proves exact non-debug IR and
    hash identity before and after a debug build, while requiring compile-unit,
    file, subprogram, and location metadata only in debug mode.

## 4. CLI and cache

- [x] 4.1 Add `-g`/`--debug-info` to `sgc build` and `sgc run`; forward `-g`
  to `clang` compile/link.
- [x] 4.2 Add the debug-mode dimension to the artifact-cache fingerprint;
  tests prove `-g` and non-`-g` artifacts never alias and cache reuse still
  works within each mode.
- [x] 4.3 Conformance examples run under `-g` with unchanged results through
  the real-CLI gate.
  - `tools/sgc/tests/core_conformance.rs` runs every core case in both default
    and `--debug-info` modes with forced rebuilds, then requires identical exit
    codes and stdout. The complete 3-test integration target passes locally.

## 5. Debugger validation and docs

- [ ] 5.1 Linux: scripted lldb transcript
  `docs/debugging-native-linux-lldb.transcript` — breakpoint on a Sengoo
  file:line binds, hits, `next` steps one source line, `continue` exits 0.
- [x] 5.2 Windows: scripted cdb/WinDbg transcript
  `docs/debugging-native-windows-cdb.transcript` with the same assertions on
  the CodeView path.
  - The Windows reference host emits CodeView into the native object, writes a
    deterministic sibling PDB with `/DEBUG:FULL`, and passes the real CDB
    driver: the source breakpoint binds and hits line 2, `p` reaches line 3,
    `value = 21`, `doubled = 42`, and execution completes normally. The
    normalized transcript preserves the actual debugger commands and values.
- [ ] 5.3 Stretch: parameter `DILocalVariable` + `llvm.dbg.declare`; ship
  only with passing reads in both debuggers, else record matrix-deferred.
- [ ] 5.4 Upgrade `docs/debugging-native.md` to source-level workflows and
  link both transcripts; document `-g` in `docs/language-features.md`.
- [x] 5.5 Add the source-level debugging row to
  `examples/realworld/SUPPORT_MATRIX.md` with proof links.
  - The matrix keeps the capability at `Supported subset`, links the compiler,
    object, cache, and native-driver evidence, and explicitly leaves live
    LLDB/CDB transcripts open.

## 6. Verification

- [x] 6.1 `cargo fmt --check`
- [x] 6.2 `cargo test -p sengoo-compiler --lib`
- [x] 6.3 `cargo test -p sgc`
- [ ] 6.4 Perf gate re-run (umbrella Phase 5 evidence): default-mode
  numbers unchanged vs reference snapshot.
- [x] 6.5 `openspec validate native-debug-info --strict`

## Archive Gate

- [x] `openspec validate native-debug-info --strict` passes.
- [ ] Breakpoint + stepping transcripts exist for both Windows (CodeView)
  and Linux (DWARF) and are linked from the docs.
- [ ] Non-`-g` IR is byte-identical to the pre-change baseline; conformance
  results are unchanged under `-g`.
- [x] Cache never aliases debug and non-debug artifacts.
  - Debug mode participates in run/build cache keys and mismatch tests prove a
    mode change cannot reuse the opposite artifact.
- [ ] The umbrella `mainstream-adoption-gap-closure` records Pillar A
  completion and the matrix row cites this change's proof.
