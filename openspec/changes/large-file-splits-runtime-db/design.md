# Design: large-file-splits-runtime-db

## Goals

1. Decompose `runtime/src/reflect/runtime_db.rs` (978 LoC) into a directory
   module of focused submodules, each below 350 LoC.
2. Preserve the full extern C surface byte-for-byte: 15 `#[no_mangle]`
   `sengoo_db_*` symbols.
3. Preserve the existing three integration tests and their `TEST_LOCK`
   serialization guarantee.
4. Establish a reusable Split SOP that future large-file changes
   (jit, net, interface, sglsp/main, lowering, sgfmt/main) can apply
   without re-deriving boundary rules each time.

## Non-goals

- No SQL parser improvements, error message changes, or behavior changes.
- No new extern C symbols, even if internal seams suggest them.
- No `serde_json` version bump or dependency churn.
- No reorganization of sibling reflect modules
  (`runtime_ffi.rs`, `runtime_lua54.rs`, `runtime_net_bench.rs`,
  `runtime_proto.rs`) in this change.

## Module Layout

```
runtime/src/reflect/
  runtime_db.md                  (lightly updated: layout pointer)
  runtime_db/
    mod.rs                       (~420 LoC: 15 extern C exports + tests + module wiring)
    status.rs                    (~12  LoC: pub const SENGOO_DB_STATUS_OK / SENGOO_DB_ERR_*)
    state.rs                     (~70  LoC: private types + OnceLock statics + accessors + clear_error / set_error)
    ffi_utils.rs                 (~55  LoC: parse_c_string / parse_optional_json / copy_bytes_to_buffer)
    sql.rs                       (~95  LoC: normalize_identifier / find_keyword_case_insensitive / parse_literal / resolve_param_token / parse_where_clause + value_to_string)
    exec.rs                      (~340 LoC: exec_create_table / resolve_insert_columns / exec_insert / build_select_result / run_select / exec_delete / execute_statement)
```

Sum after split: ~990 LoC (within ±2% of original — the small delta is the
new `mod.rs` re-exports + `use super::*` lines). The largest file shrinks
from 978 to ~420.

## Visibility Strategy

- All `pub const SENGOO_DB_*` stay `pub` (consumed by tests via wildcard
  re-export from the module root, and by `state.rs` for default error codes).
- Private structs `DbConnection`, `DbTable`, `DbQueryResult`, `DbErrorState`
  stay private to the `runtime_db` module — promoted to `pub(super)`
  (i.e. visible only within `runtime_db/`) so submodules can construct and
  destructure them without exposing to the outer reflect module.
- The three `OnceLock` statics (`NEXT_DB_HANDLE`, `DB_CONNECTIONS`,
  `DB_RESULTS`, `DB_LAST_ERROR`) and their accessor functions (`db_connections`,
  `db_results`, `db_last_error`, `next_handle`) move to `state.rs` as
  `pub(super)` so `exec.rs` and `mod.rs` can use them.
- `clear_error` and `set_error` move with the statics (they read/write
  `DB_LAST_ERROR`) and become `pub(super)`.
- FFI pointer helpers (`parse_c_string`, `parse_optional_json`,
  `copy_bytes_to_buffer`) move to `ffi_utils.rs` as `pub(super)`.
- SQL parsing helpers move to `sql.rs` as `pub(super)`.
- Statement execution functions move to `exec.rs` as `pub(super)`. Only
  `execute_statement`, `run_select`, and `value_to_string` are called from
  `mod.rs`; the rest can remain `pub(super)` for simplicity (they are still
  not visible outside the module).
- All 15 `#[no_mangle] pub extern "C" fn sengoo_db_*` stay in `mod.rs` so a
  reader can scan the ABI surface in one file.

## Test Strategy

- The three `#[cfg(test)] mod tests` integration tests in lines 890-978 stay
  in `mod.rs` so they continue to exercise the full FFI assembly end-to-end.
- `TEST_LOCK: OnceLock<Mutex<()>>` stays in `mod.rs` because it serializes
  the global statics that are now defined in `state.rs` — but since tests
  go through the extern C surface (`sengoo_db_open` etc.) they do not need
  direct access to those statics.
- Helper functions `c_str` and `read_cell` stay in `mod.rs::tests` since they
  are test-only conveniences.
- No per-submodule unit tests are added in this change. Pure-function
  candidates (`normalize_identifier`, `parse_literal`, `find_keyword_case_insensitive`)
  could grow unit tests later; that is intentionally deferred so this change
  stays purely structural.

## Slice Plan

Each slice ends with the full verification baseline (compiler, sgc, runtime,
sgpm). No commit lands without all four green.

### Slice 0: Mechanical rename

`runtime_db.rs` → `runtime_db/mod.rs` with byte-identical content. Confirms
the rustc module-system understands the directory form and the build still
links the same extern C symbols.

### Slice 1: `status.rs`

Smallest possible extraction: 6 `pub const`. Validates the visibility,
re-export, and `use` pattern. Update `mod.rs` to `mod status; pub use status::*;`.

### Slice 2: `state.rs`

Move `DbConnection` / `DbTable` / `DbQueryResult` / `DbErrorState` +
`NEXT_DB_HANDLE` / `DB_CONNECTIONS` / `DB_RESULTS` / `DB_LAST_ERROR` +
accessors + `next_handle` + `clear_error` + `set_error`. Add
`use super::status::*;` for the error-code constants.

### Slice 3: `ffi_utils.rs`

Move `parse_c_string` / `parse_optional_json` / `copy_bytes_to_buffer`.
Depends only on `set_error` from `state.rs` and `SENGOO_DB_ERR_*` from
`status.rs`.

### Slice 4: `sql.rs`

Move `normalize_identifier` / `find_keyword_case_insensitive` /
`parse_literal` / `resolve_param_token` / `parse_where_clause` +
`value_to_string`. Depends on `set_error` from `state.rs` and status codes.

### Slice 5: `exec.rs`

Move the seven statement-execution functions. Depends on `state.rs`
(types + error helpers), `sql.rs` (parsers), and `status.rs`. This is the
biggest cut.

### Slice 6: Cleanup + SOP capture

`mod.rs` should now hold only: `mod status; mod state; mod ffi_utils; mod sql;
mod exec;`, the 15 extern C functions, the tests module. Update
`runtime_db.md` doc sidecar to point at the new layout. Capture the SOP
in `tasks.md` so the next P0 change can copy it.

## Risks and Mitigations

| Risk | Likelihood | Mitigation |
|---|---|---|
| Extern C symbol accidentally dropped or renamed | Low | Slice 0 is byte-identical rename; slices 1-5 only move helpers, never `#[no_mangle]` items. CI smoke test asserts `sengoo_db_open` substring. |
| `pub(super)` mistake breaks a callsite | Medium | Each slice runs full cargo test on `sengoo-runtime`. Compile errors caught immediately. |
| Test isolation breaks (TEST_LOCK skew) | Low | `TEST_LOCK` and all three tests stay in `mod.rs`; no test moves. |
| Two submodules silently grow circular `use super::*;` chains | Low | Visibility is strictly downward: `mod.rs` → `exec.rs` → `sql.rs` → `state.rs` → `status.rs`. `ffi_utils.rs` is a leaf sibling of `state.rs`. No cycles. |
| `serde_json::Value` re-import noise | Trivial | Each file that needs it imports it explicitly. |
| Doc sidecar `runtime_db.md` becomes stale | Trivial | Slice 6 updates it. |

## Open Questions

1. Should `value_to_string` live in `sql.rs` (where parse_literal lives) or
   `ffi_utils.rs` (since it's used by result-cell FFI exports)?  
   **Decision**: `sql.rs`. It is symmetric with `parse_literal` (one parses,
   one formats) and `ffi_utils.rs` should stay scoped to raw-pointer helpers.

2. Should the `runtime_db.md` sidecar be restructured to mirror the new
   submodule layout, or just appended with a "Module layout" section?  
   **Decision**: Append a "Module layout" section. The narrative remains
   useful as-is; restructuring is out of scope.

3. Should this change retroactively rename surviving `runtime_*.rs` siblings
   (`runtime_ffi.rs`, `runtime_lua54.rs`, etc.) for naming consistency?  
   **Decision**: No. Each gets its own split change. This one establishes
   the SOP only.
