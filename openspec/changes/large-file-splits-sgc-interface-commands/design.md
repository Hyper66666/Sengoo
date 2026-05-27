## Context

The P0 Large File Splits track has already validated three reusable patterns: simple directory modules, sibling inherent-impl blocks, and roots that already own child helper directories. The next roadmap target is `tools/sgc/src/interface.rs` plus `tools/sgc/src/commands.rs`, two large `sgc` toolchain files that currently mix multiple concerns.

Current inventory before implementation:

- `tools/sgc/src/interface.rs`: 2274 LoC. Public crate-facing surface re-exported from `main.rs` includes `interface_fingerprint_from_program`, `ast_interface_signature`, `function_fingerprints_for_module`, `function_fingerprints_for_program`, `function_signatures_for_module`, `generic_fingerprints_for_module`, and `generic_fingerprints_for_program`.
- `tools/sgc/src/commands.rs`: 1390 LoC. Public crate-facing surface re-exported from `main.rs` includes `cmd_build`, `cmd_run`, and test-only helpers `can_reuse_artifacts_for_unreachable_impl_only_changes` and `can_skip_codegen_via_generic_cache`.
- `tools/sgc/src/main.rs` currently declares `mod commands;` and `mod interface;` and re-exports the above functions for command dispatch and tests.
- `tools/sgc/src/tests.rs` imports a broad `super::{...}` surface and directly exercises `cmd_build`, fingerprint helpers, generic fingerprint helpers, and workset optimization helpers.

The split must be behavior-preserving. It must not change CLI flags, stdout/stderr text, cache metadata, workset decisions, reflection sidecar decisions, incremental behavior, or test assertions.

## Goals / Non-Goals

**Goals:**

- Reduce `interface.rs` and `commands.rs` below the roadmap target by splitting each into focused directory modules.
- Preserve all existing `pub(crate)` function names, signatures, and `main.rs` re-export paths.
- Keep implementation helpers private or `pub(super)` only where sibling module boundaries require access.
- Keep `sgc` tests compiling without changing asserted behavior.
- Record final file-size evidence, formatting evidence, and verification evidence in `tasks.md`.

**Non-Goals:**

- No CLI flag additions, removals, or behavior changes.
- No cache schema, fingerprint algorithm, workset-planning, daemon, native toolchain, or reflection semantics changes.
- No attempt to split other large files such as `pipeline.rs`, `bench.rs`, `tests.rs`, or `runtime/src/net.rs` in this change.
- No broad test rewrites or output string edits to make tests pass.

## Decisions

### Decision 1: Split `interface.rs` before `commands.rs`

`interface.rs` is the largest file and has clearer internal concern clusters: AST/interface signature rendering, function fingerprint collection, function signature collection, generic item fingerprinting, generic instance collection, and shared type rendering helpers. Splitting it first reduces risk by keeping command orchestration unchanged while proving the module-root conversion.

Alternative considered: split both files mechanically in the same slice. This would create too large a review surface and make failures harder to localize.

### Decision 2: Preserve `main.rs` re-export paths

External crate users do not consume these APIs directly, but `tools/sgc/src/tests.rs` and sibling modules rely on `crate::...` and `super::{...}` imports. The root modules should continue to expose the same `pub(crate)` functions, either by keeping them in `mod.rs` temporarily or by `pub(crate) use` re-exports from sibling files.

Alternative considered: update all call sites to new submodule-qualified names. That would be noisier and would weaken the public-surface preservation evidence for this Large File Splits track.

### Decision 3: Use directory modules with focused sibling files

Planned `interface/` layout:

- `mod.rs`: module declarations, imports/re-exports, small shared types if keeping them there minimizes visibility churn.
- `signature.rs`: AST path/type/decl/interface signature rendering and `ast_interface_signature` / `interface_fingerprint_from_program` entry points.
- `function_fingerprints.rs`: function call/import collection and `function_fingerprints_for_module` / `function_fingerprints_for_program`.
- `function_signatures.rs`: `FunctionSignatureInfo` extraction.
- `generic_items.rs`: generic item fingerprint extraction.
- `generic_instances.rs`: generic instance extraction and generic callable metadata helpers.

Planned `commands/` layout:

- `mod.rs`: module declarations/re-exports and minimal shared imports.
- `shared.rs`: override guards, contract check mode resolution, large-project mode helpers, shared source/build-dir setup if practical.
- `workset_optimizations.rs`: `can_reuse_artifacts_for_unreachable_impl_only_changes`, `can_skip_codegen_via_generic_cache`, reachability helpers, generic symbol helpers.
- `build.rs`: `cmd_build` orchestration.
- `run.rs`: `cmd_run` orchestration.

The exact slice boundaries may adjust after implementation inventory, but each slice should extract one focused concern or one command entry point.

### Decision 4: Keep command output strings byte-stable

Because many `sgc` tests assert CLI behavior indirectly through command output, cache behavior, and generated artifacts, this refactor must move code without changing output text. Any helper extraction must preserve statement order, fallback messages, and metadata save order.

Alternative considered: introduce shared helper abstractions to deduplicate `cmd_build` and `cmd_run`. That could reduce code size but risks subtle behavior changes; deduplication is out of scope unless it can be done mechanically with identical output ordering.

### Decision 5: Extend the spec only for CLI/tooling split invariants

Existing Large File Splits requirements cover API preservation, behavior preservation, file-size evidence, incremental slices, impl-block splitting, and existing child directory roots. This change adds one tooling-specific requirement: command modules must preserve command entry points and CLI-observable behavior when split.

## Risks / Trade-offs

- **Risk: `interface.rs` helper visibility churn** → Mitigation: keep shared AST rendering helpers in `mod.rs` or promote only to `pub(super)` when sibling files need them.
- **Risk: `commands.rs` uses broad `use crate::*` and may hide dependencies** → Mitigation: inventory imports per extracted file and prune unused imports per slice; avoid semantic rewrites while splitting.
- **Risk: CLI output ordering changes in `cmd_build` or `cmd_run`** → Mitigation: extract whole contiguous command bodies first, then only move private helpers around them; use `cargo test -p sgc` after every slice.
- **Risk: cache/workset behavior changes are hard to detect by unit tests alone** → Mitigation: keep targeted `sgc` tests plus full baseline green each slice; record pass counts in `tasks.md`.
- **Risk: rustfmt drift outside touched files blocks `cargo fmt --all -- --check`** → Mitigation: use touched-file rustfmt checks and document unrelated blockers, following prior split SOP.

## Migration Plan

1. Create inventory and baseline evidence before code moves.
2. Convert `interface.rs` to `interface/mod.rs` with a byte-identical mechanical rename.
3. Extract interface helper clusters in small slices, running targeted and full baselines after each slice.
4. Convert `commands.rs` to `commands/mod.rs` with a byte-identical mechanical rename.
5. Extract command shared/workset/build/run concerns in small slices, running targeted and full baselines after each slice.
6. Prune imports, run formatting checks, compute final line counts, update roadmap/tasks, then archive.

Rollback is straightforward because each slice is committed independently. A failing slice can be reverted without losing earlier validated module splits.

## Open Questions

- Whether `function_signatures_for_module` should live with function fingerprints or in a separate `function_signatures.rs` depends on final shared helper usage.
- Whether `cmd_build` and `cmd_run` should share any setup helper during this change should be decided conservatively during implementation; behavior preservation takes priority over deduplication.
