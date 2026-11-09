## ADDED Requirements

### Requirement: Owning temporary method receiver cleanup
An accepted temporary owning receiver SHALL remain live through its borrowed
method chain and be released once when its owning scope exits.

#### Scenario: Result success payload borrowed by a chain
- **WHEN** a temporary `Result<JsonDoc, i64>` is propagated and its document is read through chained methods
- **THEN** the read returns the correct value before cleanup
- **AND** live JSON documents and admission reservations return to baseline afterward

#### Scenario: A later propagation exits early
- **WHEN** a method later in the borrowed chain returns an error propagated with `?`
- **THEN** the temporary document is released without a second destruction
- **AND** the original error is preserved

#### Scenario: Receiver ownership transfer
- **WHEN** a method consumes an owning temporary receiver
- **THEN** ownership belongs to the consumer and a retained caller Drop binding does not destroy it again

#### Scenario: Scoped branch returns a borrowed temporary view
- **WHEN** an accepted conditional or nested block yields a view of an anonymous owning producer to an outer consumer
- **THEN** the producer remains live through that consumer and subsequent argument evaluation
- **AND** cleanup releases only initialized producers on normal and propagated failure paths
- **AND** borrowing a temporary out of the function remains rejected

### Requirement: Discarded owning expression cleanup
An unused fresh owning expression result SHALL be released at its statement
boundary, without consuming an aliased variable or field.

#### Scenario: Discarded success in a bounded loop
- **WHEN** an expression statement discards a successful owning Call, Option, Result or propagated Try value repeatedly
- **THEN** its resources return to baseline before the next statement in each iteration
- **AND** statement cleanup does not retain resources until the surrounding loop exits

#### Scenario: Discarded result before propagation or transfer
- **WHEN** a later expression returns early or a receiver is consumed by its callee
- **THEN** cleanup preserves the original result and releases each retained owner once
- **AND** a bare variable or field used as an expression does not gain a duplicate owner
