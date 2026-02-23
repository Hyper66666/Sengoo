# A-Line Milestone: runtime-db-ffi-a

## Scope
- [x] Database runtime MVP: open/close/ping
- [x] Database runtime MVP: exec/query + result handles
- [x] Database runtime MVP: structured error mapping
- [x] C FFI path: init/call/release
- [x] Lua bridge path: load/exec/call/close
- [x] Unsafe boundary diagnostics: explicit status code + error message channel

## Validation Snapshot
- [x] `cargo test -q -p sengoo-runtime runtime_db -- --nocapture`
- [x] `cargo test -q -p sengoo-runtime runtime_ffi -- --nocapture`
- [x] Combined targeted run: `cargo test -q -p sengoo-runtime "runtime_" -- --nocapture`

## Notes
- This track is self-contained under `runtime/src/reflect/runtime_db.rs` and `runtime/src/reflect/runtime_ffi.rs`.
- No compiler/tooling/lsp/python modules were touched.
