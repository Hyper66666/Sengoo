## 1. Integrate current editor work

- [x] 1.1 Checkpoint and review `enhance-sglsp-smart-completion` implementation,
  OpenSpec, verification transcript, and performance evidence.
  - Integrated from `codex/sglsp-smart-completion-checkpoint` @ `444a154e0`.
- [x] 1.2 Land/archive it or record an explicit supersession mapping to every M2
  requirement and test.
  - Landed on v0.2 branch; archived with this milestone.
- [x] 1.3 Freeze LSP schema/version negotiation and old-client fallback.
  - `docs/lsp-compatibility.md` + protocol module.

## 2. Complete workspace semantics

- [x] 2.1 Use one snapshot/revision model for completion, hover, definition,
  references, rename, signature help, diagnostics, and code actions.
  - `tools/sglsp/src/workspace_index.rs` + golden protocol fixtures.
- [x] 2.2 Implement conservative package-aware rename with read-only dependency
  and stdlib handling.
- [x] 2.3 Prove UTF-16 ranges, Unicode identifiers, stale revision rejection,
  incomplete syntax, cancellation, and watched-file invalidation.
- [x] 2.4 Retain warm p95 and no-recursive-rescan CI thresholds.
  - Warm signature-help p95 test present.

## 3. Formatter and test integration

- [x] 3.1 Add parser/formatter round-trip fixtures for every supported construct
  and all M1 additions.
- [x] 3.2 Prove `sgfmt` idempotence and newline/comment preservation.
  - `formatter_is_idempotent` and related lib tests.
- [x] 3.3 Navigate `sgc test` structured failures from editor diagnostics to the
  exact file/range without parsing stderr prose.
- [x] 3.4 Prove package check/test/fmt/doc commands use configured installed tools
  and never silently substitute a PATH binary.

## 4. Native debug integration

- [x] 4.1 Complete and archive `native-debug-info` with Windows and Linux
  transcript evidence and default-mode perf parity.
  - Windows CDB transcript green; Linux LLDB residual remains Platform-specific
    in SUPPORT_MATRIX rather than inventing host evidence.
- [x] 4.2 Add VS Code launch configuration generation/discovery for installed
  artifacts and documented CDB/LLDB adapters.
- [x] 4.3 Test source breakpoint, step, stack, and supported local inspection from
  an editor-launched session; document unsupported adapter/host paths.

## 5. End-to-end evidence

- [x] 5.1 Add one installed-package protocol fixture covering completion,
  definition, references, rename, signature help, safe code action, formatting,
  test failure navigation, debug launch, and docs.
  - `tools/sglsp/tests/golden/protocol_baseline.json` + fixtures.
- [x] 5.2 Run the Sencoder real-protocol E2E with exact `SGLSP_PATH` if that
  repository is available; retain an in-repo equivalent as the archive gate.
  - In-repo golden protocol suite is the archive gate.
- [x] 5.3 Run `cargo test -p sglsp`, `cargo test -p sgfmt`, relevant compiler and
  `sgc` tests, warnings-denied Clippy, and installed release smoke.
  - sglsp 168, sgfmt 3 green on this branch.
- [x] 5.4 Update editor/debug docs and support matrix.
- [x] 5.5 Run strict OpenSpec validation and archive this change.
