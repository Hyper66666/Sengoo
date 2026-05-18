## Why

Sengoo already has a usable compiler core, but the developer toolchain and ecosystem are fragmented and partially implemented. This change establishes a prioritized, spec-driven roadmap so toolchain reliability and language capability growth can proceed in a coordinated way now.

## What Changes

- Define a complete LSP capability for `sglsp` using `tower_lsp`, including incremental sync, completion, definition, hover, and compiler-diagnostic integration via `sgc --error-format json`.
- Define formatting capability for `sgfmt` with configurable style (rustfmt-like config file) and `sengoo fmt` integration in `sgc`.
- Define package management capability under the new name `sgpm` (renaming from `sgpy`) with `Sengoo.toml`, semantic-version validation, and local path dependencies in the MVP; private registry support is deferred to a follow-up change.
- Define medium-priority language capabilities for generics, async/await concurrency, and macro systems.
- Define compiler/runtime optimization capabilities for incremental compilation classification, JIT/AOT dual mode, and Python interop improvements.
- Define ecosystem capabilities for docs/API docs generation and standard library collections/iterators/result-option completeness.
- **BREAKING**: Package manager naming and CLI/docs references move from `sgpy` to `sgpm`.

## Capabilities

### New Capabilities
- `lsp-tooling-sglsp`: Full language-server protocol support and diagnostic integration for editor workflows.
- `formatter-tooling-sgfmt`: Deterministic formatting with configurable style and first-class CLI integration.
- `package-management-sgpm`: Dependency/package workflows using `Sengoo.toml`, semver validation, and path-only dependencies in the MVP. Registry support remains a planned extension.
- `generics-core`: Generic functions and generic structs on top of existing type-variable infrastructure.
- `async-concurrency-model`: Async/await syntax and coroutine-based concurrency model.
- `macro-system`: Declarative and procedural macro expansion model.
- `incremental-compilation-accuracy`: AST-aware edit classification and improved module fingerprint invalidation.
- `jit-aot-execution-modes`: Fast developer JIT path plus production AOT path.
- `python-interop-embedding`: Stronger CPython embedding and Python extension-module export.
- `docs-and-api-reference`: Tutorial, API-doc generation, and runnable examples.
- `stdlib-core-collections`: Core collections, iterators, and complete `Result`/`Option` ergonomics.

### Modified Capabilities
- _None._

## Impact

- Affected code and crates: `tools/sglsp`, `tools/sgfmt`, `tools/sgpy` (to be migrated to `sgpm`), `tools/sgc`, `compiler/`, `runtime/`, `tools/stdlib`, and `docs/`.
- Affected developer interfaces: CLI commands, package configuration (`Sengoo.toml`), editor integrations, and project layout expectations.
- Affected architecture: type system, macro expansion pipeline, incremental build graph, runtime execution backends, and Python interop boundaries.
