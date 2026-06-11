## 0. Program setup

- [x] 0.1 Add `INVENTORY.md` baseline for all four pillars with current evidence.
- [x] 0.2 Run `openspec validate mainstream-adoption-gap-closure --strict`.
- [x] 0.3 Record cross-pillar dependencies and upstream prerequisites
  (`codegen-ir-correctness-and-gate`, `frontend-1000k-perf-gate`) in
  `INVENTORY.md` and `design.md`.
- [x] 0.4 Snapshot the `SUPPORT_MATRIX.md` rows this program intends to move.
- [x] 0.5 Create the four child changes named in `proposal.md`
  (`native-debug-info`, `async-cancellation-semantics`,
  `http-production-serving`, `toolchain-distribution`), each owning its
  capability deltas and archive gate.
- [x] 0.6 Link every child proposal back to this umbrella and record its
  owner/status in `INVENTORY.md`.
- [x] 0.7 Freeze public API and semantic tables in `design.md` (D1–D5); any
  later public-name or behavior change must update the table before code
  edits.
- [x] 0.8 Record `codegen-ir-correctness-and-gate` as an explicit blocker
  before `native-debug-info` merges codegen edits.

## 1. Pillar A — Source-level debugging (`native-debug-info`)

- [x] 1.1 Child change validated strictly with DI-emission requirements,
  `-g` policy (D1), and v1 surface table (D2).
- [ ] 1.2 DI compile units, subprograms, and `!dbg` statement locations
  emitted under `-g` in the textual IR path.
- [ ] 1.3 Debug artifacts get distinct cache fingerprints; `-g` and non-`-g`
  outputs never alias (`tools/sgc/src/cache.rs` tests).
- [ ] 1.4 Breakpoint + stepping transcripts validated on Windows (CodeView)
  and Linux (DWARF) and committed with `docs/debugging-native.md` upgrade.
- [ ] 1.5 Conformance examples pass under `-g` with unchanged results.

## 2. Pillar B — Cancellation semantics (`async-cancellation-semantics`)

- [x] 2.1 Child change validated strictly with the cooperative cancellation
  contract (D3) pinned before implementation.
- [ ] 2.2 Task cancellation propagation: canceled task halts at next await
  point, reaches `task_status == 3`, runs no post-await user code.
- [ ] 2.3 `select_cancel` for 2..8 homogeneous operands with deterministic
  loser cancel/drop and no dangling reactor interest registrations.
- [ ] 2.4 Cancellation-capable process wait resolving promptly on kill with
  a stable status; Windows + POSIX tests.
- [ ] 2.5 Move the three Deferred matrix rows to supported subsets with
  proof links.

## 3. Pillar C — Production HTTP serving (`http-production-serving`)

- [x] 3.1 Child change validated strictly with feature order and bounds
  pinned (D4).
- [ ] 3.2 Handler-callback routing lands with runtime tests and a realworld
  fixture update.
- [ ] 3.3 Opt-in keep-alive with max-requests and idle-timeout bounds;
  default remains `Connection: close`.
- [ ] 3.4 Streaming response bodies with bounded chunk writes.
- [ ] 3.5 TLS server subset on existing stacks with real-handshake tests on
  at least one host per stack; no plaintext-fallback success.

## 4. Pillar D — Toolchain distribution (`toolchain-distribution`)

- [x] 4.1 Child change validated strictly with artifact layout and channel
  policy pinned (D5).
- [ ] 4.2 Tag-triggered packaging workflow produces checksummed win-x64 and
  linux-x64 archives gated on the smoke matrix.
- [ ] 4.3 `install.ps1` / `install.sh` fetch, verify, and install a pinned
  version; documented fresh-host transcript runs
  `sgc run examples/01_hello.sg`.
- [ ] 4.4 `sgc`, `sgpm`, `sgfmt`, `sglsp` report one coherent version string
  with tests.

## 5. Integration and documentation

- [ ] 5.1 Refresh `examples/realworld/SUPPORT_MATRIX.md` for all moved rows
  and the two new rows (debugging, distribution).
- [ ] 5.2 Update `README.md` / `README.zh-CN.md` with adoption-wave summary
  (debugging, cancellation, serving, install).
- [ ] 5.3 Re-run the perf gate owned by `frontend-1000k-perf-gate` and record
  that `-g` emission did not regress default-mode numbers.
- [ ] 5.4 Each child change points back to this umbrella; each completed
  pillar updates its canonical capability before umbrella archive.

## 6. Verification

- [ ] 6.1 `cargo fmt --check`
- [ ] 6.2 `cargo test -p sengoo-compiler --lib`
- [ ] 6.3 `cargo test -p sengoo-runtime --lib --features native-bridge`
- [ ] 6.4 `cargo test -p sgc`
- [ ] 6.5 `cargo test -p sgpm`
- [ ] 6.6 `cargo test -p sglsp`
- [ ] 6.7 `cargo clippy -p sgc -p sgpm -p sengoo-compiler -p sengoo-runtime
  -p sgfmt -p sglsp --all-targets -- -D warnings`
- [ ] 6.8 `realworld-e2e` job (locked loop, real binaries)
- [ ] 6.9 Debugger transcript checklist (Windows + Linux) linked from
  Pillar A child
- [ ] 6.10 Release packaging workflow dry-run produces installable archives
- [ ] 6.11 `openspec validate mainstream-adoption-gap-closure --strict`
- [ ] 6.12 `openspec validate --all --strict`

## Done Definition

- [ ] All four child changes are strictly validated, implemented, and
  archived into their owned canonical capabilities.
- [ ] A developer can install a versioned toolchain on a fresh Windows or
  Linux host without building from source and debug a Sengoo program at
  source-line level.
- [ ] Async tasks, select losers, and child processes can be canceled with
  documented, tested semantics.
- [ ] An HTTP service with handler routing, keep-alive, streaming, and TLS
  subset runs from a realworld fixture.
- [ ] `SUPPORT_MATRIX.md` reflects the moved and added rows with proof
  links.

## Archive Gate

- [ ] `openspec validate mainstream-adoption-gap-closure --strict` passes.
- [ ] `openspec validate --all --strict` passes.
- [ ] All four required child changes are archived before the umbrella.
- [ ] The Phase 1 prerequisite (`codegen-ir-correctness-and-gate` archived)
  was satisfied before debug-info codegen merges.
- [ ] All verification commands in §6 pass; platform-specific skips document
  evidence and do not omit a pillar implementation.
