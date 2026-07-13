## 0. Program truth and completed foundations

- [x] 0.1 Archive and validate the P0 ownership, generics/traits, and
  strings/formatting child changes.
- [x] 0.2 Archive the executable language reference and flagship package-loop
  evidence changes.
- [x] 0.3 Record the 2026-07-11 repository/capability snapshot in
  `INVENTORY.md`.
- [x] 0.4 Recompute `INVENTORY.md` from integrated `main` and remove statements
  that were true only of the pre-integration worktree.
- [x] 0.5 Ensure each active capability has exactly one implementation owner;
  archive or mark overlapping umbrellas historical.

## 1. Phase 0 - integrated release baseline

- [ ] 1.1 Complete and archive `mainline-release-baseline`.
- [ ] 1.2 Preserve all current work in reviewable commits/patches, reconcile the
  divergent branch with latest `main`, and resolve conflicts without dropping
  unexplained changes.
- [ ] 1.3 Run the baseline gate: fmt, clippy, workspace tests, runtime native
  bridge tests, realworld package loop, and `openspec validate --all --strict`.
- [x] 1.4 Reconcile active tasks with implementation evidence, especially sgpm
  registry/alias/multiversion, debugger, numeric, and concurrency surfaces.
- [x] 1.5 Update README, support matrix, branch/upstream metadata, and generated
  artifact ignore/cleanup policy from the integrated baseline.

## 2. Phase 1 - mainstream default language and library

- [x] 2.1 Complete and archive `numeric-type-system` under the production versus
  experimental backend policy.
  - Archived as `2026-07-11-numeric-type-system` after the 994-test compiler
    library gate, native numeric/runtime and Cranelift suites, core conformance,
    warning-free compiler/sgc clippy, and strict OpenSpec validation passed.
- [x] 2.2 Complete and archive `generic-collections` using the frozen generic
  storage/drop/callback ABI; scalar helpers become thin compatibility wrappers.
  - Archived as `2026-07-13-generic-collections` after the compiler library and
    complete `sgc` suites, native collection ownership/Drop coverage, the
    default-library locked package loop, warning-free compiler clippy, and
    strict OpenSpec validation passed.
- [ ] 2.3 Complete and archive `debugger-and-test-framework`, including actual
  statement stepping and live scalar/composite inspection on installed
  reference-host debuggers.
- [x] 2.4 Add a default-library conformance package using `Vec<struct>`, a
  string-keyed map with struct values, iterator adapters, checked numeric
  conversion, and automatic Drop with no scalar-only constructors.
  - `examples/realworld/default-library-conformance` uses the ABI-v1 generic
    `vec_new()` / `hashmap_new()` paths, runs `filter -> map -> sum`, checks an
    `i64 -> u8` conversion, and compares owned-String live handles across scope
    exit without manual release calls. Its locked `sgpm check`, `test`,
    `fmt --check`, `doc`, and `build` loop plus native
    `sgc run --force-rebuild` pass on the Windows reference workspace.
- [ ] 2.5 Refresh the language reference and flagship application against the
  Phase 1 public surface.

## 3. Phase 2 - external adoption and release

- [x] 3.1 Reconcile implemented sgpm registry/package-graph behavior with
  `package-registry-and-distribution` before adding more resolver code.
  - The audit confirms the registry server, remote publish, hash-locked cache,
    aliases, multiversion resolution, yank handling, and reference-server e2e
    are implemented. Remaining child tasks are cross-host artifacts and install
    evidence, not more resolver breadth.
- [ ] 3.2 Complete and archive `package-registry-and-distribution`.
- [ ] 3.3 Prove publish -> resolve -> locked build -> test -> run against the
  reference registry, including checksum mismatch, yank, alias, multiversion,
  offline cache, and archive traversal failures.
- [ ] 3.4 Produce signed/checksummed Windows x64, Linux x86_64, macOS x86_64,
  and macOS arm64 release artifacts with install and upgrade smoke outside the
  source checkout.
- [ ] 3.5 Cut a real prerelease tag and verify every tool reports one coherent
  version.

## 4. Phase 3 - safe concurrency and async IO

- [ ] 4.1 Complete and archive `concurrency-safety-and-async-io` with scheduler
  correctness independent of work-stealing implementation.
- [ ] 4.2 Deliver generic `Arc<T>`, `Mutex<T>`, `RwLock<T>`, and `channel<T>` with
  exact Drop and Send/Sync bounds.
- [ ] 4.3 Deliver `task_scope` normal/early-exit join/cancel semantics and stress
  cancellation without leaked tasks.
- [ ] 4.4 Prove timer/socket/owned-handle reactor progress on Linux, Windows, and
  the supported macOS release channel without busy polling.
- [ ] 4.5 Refresh the flagship application with a useful concurrent workload and
  retain a deterministic serial oracle.

## 5. Phase 4 - production hardening and ecosystem

- [ ] 5.1 Complete and archive `production-hardening-v1`.
- [ ] 5.2 Enforce fuzz, sanitizer, leak, long-running concurrency, ABI/versioning,
  and compile/runtime/RSS performance gates.
- [ ] 5.3 Run every realworld fixture and selected official packages with an
  installed released toolchain rather than workspace binaries.
- [ ] 5.4 Publish compatibility and support policies, including editions,
  deprecation windows, runtime ABI versioning, and supported host matrix.
- [ ] 5.5 Grow a reviewed first-party package set for the chosen CLI/Python-hot-
  path/light-service product target; package count alone is not acceptance.

## 6. Post-v1 alternative targets

- [ ] 6.1 Pass the stable-MIR/runtime-ABI entry review for
  `wasm-and-bytecode-backends`.
- [x] 6.2 Split WASM and bytecode into independently archivable
  `wasm-backend-v1` and `bytecode-vm-v1` owner changes before implementation.
- [ ] 6.3 Complete and archive `wasm-backend-v1` with WASM/WASI conformance and
  capability diagnostics.
- [ ] 6.4 Complete and archive `bytecode-vm-v1` with ownership/Drop semantics
  and native differential conformance, or archive a replacement OpenSpec
  decision cancelling the VM with evidence.

## 7. Program closure

- [ ] 7.1 All required child changes pass strict validation and archive in
  dependency order.
- [ ] 7.2 `examples/realworld/SUPPORT_MATRIX.md` contains no unsupported
  `Supported` claim and links
  unit, integration, realworld, and host evidence at the appropriate level.
- [ ] 7.3 A clean clone can install or build the released toolchain, resolve a
  package, test it, debug it, and run the flagship on each supported host.
- [ ] 7.4 Run `openspec validate language-maturity-roadmap --strict` and
  `openspec validate --all --strict`.
