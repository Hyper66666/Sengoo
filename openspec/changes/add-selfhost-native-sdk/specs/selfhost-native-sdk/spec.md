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

#### Scenario: A manifest attempts to authorize itself
- **WHEN** a supplied catalog omits the independent caller pin, changes its target or ABI, substitutes portable layouts or contains ambiguous duplicate JSON keys
- **THEN** SG admission SHALL reject it before publishing usable declarations
- **AND** exact provider source bytes, physical source identities and actual raw prototype spans SHALL be retained and verified independently of source spelling

#### Scenario: A development object supplies a runtime declaration
- **WHEN** a caller explicitly selects a cataloged development COFF object
- **THEN** its separate caller pin, physical AMD64 object format and defined external function symbols SHALL match the selection
- **AND** an absent or undefined symbol SHALL NOT supply a runtime binding
- **AND** defined function section IDs and values SHALL remain within the actual section count and corresponding section extent
- **AND** artifact certificate validation SHALL recheck the selected catalog ordinal, physical path/identity, snapshot digest and memory-buffer digest, rejecting changed mutable bytes or metadata
- **AND** reference-runtime components SHALL be rejected in final SDK mode even when their bytes match

#### Scenario: An admitted declaration certificate is corrupted
- **WHEN** a catalog entry, declaration ID, physical signature, source identity or manifest identity is changed
- **THEN** read-only certificate validation SHALL reject the changed relationship without interning new types
- **AND** SDK-owned IDs SHALL NOT be interpreted as program binding or type IDs without an explicit physical module mapping

### Requirement: Native declarations SHALL preserve language lifecycle contracts

Declarations SHALL distinguish physical C ABI from language view/Future types
and enumerate owner/lifecycle effects. M3 owns validating typed bridge facts.

#### Scenario: A native future family is declared
- **WHEN** a language wrapper uses native start/poll/result/cancel/drop operations
- **THEN** every operation has a real declared ABI binding and selected provider
- **AND** language outcome and handle origin remain explicit rather than inferred from an integer result or source spelling

#### Scenario: File readiness returns an outcome through an explicit pointer
- **WHEN** the native readiness result declaration is admitted
- **THEN** its physical signature SHALL retain a mutable pointer to the sixteen-byte, alignment-eight outcome followed by a consumed i64 handle and a unit return
- **AND** its one-byte bool fields at offsets zero and one and i64 error at offset eight SHALL retain their declared roles
- **AND** this SDK declaration SHALL NOT broaden arbitrary source aggregate FFI or authorize integer-to-reference casts

### Requirement: Final SDK materialization SHALL respect SG runtime ownership

Development reference providers SHALL be separate from final SDK components.
Final components SHALL be permitted reviewed shims/libraries with provenance,
and SHALL NOT conceal unmigrated algorithms or compiler logic.

#### Scenario: Ordinary SG self-building uses installed SDK binaries
- **WHEN** locked SDK artifacts are selected for ordinary self-building
- **THEN** their identities are verified without invoking Rust/C compilers, assemblers or an external linker
- **AND** any source rebuild occurs through a separate explicit development procedure
