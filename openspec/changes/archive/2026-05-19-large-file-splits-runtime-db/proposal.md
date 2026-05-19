## Why

`runtime/src/reflect/runtime_db.rs` is 978 lines and mixes six unrelated concerns:
status codes, private storage types, OnceLock state plumbing, FFI pointer
helpers, a hand-written SQL fragment parser, SQL statement execution, and the
`#[no_mangle]` extern C surface that the Sengoo stdlib `db.sg` wrapper imports.

This is the first slice of the roadmap P0 **Large File Splits** track. It was
picked over the absolute largest targets (`runtime/src/net.rs` at 2729 LoC,
`tools/sgc/src/interface.rs` at 2274 LoC) deliberately, because runtime_db.rs
is the smallest member of the family that still has clear logical seams. That
makes it the right place to establish a reusable splitting SOP before tackling
the 2 KLoC+ giants in follow-up changes.

The split also unblocks two concrete near-term needs:

- The Phase 2 ty interning storage sweep (P1-A) will need to touch runtime
  reflection types when migrating Symbol storage; a tidier `runtime_db`
  module makes that audit smaller.
- The P1-B runtime decomposition (net.rs + runtime_ffi.rs) will reuse the
  exact same module shape proven here.

## What Changes

- Convert `runtime/src/reflect/runtime_db.rs` (a single file module) into
  `runtime/src/reflect/runtime_db/` (a directory module with `mod.rs`).
- Extract five focused submodules — `status.rs`, `state.rs`, `ffi_utils.rs`,
  `sql.rs`, `exec.rs` — each scoped to a single concern documented below.
- Preserve every `#[no_mangle] pub extern "C" fn sengoo_db_*` symbol with the
  exact same name, signature, and observable behavior. The `tools/stdlib/db.sg`
  extern block and the `examples_smoke_reflection_db_open_query` sgc assertion
  must keep working without edits.
- Keep all three integration tests (`db_open_ping_close_and_error_mapping`,
  `db_exec_query_with_params_smoke`, `db_invalid_sql_returns_parse_error`) and
  the `TEST_LOCK` static in `mod.rs` so they exercise the assembled FFI surface
  end-to-end.
- Document the resulting SOP in the change tasks so subsequent large-file
  splits (jit, net, interface, lowering, sglsp) can copy it verbatim.
- No **BREAKING** Sengoo language change. No extern C ABI change.

## Capabilities

### New Capabilities

- `large-file-splits`: Defines the project-wide capability for decomposing
  oversized source files into focused submodules while preserving public APIs,
  observable behavior, and the verification baseline.

### Modified Capabilities

- None. `openspec/specs/` currently has only `interned-types`; this change
  adds the second entry.

## Impact

- Affected code:
  - `runtime/src/reflect/runtime_db.rs` → split into 6 files under
    `runtime/src/reflect/runtime_db/`.
  - No other Rust source touches the internals (verified via grep: only
    `runtime/src/reflect.rs:13` carries `mod runtime_db;`).
- Affected non-Rust:
  - `runtime/src/reflect/runtime_db.md` (doc sidecar) gets a brief update
    pointing at the new submodule layout.
- Unchanged:
  - All 15 `sengoo_db_*` extern C symbol names, signatures, and visibility.
  - `tools/stdlib/db.sg` extern declarations.
  - `tools/sgc/src/tests.rs::examples_smoke_reflection_db_open_query`.
  - Public surface of `runtime/src/reflect.rs` (still re-exports the module
    via `mod runtime_db;` exactly as today).
- Verification must keep the standard baseline green after every slice:
  `cargo test -p sengoo-compiler --lib`, `-p sgc`, `-p sengoo-runtime --lib`,
  `-p sgpm`.
