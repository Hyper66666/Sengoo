## 1. Inventory and baseline

- [x] 1.1 Record current branch, active change name, and working tree status before implementation.
- [x] 1.2 Record pre-split line counts for `tools/sgc/src/interface.rs`, `tools/sgc/src/commands.rs`, and existing `tools/sgc/src/*.rs` context.
- [x] 1.3 Inventory public crate-facing surfaces re-exported by `tools/sgc/src/main.rs` for `interface` and `commands`.
- [x] 1.4 Inventory direct callers and tests that use `cmd_build`, `cmd_run`, interface fingerprint helpers, generic fingerprint helpers, and workset optimization helpers.
- [x] 1.5 Identify command-output, cache, workset, reflection, and generated-artifact invariants that must remain byte-stable.
- [x] 1.6 Run baseline `cargo test -p sgc` and record pass counts.
- [x] 1.7 Run full baseline `cargo test -p sengoo-compiler --lib`, `cargo test -p sgc`, `cargo test -p sengoo-runtime --lib`, and `cargo test -p sgpm`; record pass counts.
- [x] 1.8 Commit planning artifacts and inventory evidence.

## 2. Slice 0: Mechanical `interface.rs` directory-module rename

- [x] 2.1 Move `tools/sgc/src/interface.rs` to `tools/sgc/src/interface/mod.rs` with no content edits.
- [x] 2.2 Verify `tools/sgc/src/main.rs` module declaration continues to resolve `mod interface;` unchanged.
- [x] 2.3 Confirm git reports an `R100` rename for the moved file.
- [x] 2.4 Run targeted `cargo test -p sgc interface_fingerprint` and `cargo test -p sgc generic_fingerprint` smoke tests.
- [x] 2.5 Run full baseline and record pass counts.
- [x] 2.6 Commit `refactor(sgc): convert interface to directory module (slice 0/N)`.

## 3. Slice 1: Extract interface signature rendering

- [x] 3.1 Create `tools/sgc/src/interface/signature.rs` for AST path/type/decl/interface signature rendering helpers.
- [x] 3.2 Move `interface_fingerprint_from_program` and `ast_interface_signature` into `signature.rs` while preserving root re-exports.
- [x] 3.3 Promote helpers only to `pub(super)` where sibling modules require them; keep private helpers private when possible.
- [x] 3.4 Prune imports in `interface/mod.rs` and `signature.rs`.
- [x] 3.5 Run targeted `cargo test -p sgc interface_fingerprint` and full baseline; record pass counts.
- [x] 3.6 Commit `refactor(sgc): extract interface signature helpers (slice 1/N)`.

## 4. Slice 2: Extract function fingerprint and signature helpers

- [x] 4.1 Create `tools/sgc/src/interface/function_fingerprints.rs` for function fingerprint collection.
- [x] 4.2 Move `function_fingerprints_for_module` and `function_fingerprints_for_program` with their private call/import collection helpers.
- [x] 4.3 Create `tools/sgc/src/interface/function_signatures.rs` for `function_signatures_for_module` and related signature-info helpers if it is separable without churn.
- [x] 4.4 Preserve all `main.rs` re-exports and test imports unchanged.
- [x] 4.5 Run targeted function fingerprint/signature tests and full baseline; record pass counts.
- [x] 4.6 Commit `refactor(sgc): extract interface function fingerprints (slice 2/N)`.

## 5. Slice 3: Extract generic fingerprint helpers

- [x] 5.1 Create `tools/sgc/src/interface/generic_items.rs` for generic item fingerprint extraction.
- [x] 5.2 Create `tools/sgc/src/interface/generic_instances.rs` for generic instance extraction and callable metadata helpers.
- [x] 5.3 Move `generic_fingerprints_for_module` and `generic_fingerprints_for_program` while preserving root re-exports.
- [x] 5.4 Keep shared type rendering helpers private or `pub(super)` only where cross-file calls require it.
- [x] 5.5 Run targeted `cargo test -p sgc generic_fingerprint` and `cargo test -p sgc generic_instance` smoke tests plus full baseline; record pass counts.
- [x] 5.6 Commit `refactor(sgc): extract interface generic fingerprints (slice 3/N)`.

## 6. Slice 4: Mechanical `commands.rs` directory-module rename

- [x] 6.1 Move `tools/sgc/src/commands.rs` to `tools/sgc/src/commands/mod.rs` with no content edits.
- [x] 6.2 Verify `tools/sgc/src/main.rs` module declaration continues to resolve `mod commands;` unchanged.
- [x] 6.3 Confirm git reports an `R100` rename for the moved file.
- [x] 6.4 Run targeted `cargo test -p sgc workset` and `cargo test -p sgc cache` smoke tests.
- [x] 6.5 Run full baseline and record pass counts.
- [x] 6.6 Commit `refactor(sgc): convert commands to directory module (slice 4/N)`.

## 7. Slice 5: Extract command shared state and workset helpers

- [x] 7.1 Create `tools/sgc/src/commands/shared.rs` for override guards, contract check resolution, and large-project mode helpers.
- [x] 7.2 Create `tools/sgc/src/commands/workset_optimizations.rs` for reachability, generic symbol, and cache/workset skip helpers.
- [x] 7.3 Preserve `can_reuse_artifacts_for_unreachable_impl_only_changes` and `can_skip_codegen_via_generic_cache` through the same crate-visible root path used by tests.
- [x] 7.4 Prune imports and restrict visibility to `pub(super)` or `pub(crate)` only where already required by tests.
- [x] 7.5 Run targeted `cargo test -p sgc workset` and full baseline; record pass counts.
- [x] 7.6 Commit `refactor(sgc): extract command shared workset helpers (slice 5/N)`.

## 8. Slice 6: Extract build command orchestration

- [x] 8.1 Create `tools/sgc/src/commands/build.rs`.
- [x] 8.2 Move `cmd_build` intact, preserving signature, asyncness, logging order, cache behavior, reflection sidecar behavior, and artifact paths.
- [x] 8.3 Promote only required shared helpers from `commands/shared.rs` or `commands/workset_optimizations.rs`.
- [x] 8.4 Run targeted build/cache/reflection smoke tests and full baseline; record pass counts.
- [x] 8.5 Commit `refactor(sgc): extract build command orchestration (slice 6/N)`.

## 9. Slice 7: Extract run command orchestration

- [x] 9.1 Create `tools/sgc/src/commands/run.rs`.
- [x] 9.2 Move `cmd_run` intact, preserving signature, asyncness, logging order, cache behavior, workset behavior, engine resolution behavior, and artifact paths.
- [x] 9.3 Promote only required shared helpers from sibling modules.
- [x] 9.4 Run targeted run/cache/engine smoke tests and full baseline; record pass counts.
- [x] 9.5 Commit `refactor(sgc): extract run command orchestration (slice 7/N)`.

## 10. Slice 8: Import pruning, file-size evidence, and documentation

- [ ] 10.1 Prune unused imports from all touched `interface/` and `commands/` files.
- [ ] 10.2 Run rustfmt check on touched files; if `cargo fmt --all -- --check` is blocked by unrelated formatting drift, record the blocker and the narrower touched-file command that passed.
- [ ] 10.3 Run `git diff --check` and record the result.
- [ ] 10.4 Compute final line counts for every file under `tools/sgc/src/interface/` and `tools/sgc/src/commands/`.
- [ ] 10.5 Verify size targets: both `mod.rs` roots below ~500 LoC if practical, every resulting file below its original root size, and every non-test file below the roadmap ~1000 LoC target or document any justified exception.
- [ ] 10.6 Update `docs/plans/2026-05-18-next-priorities.md` with implementation status and next Large File Splits candidate.
- [ ] 10.7 Update this `tasks.md` with final sizes, visibility promotions, formatting evidence, and final verification evidence.
- [ ] 10.8 Run final targeted `sgc` smoke tests and full verification baseline.
- [ ] 10.9 Commit `docs(sgc): update interface commands split status (slice 8/N)`.

## 11. Archival prerequisites

- [ ] 11.1 All implementation tasks above are checked and completion notes are recorded.
- [ ] 11.2 `openspec.cmd validate large-file-splits-sgc-interface-commands --strict` reports no errors.
- [ ] 11.3 `openspec.cmd list` shows the change ready to archive.
- [ ] 11.4 Promote the updated `large-file-splits` capability spec into `openspec/specs/large-file-splits/spec.md` if the CLI/tooling split requirement remains applicable after implementation.
- [ ] 11.5 Archive to `openspec/changes/archive/YYYY-MM-DD-large-file-splits-sgc-interface-commands/`.
- [ ] 11.6 Update persistent roadmap notes so the next split can reuse the tooling/CLI split SOP.

## 12. Evidence notes

- 1.2 Initial measured LoC before implementation: `tools/sgc/src/interface.rs` 2274; `tools/sgc/src/commands.rs` 1390. Nearby large `sgc` files include `pipeline.rs` 1440 and `bench.rs` 1000+ but are out of scope for this change.
- 1.3 Initial public crate-facing interface surface from `main.rs`: `interface_fingerprint_from_program`, `ast_interface_signature`, `function_fingerprints_for_module`, `function_fingerprints_for_program`, `function_signatures_for_module`, `generic_fingerprints_for_module`, `generic_fingerprints_for_program`, `cmd_build`, `cmd_run`, test-only `can_reuse_artifacts_for_unreachable_impl_only_changes`, and test-only `can_skip_codegen_via_generic_cache`.
- 1.4 Initial caller inventory: `tools/sgc/src/main.rs` re-exports command/interface helpers; `tools/sgc/src/tests.rs` imports the re-exported functions through `super::{...}`; `tools/sgc/src/frontend_snapshot.rs` calls `function_fingerprints_for_module` and `generic_fingerprints_for_module` through crate-root imports.
- 1.1 Initial branch/status evidence: implementation started on `codex/large-file-splits-sgc-interface-commands`; unrelated pre-existing untracked paths were `.tmp/`, `ChatGPT Image 2026年5月5日 23_30_50 (1).png`, `bench/`, and `website/`; active OpenSpec change path is `openspec/changes/large-file-splits-sgc-interface-commands/`.
- 1.5 Byte-stable invariants: preserve `cmd_build` and `cmd_run` signatures, asyncness, CLI flags, stdout/stderr text, logging order, cache metadata/key behavior, workset reachability/skip decisions, reflection sidecar generation/cleanup behavior, generated artifact paths, and `main.rs`/test re-export compatibility.
- 1.6 Baseline `cargo test -p sgc`: 217 passed; 0 failed.
- 1.7 Full baseline: `cargo test -p sengoo-compiler --lib` 559 passed; `cargo test -p sgc` 217 passed; `cargo test -p sengoo-runtime --lib` 42 passed; `cargo test -p sgpm` 18 unit + 8 integration passed.
- 1.8 Planning validation evidence: `cmd /c openspec validate large-file-splits-sgc-interface-commands --strict` passed before the planning/inventory commit.
- 2.1 Mechanical rename moved `tools/sgc/src/interface.rs` to `tools/sgc/src/interface/mod.rs`; no content edits were made in the moved Rust file.
- 2.2 `tools/sgc/src/main.rs` still declares `mod interface;` unchanged, and `cargo test -p sgc interface_fingerprint` compiled through the directory module path.
- 2.3 Git rename evidence recorded after staging: `tools/sgc/src/{interface.rs => interface/mod.rs} (100%)`.
- 2.4 Slice 0 targeted tests: `cargo test -p sgc interface_fingerprint` 2 passed; `cargo test -p sgc generic_fingerprint` 7 passed.
- 2.5 Slice 0 full affected baseline: `cargo test -p sgc` 217 passed; 0 failed.
- 3.1 Slice 1 created `tools/sgc/src/interface/signature.rs` for AST path/type/decl/interface signature rendering; post-format sizes: `interface/mod.rs` 1750 lines, `interface/signature.rs` 456 lines.
- 3.2 Root compatibility is preserved via `pub(crate) use self::signature::{ast_interface_signature, interface_fingerprint_from_program};`.
- 3.3 Cross-module helper promotions were limited to `pub(super)` for `ast_path_signature`, `trait_bound_signature`, `type_signature`, and `function_signature`; signature-only helpers such as `visibility_label`, `param_signature`, and `variant_field_signature` remain private.
- 3.4 `interface/mod.rs` imports were pruned to the remaining fingerprint/generic dependencies after extraction; touched files were formatted with `rustfmt --edition 2021 tools\sgc\src\interface\mod.rs tools\sgc\src\interface\signature.rs`.
- 3.5 Slice 1 verification: `cargo test -p sgc interface_fingerprint` 2 passed; `cargo test -p sgc` 217 passed.
- 4.1 Slice 2 created `tools/sgc/src/interface/function_fingerprints.rs` for call collection and function fingerprint entry points; post-format size 274 lines.
- 4.2 `function_fingerprints_for_module` and `function_fingerprints_for_program` moved behind root re-exports. `source_span_slice` and `function_symbol` remain in `interface/mod.rs` because generic fingerprinting also depends on them.
- 4.3 Slice 2 created `tools/sgc/src/interface/function_signatures.rs` for `function_signatures_for_module`; post-format size 80 lines.
- 4.4 Root compatibility is preserved via `pub(crate) use self::function_fingerprints::{function_fingerprints_for_module, function_fingerprints_for_program};` and `pub(crate) use self::function_signatures::function_signatures_for_module;`; `main.rs` imports were unchanged.
- 4.5 Shared helper visibility promotions were limited to `pub(super)` for `call_target_signature` and `collect_calls_in_stmt`, because generic instance collection reuses those helpers. Slice 2 verification: `cargo test -p sgc function_fingerprint` 3 passed; `cargo test -p sgc function_signature` 1 passed; `cargo test -p sgc` 217 passed.
- 5.1 Slice 3 created `tools/sgc/src/interface/generic_items.rs` for generic item fingerprint extraction, callable metadata structs, and impl method templates; post-format size 440 lines.
- 5.2 Slice 3 created `tools/sgc/src/interface/generic_instances.rs` for type inference/substitution, generic instance extraction, and `generic_fingerprints_*` orchestration; post-format size 959 lines.
- 5.3 Root compatibility is preserved via `pub(crate) use self::generic_instances::{generic_fingerprints_for_module, generic_fingerprints_for_program};`.
- 5.4 Shared visibility stayed constrained: `GenericCallableMeta`, `GenericMethodTemplate`, their fields, `collect_impl_method_templates_from_decl`, and `collect_generic_item_fingerprints_from_decl` are `pub(super)` for sibling access; type rendering helpers remain in `signature.rs` with prior `pub(super)` visibility.
- 5.5 Slice 3 verification: `cargo test -p sgc generic_fingerprint` 7 passed; `cargo test -p sgc generic_instance` 9 passed; `cargo test -p sgc` 217 passed. Final interface split sizes after slice 3: `mod.rs` 24, `signature.rs` 456, `function_fingerprints.rs` 274, `function_signatures.rs` 80, `generic_items.rs` 440, `generic_instances.rs` 959.
- 6.1 Mechanical rename moved `tools/sgc/src/commands.rs` to `tools/sgc/src/commands/mod.rs`; no content edits were made in the moved Rust file.
- 6.2 `tools/sgc/src/main.rs` still declares `mod commands;` unchanged, and `cargo test -p sgc workset` compiled through the directory module path.
- 6.3 Git rename evidence recorded after staging: `tools/sgc/src/{commands.rs => commands/mod.rs} (100%)`.
- 6.4 Slice 4 targeted tests: `cargo test -p sgc workset` 11 passed; `cargo test -p sgc cache` 18 passed.
- 6.5 Slice 4 full affected baseline: `cargo test -p sgc` 217 passed; 0 failed.
- 7.1 Slice 5 created `tools/sgc/src/commands/shared.rs` for large-project and contract-check override guards, contract check mode labeling/resolution, and large-project/env threshold helpers; post-format size 70 lines.
- 7.2 Slice 5 created `tools/sgc/src/commands/workset_optimizations.rs` for generic symbol, reachability, unreachable impl-only artifact reuse, and generic-cache codegen skip helpers; post-format size 126 lines.
- 7.3 Test-facing compatibility is preserved via `pub(crate) use self::workset_optimizations::{can_reuse_artifacts_for_unreachable_impl_only_changes, can_skip_codegen_via_generic_cache};` from `commands/mod.rs`.
- 7.4 Visibility stayed constrained: shared guards and helper functions are `pub(super)` for sibling command modules, workset skip helpers remain `pub(crate)` because existing tests import them through the crate command root, and internal reachability/generic-symbol helpers remain private.
- 7.5 Slice 5 verification: `rustfmt --edition 2021 tools\sgc\src\commands\mod.rs tools\sgc\src\commands\shared.rs tools\sgc\src\commands\workset_optimizations.rs`; `cargo test -p sgc workset` 11 passed; `cargo test -p sgc cache` 18 passed; `cargo test -p sgc` 217 passed.
- 7.6 Slice 5 file sizes after extraction: `commands/mod.rs` 1152 lines, `commands/shared.rs` 70 lines, `commands/workset_optimizations.rs` 126 lines.
- 8.1 Slice 6 created `tools/sgc/src/commands/build.rs`; post-format size 582 lines.
- 8.2 `cmd_build` was moved as an intact async command body behind `pub(crate) use self::build::cmd_build;`; `main.rs` and test imports remain unchanged.
- 8.3 No new helper promotions were required for build extraction. `build.rs` imports shared guards/mode helpers through `super::shared` and workset/cache skip helpers through `super::workset_optimizations`.
- 8.4 Slice 6 verification: `rustfmt --edition 2021 tools\sgc\src\commands\mod.rs tools\sgc\src\commands\build.rs`; `cargo test -p sgc build` 27 passed; `cargo test -p sgc cache` 18 passed; `cargo test -p sgc reflection` 18 passed; `cargo test -p sgc` 217 passed.
- 8.5 Slice 6 file sizes after extraction: `commands/mod.rs` 583 lines, `commands/build.rs` 582 lines, `commands/shared.rs` 70 lines, `commands/workset_optimizations.rs` 126 lines.
- 9.1 Slice 7 created `tools/sgc/src/commands/run.rs`; post-format size 579 lines.
- 9.2 `cmd_run` was moved as an intact async command body behind `pub(crate) use self::run::cmd_run;`; `main.rs` and test imports remain unchanged.
- 9.3 No new helper promotions were required for run extraction. `run.rs` imports shared guards/mode helpers through `super::shared` and workset/cache skip helpers through `super::workset_optimizations`.
- 9.4 Slice 7 verification: `rustfmt --edition 2021 tools\sgc\src\commands\mod.rs tools\sgc\src\commands\run.rs tools\sgc\src\commands\build.rs tools\sgc\src\commands\shared.rs tools\sgc\src\commands\workset_optimizations.rs`; `cargo test -p sgc run` 76 passed; `cargo test -p sgc cache` 18 passed; `cargo test -p sgc engine` 4 passed; `cargo test -p sgc` 217 passed.
- 9.5 Slice 7 file sizes after extraction: `commands/mod.rs` 9 lines, `commands/build.rs` 582 lines, `commands/run.rs` 579 lines, `commands/shared.rs` 70 lines, `commands/workset_optimizations.rs` 126 lines.
