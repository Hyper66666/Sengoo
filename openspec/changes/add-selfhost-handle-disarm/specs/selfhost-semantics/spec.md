## ADDED Requirements

### Requirement: Canonical handle disarm SHALL preserve moved-owner state

An authorized storage disarm SHALL NOT reinitialize the moved owning root,
restore root use, or re-enable its Drop flag.

#### Scenario: A canonical handle is extracted then zeroed
- **WHEN** a direct let initializer extracts the exact handle field of an owned canonical Buffer, Vec or HashMap local and a later direct sibling statement writes literal zero to that field
- **THEN** ownership checking SHALL retain the root move and publish action ten with the assignment, original root, field path, lexical scope and unconditional flag
- **AND** the zero store SHALL NOT publish initialization or restore root ownership

#### Scenario: Layout or destructor is not the audited canonical form
- **WHEN** a proposed disarm has additional fields, noncanonical markers, unresolved or owning payloads, references, inheritance, an enum, or a destructor with other effects
- **THEN** it SHALL NOT be authorized by this rule
- **AND** authorization SHALL require the exact corresponding external fixed-C i64-to-i64 free call on the receiver handle from an SG unit callable taking a matching mutable receiver

#### Scenario: The move or assignment is unsuitable
- **WHEN** the preceding root event is a real consuming call, conditional move, intervening init/drop/disarm, a different field extraction, or the zero write is not an ordered direct sibling of its let initializer
- **THEN** the special disarm SHALL be rejected without reviving the root
- **AND** nonzero writes, later root reuse and repeated disarm SHALL retain explicit ownership diagnostics

### Requirement: HIR schema five SHALL encode validated disarm facts

The owned HIR bridge SHALL use schema five when carrying action-ten semantics,
while preserving the existing builtin-call six-word table and action-eight/nine
parameter certificate representations. The existing deferred-equality table SHALL remain four words per record.

#### Scenario: Saved disarm facts are restored
- **WHEN** a schema-five image contains action ten
- **THEN** pure validation SHALL recheck its AST block order, typed field/root, exact zero write, canonical layout/destructor and preceding root move
- **AND** invalid action node/path/conditional/prior move or receiver mutability SHALL be rejected without repeating semantic analysis or adding type IDs
- **AND** even a coordinated field-path and target/producer-resolution replacement SHALL require that exact field to belong to the root ADT, while root-path and field/node types SHALL match their actual owning binding and field types

#### Scenario: An earlier image is submitted
- **WHEN** schema four or another unsupported bridge version is decoded
- **THEN** decoding SHALL fail with 5502 rather than assigning new action semantics to the old version

#### Scenario: A disarm is lowered
- **WHEN** a validated moved-root disarm reaches native lowering
- **THEN** the original extraction SHALL retain the source initialization-flag clear, the assignment SHALL emit only the typed zero store, and the source SHALL have no new flag set or Drop instruction
- **AND** independent native execution SHALL verify exactly one destination release per allocation and no moved-source destructor call
- **AND** a frozen-reference zero-handle destructor invocation SHALL be preserved as a destruction-call-policy difference rather than falsely reported as a resource leak or semantic equivalence
