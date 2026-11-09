## ADDED Requirements

### Requirement: Native SDK admission SHALL verify caller-pinned provider identities

A selected SDK SHALL identify its native family, target, pointer width, ABI,
component class, exact exports and immutable provider/source hashes. Admission
SHALL verify a caller-supplied manifest pin and expected target/ABI. Portable
logical layouts SHALL NOT substitute for native physical prototypes.

#### Scenario: An SDK export is selected
- **WHEN** a compiler or linker consumes an SDK declaration
- **THEN** its exact prototype and provider identities match the pinned native manifest
- **AND** missing, conflicting, wrong-target, wrong-ABI or changed-content inputs fail explicitly

### Requirement: Native declarations SHALL preserve language lifecycle contracts

Declarations SHALL distinguish physical C ABI from language view/Future types
and enumerate owner/lifecycle effects. M3 owns validating typed bridge facts.

#### Scenario: A native future family is declared
- **WHEN** a language wrapper uses native start/poll/result/cancel/drop operations
- **THEN** every operation has a real declared ABI binding and selected provider
- **AND** language outcome and handle origin remain explicit rather than inferred from an integer result or source spelling

### Requirement: Final SDK materialization SHALL respect SG runtime ownership

Development reference providers SHALL be separate from final SDK components.
Final components SHALL be permitted reviewed shims/libraries with provenance,
and SHALL NOT conceal unmigrated algorithms or compiler logic.

#### Scenario: Ordinary SG self-building uses installed SDK binaries
- **WHEN** locked SDK artifacts are selected for ordinary self-building
- **THEN** their identities are verified without invoking Rust/C compilers, assemblers or an external linker
- **AND** any source rebuild occurs through a separate explicit development procedure
