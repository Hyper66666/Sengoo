# Tasks

- [x] Extend recursive future-escape checks through wrapper types
- [x] Replace hashed async dispatch IDs with stable ordinals
- [x] Convert async-lowering CFG/remap/result-dispatch ICE paths into diagnostics
- [x] Split `async_lowering.rs` into helper submodules
- [x] Add regression coverage for wrapper escapes, dispatch IDs, and frame access
- [x] Remove `lowering.rs current_block.expect(...)` from user-triggerable path
- [x] Align `runtime.c sengoo_async_frame_free(...)` with the async frame debug/release contract
- [ ] First-cut split of `lowering.rs` for builtin/planning-heavy logic
