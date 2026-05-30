# large-file-splits Capability

## ADDED Requirements

### Requirement: Inherent impl block splitting

Large-file split changes MUST preserve the public type surface when decomposing a large inherent `impl` block across multiple sibling module files; the split may use multiple `impl TypeName { ... }` blocks, but public method names, signatures, return types, and re-export paths MUST remain unchanged.

#### Scenario: Public constructor and methods remain available

- **GIVEN** a module exposes `pub struct TypeName` with public inherent methods such as `new`, `generate`, and `to_string`
- **WHEN** the implementation is split from a single `impl TypeName` block into sibling submodules that each contain their own `impl TypeName` block
- **THEN** external consumers MUST still compile using the same public paths and method calls
- **AND** no public method MAY require a new trait import, wrapper type, or module-qualified helper call.

#### Scenario: Cross-file helpers use module-scoped visibility

- **GIVEN** a private method from the original impl block is moved to one sibling submodule and called by a method in another sibling submodule
- **WHEN** Rust privacy requires visibility widening for that call
- **THEN** the helper MUST be widened no further than `pub(super)` unless it was already public before the split
- **AND** methods used only inside their new submodule MUST remain private.

#### Scenario: Existing submodule helpers continue to attach to the same type

- **GIVEN** the original file already declares a child submodule containing `impl TypeName` helper methods
- **WHEN** the parent file is converted to a directory module
- **THEN** that existing helper submodule MUST remain under the same logical module path
- **AND** its helper methods MUST continue attaching to the same public type without changing caller code.
