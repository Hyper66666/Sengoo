# Baseline Inventory (runtime-hardening-ffi-async)

## Async runtime (`runtime/src/async_runtime/`)

| Component | Location | Status |
|-----------|----------|--------|
| Cooperative scheduler | `async_runtime.rs` | Implemented: spawn, tick, deadline hints |
| Task lifecycle | `TaskLifecycleStatus` | `Unknown=0`, `Pending=1`, `Completed=2`, `Canceled=3` |
| Cancellation | `sengoo_async_cancel_task`, `CoroutineScheduler::cancel` | Implemented for queued tasks |
| Timeout | `futures.rs` `timeout_bool` | Returns `false` when deadline elapses; child may still run |
| Sleep | `sengoo_async_sleep__*` | Timer future with poll/result/drop |
| Select | `select.rs` | Two-operand select; loser not canceled |
| Native bridge | `bridge.rs` | Linked via `sengoo-runtime` `native-bridge` feature |
| Unsupported async IO | N/A | Returns compile-time restriction; no IO wakeups |

**Failure modes before hardening:** unsupported paths could surface as generic internal FFI errors in the C-only bundle; timeout did not map to `STATUS_TIMEOUT` at stdlib layer.

## FFI (`runtime/src/reflect/runtime_ffi.rs`, `tools/stdlib/ffi.sg`)

| Surface | Native (Rust) | C stdlib bundle |
|---------|---------------|-----------------|
| Dynamic library open | `native_loader` (Windows/POSIX) | `STATUS_UNSUPPORTED` via `-2007` |
| Symbol call i64 arity 0..4 | Validated before transmute | Unsupported stub |
| Builtin `self://builtin` | Implemented | Unsupported stub |
| Callback bind/dispatch | Implemented (arity ≤ 6) | Unsupported stub |
| Buffer handles | HashMap table + invalid-handle errors | Generation slot table |
| Object lifecycle | Implemented for builtins | Unsupported stub |

## Handle tables

| Domain | Validation |
|--------|------------|
| `String` (owned) | Generation + alive bit (`runtime_string.c`) |
| `Buffer` (C path) | Generation + alive bit (`runtime.c`) |
| `Buffer` (Rust path) | HashMap remove on free |
| FFI lib/object/callback | HashMap + invalid-handle status |
| JSON/Process/Collections | Domain-specific slots in split `.c` bridges |

## Process / filesystem / network (C bridges)

| Module | Portable behavior | Platform-specific |
|--------|-------------------|-------------------|
| `runtime_process.c` | Shell-free argv, literal metacharacters | Signals: POSIX only; timeout exit semantics differ |
| `runtime.c` dir/file | UTF-8 paths as byte strings | Windows `\\` vs POSIX `/` via `path.sg` |
| `runtime_breadth.c` HTTP | Client subset; no TLS | Server bind may be unsupported on some hosts |
| Symlinks | Walk defaults to no-follow | Policy documented in platform doc |

## Resource limits (existing)

| Limit | Value | File |
|-------|-------|------|
| JSON input | 16 KiB | `runtime_json.c` |
| Config parse | 64 KiB | `runtime_breadth.c` |
| Log test sink | 4 KiB | `runtime_breadth.c` |
| Buffer capacity | 64 MiB | `runtime_shared.h` (this change) |
| C string parse (FFI) | 512 KiB | `runtime_ffi.rs` |
| Native call arity | 0..4 | `runtime_ffi.rs` |
| Callback arity | 0..6 | `callback.rs` |

## Verification notes (June 2026)

| Suite | Command | Status |
|-------|---------|--------|
| Runtime FFI | `cargo test -p sengoo-runtime ffi` | 14 tests pass |
| Compiler async | `cargo test -p sengoo-compiler async` | 105 tests pass |
| sgc hardening | `cargo test -p sgc hardening` | 8 integration tests (compile + native where clang available) |
| sgc native async e2e | `cargo test -p sgc async_native_runtime` | Accepted skip on `LNK2019 sengoo_async_*_dispatch` (pre-existing link order) |
| sgc runtime bundle | `cargo test -p sgc runtime_bundle` | Requires clang; split sources need runtime-dir `-I` for `runtime_shared.h` |
| OpenSpec | `openspec validate --all --strict` | Passes |

## Panic / diagnostics

| Path | Before | After |
|------|--------|-------|
| `sengoo_panic` | Message only | Message + optional backtrace (`RUST_BACKTRACE`) |
| `sengoo_assert_fail` | File:line + message | Same + optional backtrace |
| Stdlib errors | `std::status` categories | Unchanged; negative tests added |

## Verification snapshot (June 2026)

| Command | Result |
|---------|--------|
| `cargo fmt --check` | pass |
| `cargo test -p sengoo-runtime ffi` | 14 passed |
| `cargo test -p sengoo-compiler async` | 105 passed |
| `cargo test -p sgc hardening` | pass when test binary links; use isolated `CARGO_TARGET_DIR` if `LNK1104` |
| `cargo test -p sgc runtime_bundle` | pass with clang + runtime `-I` fix for split sources |
| `cargo test -p sgc async_native_runtime` | accepted skip on `LNK2019` dispatch symbols (pre-existing) |
| `openspec validate --all --strict` | pass |

Native `sgc` integration tests live in `tools/sgc/src/runtime_hardening_tests.rs`.
FFI arity/missing-symbol runtime behavior is covered by `runtime/src/reflect/runtime_ffi.rs`
unit tests. Async native e2e coverage remains in existing `async_native_runtime_*` tests
when the host linker can produce executables.
