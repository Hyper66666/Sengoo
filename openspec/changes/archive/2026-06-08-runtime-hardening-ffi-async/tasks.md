## 1. Baseline And Spec

- [x] 1.1 Validate this change with `openspec validate runtime-hardening-ffi-async --strict`.
- [x] 1.2 Inventory existing async runtime, runtime C bridge, FFI, process, filesystem, network, reflection-native, and handle tables.
- [x] 1.3 Record currently unsupported host/platform features and current failure modes.

## 2. Async Runtime

- [x] 2.1 Specify stable scheduler progress, cancellation, timeout, task status, and cleanup semantics in docs.
- [x] 2.2 Add native async tests across `sgc check`, `sgc build`, `sgc run`, and package/test runners where available.
- [x] 2.3 Convert unsupported async behavior to explicit `STATUS_UNSUPPORTED` or more specific status categories.
- [x] 2.4 Add leak/cleanup tests for completed, cancelled, timed-out, and failed tasks.

## 3. Dynamic FFI

- [x] 3.1 Inventory supported host platforms for dynamic library/object loading and symbol lookup.
- [x] 3.2 Implement supported-platform dynamic FFI or add explicit unsupported status paths.
- [x] 3.3 Add call-shape, argument type, return type, callback, missing symbol, and unsupported-platform tests.
- [x] 3.4 Ensure native link/build failures are not used as the unsupported-path signal.

## 4. Platform Behavior

- [x] 4.1 Document Windows/POSIX differences for path encoding, permissions, symlinks, process termination, signals, stdio capture, and network operations.
- [x] 4.2 Add tests for portable behavior and documented platform skips.
- [x] 4.3 Add status mappings for host failures that can be classified portably.

## 5. Panic, Backtrace, And Debug Context

- [x] 5.1 Add runtime panic messages with source location or best available call context.
- [x] 5.2 Add backtrace/debug context where supported by the host and build profile.
- [x] 5.3 Add tests proving stdlib/user-code failures are diagnosable and do not corrupt runtime state.

## 6. Handle Lifecycle And Resource Limits

- [x] 6.1 Add generation/type/closed-state validation for runtime-owned handles where practical.
- [x] 6.2 Add tests for invalid handle, wrong handle type, double close, use-after-close, leak detection, and resource exhaustion.
- [x] 6.3 Add resource limits for config, regex, JSON, compression, FFI, network, command output, and large Buffer/String inputs.

## 7. Security Boundaries

- [x] 7.1 Add command-execution tests for shell-free argv handling, path traversal, timeout, env clearing, and unsupported signal behavior.
- [x] 7.2 Add network and config parsing tests for large inputs, unsupported TLS, header/body limits, and invalid handles.
- [x] 7.3 Add FFI safety tests for invalid signatures and unsupported callbacks.

## 8. Verification

- [x] 8.1 Run `cargo fmt --check`.
- [x] 8.2 Run `cargo test -p sgc runtime -- --nocapture`.
- [x] 8.3 Run `cargo test -p sgc async -- --nocapture`.
- [x] 8.4 Run `cargo test -p sgc ffi -- --nocapture`.
- [x] 8.5 Run `cargo test -p sengoo-compiler async -- --nocapture`.
- [x] 8.6 Run native `sgc check/build/run` scenarios for supported and unsupported runtime paths.

## Done Definition

- [x] Async behavior is documented and tested for success, failure, cancellation, timeout, and cleanup.
- [x] Dynamic FFI is supported with tests or explicitly unsupported with status diagnostics on each target platform.
- [x] Platform differences are documented and covered by tests or accepted skips.
- [x] Panic/backtrace/debug context is available where supported.
- [x] Handle lifecycle, resource limits, and security boundaries have negative tests.

## Archive Gate

- [x] `openspec validate runtime-hardening-ffi-async --strict` passes.
- [x] `openspec validate --all --strict` passes.
- [x] All verification commands above pass or have documented, accepted platform skips (see `INVENTORY.md` § Verification notes).
