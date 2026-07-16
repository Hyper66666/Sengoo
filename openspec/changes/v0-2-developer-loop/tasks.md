## 1. Integrate current editor work

- [ ] 1.1 Checkpoint and review `enhance-sglsp-smart-completion` implementation,
  OpenSpec, verification transcript, and performance evidence.
- [ ] 1.2 Land/archive it or record an explicit supersession mapping to every M2
  requirement and test.
- [ ] 1.3 Freeze LSP schema/version negotiation and old-client fallback.

## 2. Complete workspace semantics

- [ ] 2.1 Use one snapshot/revision model for completion, hover, definition,
  references, rename, signature help, diagnostics, and code actions.
- [ ] 2.2 Implement conservative package-aware rename with read-only dependency
  and stdlib handling.
- [ ] 2.3 Prove UTF-16 ranges, Unicode identifiers, stale revision rejection,
  incomplete syntax, cancellation, and watched-file invalidation.
- [ ] 2.4 Retain warm p95 and no-recursive-rescan CI thresholds.

## 3. Formatter and test integration

- [ ] 3.1 Add parser/formatter round-trip fixtures for every supported construct
  and all M1 additions.
- [ ] 3.2 Prove `sgfmt` idempotence and newline/comment preservation.
- [ ] 3.3 Navigate `sgc test` structured failures from editor diagnostics to the
  exact file/range without parsing stderr prose.
- [ ] 3.4 Prove package check/test/fmt/doc commands use configured installed tools
  and never silently substitute a PATH binary.

## 4. Native debug integration

- [ ] 4.1 Complete and archive `native-debug-info` with Windows and Linux
  transcript evidence and default-mode perf parity.
- [ ] 4.2 Add VS Code launch configuration generation/discovery for installed
  artifacts and documented CDB/LLDB adapters.
- [ ] 4.3 Test source breakpoint, step, stack, and supported local inspection from
  an editor-launched session; document unsupported adapter/host paths.

## 5. End-to-end evidence

- [ ] 5.1 Add one installed-package protocol fixture covering completion,
  definition, references, rename, signature help, safe code action, formatting,
  test failure navigation, debug launch, and docs.
- [ ] 5.2 Run the Sencoder real-protocol E2E with exact `SGLSP_PATH` if that
  repository is available; retain an in-repo equivalent as the archive gate.
- [ ] 5.3 Run `cargo test -p sglsp`, `cargo test -p sgfmt`, relevant compiler and
  `sgc` tests, warnings-denied Clippy, and installed release smoke.
- [ ] 5.4 Update editor/debug docs and support matrix.
- [ ] 5.5 Run strict OpenSpec validation and archive this change.
