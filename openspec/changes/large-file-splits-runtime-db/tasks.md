# Tasks: large-file-splits-runtime-db

Pre-split baseline (must be reconfirmed once on a fresh checkout before
starting Slice 0):

```powershell
cargo test -p sengoo-compiler --lib   # expect 559
cargo test -p sgc                      # expect 217
cargo test -p sengoo-runtime --lib     # expect 42
cargo test -p sgpm                     # expect 18 + 8
```

## 1. Pre-split inventory

- [x] 1.1 Re-confirm the 15 `#[no_mangle] pub extern "C" fn sengoo_db_*` symbol
  list against `runtime/src/reflect/runtime_db.rs` and record it in the
  change notes. Expected list (from Phase 0 audit):
  `sengoo_db_last_error_code`, `sengoo_db_last_error_len`,
  `sengoo_db_last_error_copy`, `sengoo_db_last_error_clear`,
  `sengoo_db_open`, `sengoo_db_close`, `sengoo_db_ping`,
  `sengoo_db_exec`, `sengoo_db_query`, `sengoo_db_result_close`,
  `sengoo_db_result_row_count`, `sengoo_db_result_col_count`,
  `sengoo_db_result_col_name_len`, `sengoo_db_result_col_name_copy`,
  `sengoo_db_result_cell_len`, `sengoo_db_result_cell_copy`.
  Confirmed 16 symbols (one more than the planning count: the 4 error-state
  helpers + `open`/`close`/`ping`/`exec`/`query` + 1 result-close + 6 result
  accessors = 16, not 15 as the planning text said — doc-only error, the
  spec scenarios are unaffected because they reference the full FFI surface).
- [x] 1.2 Re-confirm the only external Rust callsite is the smoke assertion
  `tools/sgc/src/tests.rs::examples_smoke_reflection_db_open_query`
  (substring check on `sengoo_db_open`). Confirmed.
- [x] 1.3 Re-confirm the only external Sengoo callsite is
  `tools/stdlib/db.sg` lines 1-17 extern block. Confirmed.
- [x] 1.4 Run the verification baseline above and record exact pass counts in
  the slice 0 commit message. Confirmed pristine: compiler 559, sgc 217,
  runtime 42, sgpm 18+8.

## 2. Slice 0: Directory module rename

- [x] 2.1 Create directory `runtime/src/reflect/runtime_db/`.
- [x] 2.2 Move `runtime/src/reflect/runtime_db.rs` → `runtime/src/reflect/runtime_db/mod.rs`
  with `git mv` (byte-identical, no content edit). Git reported `R` rename
  with 100% similarity (0 insertions, 0 deletions).
- [x] 2.3 Verify `runtime/src/reflect.rs:13` (`mod runtime_db;`) still resolves.
  Confirmed via `cargo test -p sengoo-runtime --lib` returning 42/42.
- [x] 2.4 Run verification baseline; commit `refactor(runtime_db): convert to directory module (slice 0/6)`.
  Landed as commit `231a5bfb`.

## 3. Slice 1: Extract `status.rs`

- [x] 3.1 Create `runtime/src/reflect/runtime_db/status.rs` with the 7
  `pub const SENGOO_DB_*` items (1 `STATUS_OK` + 6 `ERR_*`; planning text
  undercounted as 6 — doc-only error, no spec impact).
- [x] 3.2 In `mod.rs`, replace the const definitions with `mod status; pub use status::*;`.
- [x] 3.3 Verify the still-in-`mod.rs` helpers (`DbErrorState::default`,
  `set_error` callsites, `clear_error` callsite, tests) still resolve
  `SENGOO_DB_STATUS_OK` etc. via the re-export. Confirmed via baselines.
- [x] 3.4 Run verification baseline; commit `refactor(runtime_db): extract status.rs (slice 1/6)`.

## 4. Slice 2: Extract `state.rs`

- [x] 4.1 Create `runtime/src/reflect/runtime_db/state.rs` containing:
  private types `DbConnection`, `DbTable`, `DbQueryResult`, `DbErrorState`
  (promoted to `pub(super)`, including their fields since Slice 5's exec
  helpers need direct field access for `conn.tables`, `table.columns`,
  `table.rows`); the four `OnceLock` statics; their accessor fns
  `db_connections`, `db_results`, `db_last_error` (all `pub(super)`);
  `next_handle` (`pub(super)`); `clear_error` and `set_error` (`pub(super)`).
- [x] 4.2 Add `use super::status::*;` to `state.rs` for the default error code
  in `DbErrorState::default`.
- [x] 4.3 In `mod.rs`, add `mod state;` and `use state::*;` for the
  symbols the remaining helpers need. Also pruned now-unused
  `use std::collections::HashMap;` and `use std::sync::atomic::*;` from
  `mod.rs` (HashMap field access only needs methods, not the type in scope).
- [x] 4.4 Run verification baseline; commit `refactor(runtime_db): extract state.rs (slice 2/6)`.

## 5. Slice 3: Extract `ffi_utils.rs`

- [x] 5.1 Create `runtime/src/reflect/runtime_db/ffi_utils.rs` with
  `parse_c_string`, `parse_optional_json`, `copy_bytes_to_buffer` (all
  `pub(super)`).
- [x] 5.2 Add `use super::state::set_error;` and `use super::status::*;` to
  `ffi_utils.rs`. (Split into two `use` statements rather than the planned
  combined `use super::{state::set_error, status::*};` for readability.)
- [x] 5.3 In `mod.rs`, add `mod ffi_utils;` and `use ffi_utils::*;`.
- [x] 5.4 Run verification baseline; commit `refactor(runtime_db): extract ffi_utils.rs (slice 3/6)`.

## 6. Slice 4: Extract `sql.rs`

- [x] 6.1 Create `runtime/src/reflect/runtime_db/sql.rs` with
  `normalize_identifier`, `find_keyword_case_insensitive`, `parse_literal`,
  `resolve_param_token`, `parse_where_clause`, and `value_to_string`
  (all `pub(super)`).
- [x] 6.2 Add `use super::state::set_error;`, `use super::status::*;`, and
  `use serde_json::Value;` to `sql.rs`.
- [x] 6.3 In `mod.rs`, add `mod sql;` and `use sql::*;`. Verified that the
  `sql: &str` parameter name in `exec_create_table` / `exec_insert` /
  `exec_delete` / `run_select` / `execute_statement` shadows the module
  name `sql` locally — harmless because callers use the items unqualified
  via `use sql::*;`.
- [x] 6.4 Run verification baseline; commit `refactor(runtime_db): extract sql.rs (slice 4/6)`.

## 7. Slice 5: Extract `exec.rs`

- [ ] 7.1 Create `runtime/src/reflect/runtime_db/exec.rs` with
  `exec_create_table`, `resolve_insert_columns`, `exec_insert`,
  `build_select_result`, `run_select`, `exec_delete`,
  `execute_statement` (all `pub(super)`).
- [ ] 7.2 Add `use super::{state::*, sql::*, status::*};` and
  `use serde_json::Value;` to `exec.rs`.
- [ ] 7.3 In `mod.rs`, add `mod exec;` and `use exec::*;`. After this, the
  module root contains only: `mod` declarations + `pub use status::*;` + the
  15 `#[no_mangle]` extern C exports + the `#[cfg(test)] mod tests`.
- [ ] 7.4 Run verification baseline; commit `refactor(runtime_db): extract exec.rs (slice 5/6)`.

## 8. Slice 6: Doc sidecar + SOP capture

- [ ] 8.1 Append a "Module layout (2026-05-20)" section to
  `runtime/src/reflect/runtime_db.md` describing the new directory module
  layout from §2-7 above.
- [ ] 8.2 Verify the line-count target was met: `mod.rs` ≤ 500 LoC, every
  submodule ≤ 350 LoC, largest file is strictly smaller than the original 978.
- [ ] 8.3 Verify `tools/stdlib/db.sg` and
  `tools/sgc/src/tests.rs::examples_smoke_reflection_db_open_query` still
  compile and pass unchanged.
- [ ] 8.4 Run verification baseline plus
  `cargo test -p sgc examples_smoke_reflection_ -- --nocapture` to exercise
  the FFI surface end-to-end.
- [ ] 8.5 Update `docs/plans/2026-05-18-next-priorities.md`: mark
  `large-file-splits-runtime-db` as the in-progress P0 slice and note that
  the SOP from §9 is now ready for re-use.
- [ ] 8.6 Commit `docs(runtime_db): update layout sidecar + SOP capture (slice 6/6)`.

## 9. Reusable Split SOP (apply verbatim to follow-up large-file changes)

This SOP is captured so the next file split (jit.rs, net.rs, interface.rs,
sglsp/main.rs, sgfmt/main.rs, lowering.rs) does not need to re-derive these
rules.

1. **Inventory first.** Enumerate every public item that crosses the module
   boundary (extern C fns, `pub fn`, `pub type`, `pub const`, `pub use`).
   Enumerate every external Rust + Sengoo callsite. Record both lists in
   the change tasks before any code moves.
2. **Run the verification baseline before Slice 0.** Record exact pass
   counts in the Slice 0 commit message.
3. **Slice 0 is always a byte-identical `git mv` to a directory module.**
   No content edit. This isolates "did the rename break anything?" from
   "did a helper move break anything?".
4. **Each subsequent slice extracts exactly one concern into exactly one
   new file.** Never bundle two concerns into one slice.
5. **Order slices from smallest concern to largest.** Constants first,
   then state, then helpers, then large business logic. This grows the
   `use super::X;` graph in only one direction (downward).
6. **Widen visibility to `pub(super)` not `pub(crate)` not `pub`.** Only
   pre-existing public items stay `pub`. Promotion to `pub(crate)` requires
   a justified note in tasks.md.
7. **Keep all integration tests in `mod.rs`.** They exercise the assembled
   FFI surface end-to-end and must not be sharded.
8. **Each slice ends with full baseline green:**
   ```powershell
   cargo test -p sengoo-compiler --lib
   cargo test -p sgc
   cargo test -p sengoo-runtime --lib
   cargo test -p sgpm
   ```
9. **Final slice updates the `.md` doc sidecar** (if one exists) with a
   "Module layout (YYYY-MM-DD)" section pointing at the new submodules.
10. **Final commit also bumps `docs/plans/2026-05-18-next-priorities.md`**
    with completion status and any newly-discovered follow-ups.

## 10. Archival prerequisites

- [ ] 10.1 All 30+ tasks above checked.
- [ ] 10.2 `openspec validate large-file-splits-runtime-db --strict` reports no errors.
- [ ] 10.3 `openspec list` shows the change as ready to archive.
- [ ] 10.4 Archive to `openspec/changes/archive/2026-05-XX-large-file-splits-runtime-db/`
  and promote the new `large-file-splits` capability spec into
  `openspec/specs/large-file-splits/spec.md`.
- [ ] 10.5 Update roadmap to point at the next split target
  (recommended: `compiler/src/codegen/jit.rs` per the original P0 plan,
  now that the SOP exists).
