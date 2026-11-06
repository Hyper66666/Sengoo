# Foundation implementation and remaining gates

- [x] Review public types, retained file policy and source-consistency scope with integration owner.
- [x] Implement native bounded snapshot with complete-read, forced-failure, reparse and pinned-launch fixtures (`native_file_snapshot_guard_bounds_and_cleanup`).
- [x] Verify public ownership, move/Drop/close and guarded writes (`public_file_snapshot_move_drop_and_guarded_paths`).
- [x] Implement pure Sengoo bounded PNG/JPEG/PDF headers and truthful capabilities (`pure_sengoo_media_header_classification_and_explicit_limits`).
- [x] Verify copied installed consumers without Cargo/Rust/clang on runtime PATH (`installed_file_media_consumer_uses_copied_payload_without_cargo`).
- [x] Document Windows x64 evidence and unsupported/non-release boundaries in `docs/bounded-file-media.md`.
- [ ] Obtain clean native sanitizer qualification; current Windows ASan run exits zero but reports an interception warning.
- [ ] Verify source-physically-hidden consumer and actual packaged release on required hosts.
- [ ] Integrate responsive cooperative or bounded-worker reads before claiming live TUI attachment readiness.
- [ ] Close consumer media acceptance and request-size admission through separately named network/JSON checks.

Four local Cargo tests pass. Full PDF parsing, text extraction and rendering
remain separate capabilities; header-only evidence does not close them. No
release, installed CLI or main compiler was overwritten by these fixtures.
