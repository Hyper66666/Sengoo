## 1. Prerequisites and baseline

- [x] 1.1 Run `openspec validate native-debug-info --strict`.
- [x] 1.2 Confirm `codegen-ir-correctness-and-gate` is archived (conformance
  gate drives the real `sgc` CLI); otherwise record it as the active blocker
  and stop before §3.
- [ ] 1.3 Record the no-`-g` baseline: pick three fixtures (scalar control
  flow, struct/method, async main) and check in their emitted IR hashes to
  prove later byte-identity without `-g`.
- [x] 1.4 Pin explicit deferrals: no `sgpm build` debug-profile forwarding,
  no DAP/IDE debug UI, no pretty-printers, and no full local-variable
  inspection in v1.

## 2. Span plumbing audit

- [ ] 2.1 Audit HIR→MIR lowering for statements whose spans are dropped;
  thread or inherit spans so every MIR instruction maps to a source line.
- [ ] 2.2 Add a compiler unit test asserting representative MIR functions
  have line coverage for calls, branches, returns, and assignments.

## 3. DI emission

- [ ] 3.1 Emit `DIFile`/`DICompileUnit` named metadata and module flags
  (`Debug Info Version`, DWARF version) under `-g`.
- [ ] 3.2 Emit `DISubprogram` per function with `!dbg` attachment, including
  synthesized lambda names.
- [ ] 3.3 Attach statement `!dbg` locations per design D-A2.
- [ ] 3.4 IR tests: DI presence/shape under `-g`; byte-identical IR without
  `-g` against §1.3 baselines.

## 4. CLI and cache

- [ ] 4.1 Add `-g`/`--debug-info` to `sgc build` and `sgc run`; forward `-g`
  to `clang` compile/link.
- [ ] 4.2 Add the debug-mode dimension to the artifact-cache fingerprint;
  tests prove `-g` and non-`-g` artifacts never alias and cache reuse still
  works within each mode.
- [ ] 4.3 Conformance examples run under `-g` with unchanged results through
  the real-CLI gate.

## 5. Debugger validation and docs

- [ ] 5.1 Linux: scripted lldb transcript
  `docs/debugging-native-linux-lldb.transcript` — breakpoint on a Sengoo
  file:line binds, hits, `next` steps one source line, `continue` exits 0.
- [ ] 5.2 Windows: scripted cdb/WinDbg transcript
  `docs/debugging-native-windows-cdb.transcript` with the same assertions on
  the CodeView path.
- [ ] 5.3 Stretch: parameter `DILocalVariable` + `llvm.dbg.declare`; ship
  only with passing reads in both debuggers, else record matrix-deferred.
- [ ] 5.4 Upgrade `docs/debugging-native.md` to source-level workflows and
  link both transcripts; document `-g` in `docs/language-features.md`.
- [ ] 5.5 Add the source-level debugging row to
  `examples/realworld/SUPPORT_MATRIX.md` with proof links.

## 6. Verification

- [ ] 6.1 `cargo fmt --check`
- [ ] 6.2 `cargo test -p sengoo-compiler --lib`
- [ ] 6.3 `cargo test -p sgc`
- [ ] 6.4 Perf gate re-run (umbrella Phase 5 evidence): default-mode
  numbers unchanged vs reference snapshot.
- [ ] 6.5 `openspec validate native-debug-info --strict`

## Archive Gate

- [ ] `openspec validate native-debug-info --strict` passes.
- [ ] Breakpoint + stepping transcripts exist for both Windows (CodeView)
  and Linux (DWARF) and are linked from the docs.
- [ ] Non-`-g` IR is byte-identical to the pre-change baseline; conformance
  results are unchanged under `-g`.
- [ ] Cache never aliases debug and non-debug artifacts.
- [ ] The umbrella `mainstream-adoption-gap-closure` records Pillar A
  completion and the matrix row cites this change's proof.
