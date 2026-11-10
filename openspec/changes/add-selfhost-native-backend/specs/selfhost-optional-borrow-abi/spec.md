## ADDED Requirements

### Requirement: Optional borrow MIR SHALL preserve an inactive payload boundary

The native backend SHALL represent a qualified optional borrowed pointer as
the complete concrete bool-tag and immutable-reference aggregate. Physical
shape validation SHALL NOT itself authorize source-level reference creation.

#### Scenario: A selected pointer provider returns null
- **WHEN** an authorized opcode-27 operation consumes the actual selected fixed-C next-call result and that call's exact handle SSA input
- **THEN** null SHALL produce a false tag and an inactive payload without creating an independently usable zero reference
- **AND** nonnull SHALL preserve the actual returned address under the separate source/provider/backing contract
- **AND** ordinary MIR/native entrypoints lacking fresh source authority SHALL reject this operation explicitly

#### Scenario: A payload is observed through a local carrier
- **WHEN** the complete optional initializes one uniquely identified local or stays in direct SSA form
- **THEN** every payload extraction or payload-address load SHALL follow the true branch of a tag observation from that same optional initializer
- **AND** the false successor SHALL NOT reach the payload without retesting the same guard
- **AND** uninitialized or multiply initialized carriers, hidden aliases, whole-optional return/call/phi transfer and payload-reference escape SHALL reject
- **AND** reading a field of a borrowed non-Copy owner SHALL NOT copy or destroy that owner

### Requirement: Typed destruction SHALL initialize its actual layout dependency

Destruction of a supported aggregate SHALL initialize the layout for its actual
MIR type before obtaining field offsets, independent of destructor-body reads.

#### Scenario: A destructor body does not inspect fields
- **WHEN** actual MIR Drop targets an aggregate whose type has only appeared through pointer or collection-callback signatures
- **THEN** native emission SHALL visit that aggregate's layout before walking its fields
- **AND** unsupported Drop-glue categories SHALL retain their explicit rejection
- **AND** an empty source destructor SHALL remain observable as failing resource cleanup instead of accidentally failing layout lookup
