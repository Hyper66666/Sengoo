## ADDED Requirements

### Requirement: Tooling command module splitting

Large-file split changes that decompose tooling or CLI command modules MUST preserve command entry points, command-line observable behavior, test-only re-export paths, and generated artifact semantics while moving implementation details into focused submodules.

#### Scenario: Command entry points remain stable

- **WHEN** a tooling module exposing `pub(crate)` command entry points is split into a directory module
- **THEN** existing callers MUST continue importing and invoking the same command functions through the same module root path
- **AND** command function names, argument order, return types, asyncness, and crate visibility MUST remain unchanged.

#### Scenario: CLI-observable behavior remains stable

- **WHEN** command orchestration code is moved into sibling files
- **THEN** CLI flags, stdout/stderr message text, message ordering, exit behavior, cache metadata, and generated artifact paths MUST remain unchanged
- **AND** existing command tests MUST pass without changing asserted output text or fixture values.

#### Scenario: Test-only helper re-exports remain stable

- **WHEN** helper functions used by crate-local tests are moved out of a large tooling module root
- **THEN** existing test imports through the parent module or crate root MUST continue compiling unchanged
- **AND** any helper visibility widening required by the split MUST stop at `pub(super)` or `pub(crate)` as appropriate for the pre-existing test surface.
