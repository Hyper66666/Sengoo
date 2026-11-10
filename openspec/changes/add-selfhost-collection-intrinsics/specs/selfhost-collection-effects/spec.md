## ADDED Requirements

### Requirement: Collection effect refinements SHALL retain independently replayable proofs

An effect refinement SHALL bind the actual physical source, callable, concrete
element, backing token and selected provider. Saved facts and a provider's
required-contract mask SHALL NOT authorize a refinement by themselves.

#### Scenario: A concrete vector is pushed before creating a fresh iterator
- **WHEN** independently selected source/provider artifacts establish the exact canonical push contract for a known live same-T vector
- **THEN** the new flow proof MAY preserve that vector's owner token while invalidating all existing iterator and element tokens rooted in it
- **AND** any subsequently admitted iterator SHALL have a new canonical factory/backing proof at its actual CFG location
- **AND** continued use of an iterator or borrowed element created before the push SHALL reject
- **AND** a user destructor SHALL require an independently verified effect contract before push can preserve backing across it; this checkpoint admits only a unique empty actual Drop body, and rejects callbacks with calls or writes
- **AND** this checkpoint SHALL admit only actual struct declarations for ADT elements; class declarations, parent relations and enum declarations SHALL reject until their full inherited/variant drop effects are proved

#### Scenario: A primitive helper observes a raw handle field
- **WHEN** a fresh complete-body helper contract and actual caller receipt identify the same canonical iterator field, element and live token
- **THEN** an explicit proof MAY discharge that exact conservative scalar-handle observation
- **AND** the original loan/NLL diagnostics SHALL remain preserved with a row-specific supersession
- **AND** conditional callers, source-name shadows, forged fields, cross-T values, missing CFG coverage and unresolved diagnostics SHALL NOT acquire reference authority
- **AND** the backing SHALL remain valid at the actual call after all arguments have been evaluated

#### Scenario: An effect payload is replayed
- **WHEN** an effect payload or any provider/source/CFG/token relationship is altered
- **THEN** read-only replay against actual independently supplied pins and typed modules SHALL recompute and compare every owned table
- **AND** adjusted facts SHALL NOT be accepted as if they were qualified by the original loan/NLL validator
- **AND** decoding or candidate validation alone SHALL NOT grant native reference permission

#### Scenario: An Option carrier is changed after testing its tag
- **WHEN** a payload read refers to a mutable carrier or a different initializer from the actual borrowed-next producer
- **THEN** the old tag SHALL NOT authorize that payload
- **AND** admitted payload reads SHALL belong to the true branch of the same immutable, directly initialized Option local, with its exact element token still live

### Requirement: Source push integration SHALL transfer caller ownership responsibility

Source push integration SHALL use the separately qualified physical push ABI
builder only after proving the actual T and transferring responsibility from
the original source temporary or moved path, including non-Copy values.

#### Scenario: Push succeeds or fails without consuming its input
- **WHEN** the selected provider returns success after move, or invalid-argument without consumption
- **THEN** success SHALL disarm the transferred typed temporary while invalid-argument SHALL execute exactly one typed caller cleanup
- **AND** any original source temporary or moved path SHALL be disarmed when the sink takes responsibility
- **AND** lowering SHALL NOT infer cleanup eligibility from zero-valued element bytes
- **AND** a placeholder body's inferred observation contract SHALL NOT preserve an input that the admitted push consumes

#### Scenario: Push runs out of memory after consuming by Drop
- **WHEN** the qualified provider reports its audited out-of-memory status after dropping the input
- **THEN** lowering SHALL disarm that input without a second typed Drop
- **AND** independent actual allocation-failure execution SHALL prove the preexisting vector contents and owner remain valid
- **AND** statuses outside the selected contract SHALL fail closed
