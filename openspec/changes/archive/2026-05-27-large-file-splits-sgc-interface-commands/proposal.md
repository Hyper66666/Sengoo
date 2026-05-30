## Why

`tools/sgc/src/interface.rs` (2274 LoC) and `tools/sgc/src/commands.rs` (1390 LoC) are the next roadmap targets in the P0 Large File Splits track. Splitting them into focused submodules will reduce review and maintenance risk while preserving `sgc` CLI behavior, incremental-cache behavior, diagnostics, and test outcomes.

## What Changes

- Convert `tools/sgc/src/interface.rs` into an `interface/` directory module with focused sibling files for AST signature rendering, function fingerprints, function signature collection, and generic fingerprints.
- Convert `tools/sgc/src/commands.rs` into a `commands/` directory module with focused sibling files for shared command setup, build orchestration, run orchestration, and workset/cache optimization helpers.
- Preserve all existing `pub(crate)` re-exports from `tools/sgc/src/main.rs`, including `cmd_build`, `cmd_run`, `can_reuse_artifacts_for_unreachable_impl_only_changes`, `can_skip_codegen_via_generic_cache`, `ast_interface_signature`, `function_fingerprints_for_module`, `function_fingerprints_for_program`, `function_signatures_for_module`, `generic_fingerprints_for_module`, `generic_fingerprints_for_program`, and `interface_fingerprint_from_program`.
- Preserve CLI-visible behavior, stdout/stderr text, cache keys, workset decisions, reflection sidecar behavior, and existing tests; this is a structural refactor only.
- Extend the Large File Splits SOP with a tooling/CLI command-module split requirement if implementation confirms reusable guidance.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `large-file-splits`: Add requirements for splitting tooling/CLI modules while preserving command entry points, CLI-observable behavior, and test-only re-export compatibility.

## Impact

- Affected code:
  - `tools/sgc/src/interface.rs`
  - `tools/sgc/src/interface/*.rs`
  - `tools/sgc/src/commands.rs`
  - `tools/sgc/src/commands/*.rs`
  - `tools/sgc/src/main.rs` only if module declarations/re-exports need mechanical updates.
- Affected tests:
  - `cargo test -p sgc`
  - Full baseline: `cargo test -p sengoo-compiler --lib`, `cargo test -p sgc`, `cargo test -p sengoo-runtime --lib`, `cargo test -p sgpm`.
- No external dependencies, public Rust crate APIs, or CLI flags are intended to change.
