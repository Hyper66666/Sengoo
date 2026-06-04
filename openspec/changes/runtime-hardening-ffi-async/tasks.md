## 1. Baseline And Spec

- [ ] 1.1 Validate this change with `openspec validate runtime-hardening-ffi-async --strict`.
- [ ] 1.2 Inventory existing async runtime, runtime C bridge, FFI, process, filesystem, network, reflection-native, and handle tables.
- [ ] 1.3 Record currently unsupported host/platform features and current failure modes.

## 2. Async Runtime

- [ ] 2.1 Specify stable scheduler progress, cancellation, timeout, task status, and cleanup semantics in docs.
- [ ] 2.2 Add native async tests across `sgc check`, `sgc build`, `sgc run`, and package/test runners where available.
- [ ] 2.3 Convert unsupported async behavior to explicit `STATUS_UNSUPPORTED` or more specific status categories.
- [ ] 2.4 Add leak/cleanup tests for completed, cancelled, timed-out, and failed tasks.

## 3. Dynamic FFI

- [ ] 3.1 Inventory supported host platforms for dynamic library/object loading and symbol lookup.
- [ ] 3.2 Implement supported-platform dynamic FFI or add explicit unsupported status paths.
- [ ] 3.3 Add call-shape, argument type, return type, callback, missing symbol, and unsupported-platform tests.
- [ ] 3.4 Ensure native link/build failures are not used as the unsupported-path signal.

## 4. Platform Behavior

- [ ] 4.1 Document Windows/POSIX differences for path encoding, permissions, symlinks, process termination, signals, stdio capture, and network operations.
- [ ] 4.2 Add tests for portable behavior and documented platform skips.
- [ ] 4.3 Add status mappings for host failures that can be classified portably.

## 5. Panic, Backtrace, And Debug Context

- [ ] 5.1 Add runtime panic messages with source location or best available call context.
- [ ] 5.2 Add backtrace/debug context where supported by the host and build profile.
- [ ] 5.3 Add tests proving stdlib/user-code failures are diagnosable and do not corrupt runtime state.

## 6. Handle Lifecycle And Resource Limits

- [ ] 6.1 Add generation/type/closed-state validation for runtime-owned handles where practical.
- [ ] 6.2 Add tests for invalid handle, wrong handle type, double close, use-after-close, leak detection, and resource exhaustion.
- [ ] 6.3 Add resource limits for config, regex, JSON, compression, FFI, network, command output, and large Buffer/String inputs.

## 7. Security Boundaries

- [ ] 7.1 Add command-execution tests for shell-free argv handling, path traversal, timeout, env clearing, and unsupported signal behavior.
- [ ] 7.2 Add network and config parsing tests for large inputs, unsupported TLS, header/body limits, and invalid handles.
- [ ] 7.3 Add FFI safety tests for invalid signatures and unsupported callbacks.

## 8. Verification

- [ ] 8.1 Run `cargo fmt --check`.
- [ ] 8.2 Run `cargo test -p sgc runtime -- --nocapture`.
- [ ] 8.3 Run `cargo test -p sgc async -- --nocapture`.
- [ ] 8.4 Run `cargo test -p sgc ffi -- --nocapture`.
- [ ] 8.5 Run `cargo test -p sengoo-compiler async -- --nocapture`.
- [ ] 8.6 Run native `sgc check/build/run` scenarios for supported and unsupported runtime paths.

## Done Definition

- [ ] Async behavior is documented and tested for success, failure, cancellation, timeout, and cleanup.
- [ ] Dynamic FFI is supported with tests or explicitly unsupported with status diagnostics on each target platform.
- [ ] Platform differences are documented and covered by tests or accepted skips.
- [ ] Panic/backtrace/debug context is available where supported.
- [ ] Handle lifecycle, resource limits, and security boundaries have negative tests.

## Archive Gate

- [ ] `openspec validate runtime-hardening-ffi-async --strict` passes.
- [ ] `openspec validate --all --strict` passes.
- [ ] All verification commands above pass or have documented, accepted platform skips.
