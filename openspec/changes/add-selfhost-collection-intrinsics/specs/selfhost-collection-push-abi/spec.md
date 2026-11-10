## ADDED Requirements

### Requirement: Physical push expansion SHALL retain status-dependent typed ownership

A physical MIR push builder receiving an already transferred concrete value
SHALL own one typed sink and preserve the selected provider's audited
consumption statuses. This builder SHALL NOT establish source permission.

#### Scenario: The selected provider consumes by move or by Drop
- **WHEN** the provider returns success zero after moving the sink value, or out-of-memory fourteen after dropping it
- **THEN** the generated CFG SHALL disarm that sink without a second Drop
- **AND** actual owning-value execution SHALL verify exactly-once release and preserve preexisting vector contents on allocation failure

#### Scenario: The selected provider does not consume the input
- **WHEN** the provider returns its audited invalid-argument status two with the sink unchanged
- **THEN** the generated CFG SHALL perform one typed caller Drop and disarm the sink
- **AND** cleanup SHALL use explicit ownership state rather than inferring ownership from zeroed bytes
- **AND** statuses outside the selected contract SHALL fail closed

#### Scenario: A physically valid push is used by a source compiler
- **WHEN** the source compiler requests physical push expansion
- **THEN** it SHALL separately establish the actual provider, same-T backing, concrete layout/callback and source value-transfer relationships
- **AND** physical ABI validation alone SHALL NOT authorize a nominal, raw-integer or decoded source candidate
