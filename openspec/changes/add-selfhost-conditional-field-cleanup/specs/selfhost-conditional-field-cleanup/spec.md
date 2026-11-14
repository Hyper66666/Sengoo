## ADDED Requirements

### Requirement: Conditional named-field cleanup SHALL preserve checked ownership

For admitted concrete named aggregate paths, SG lowering SHALL represent the
initialization state of independently owned subobjects and release exactly the
unmoved initialized leaves on each supported exit. It SHALL preserve existing
source ownership decisions, including reserved legacy handle projection moves.

#### Scenario: Only one branch moves an owning field

- **WHEN** checked source moves one field on one predecessor and retains it on another
- **THEN** cleanup SHALL skip that field only on the moving path
- **AND** independent live siblings SHALL each be released exactly once in reverse field order
- **AND** moved-field and whole-value use SHALL retain their existing diagnostics

#### Scenario: Cleanup occurs through another supported exit

- **WHEN** an admitted function exits by normal scope completion, return or supported propagation
- **THEN** its live subobjects SHALL be cleaned before control leaves the scope
- **AND** transferred return or propagated payloads SHALL remain owned by the destination

#### Scenario: Source uses a retained legacy handle projection

- **WHEN** the existing checked rule classifies a reserved owning handle projection as a move
- **THEN** the corresponding path SHALL lose its cleanup obligation at that transfer
- **AND** this extension SHALL NOT authorize a copy, observing call, zero-disarm or later owner reuse

#### Scenario: An excluded partial-move shape is encountered

- **WHEN** a partial move crosses a custom-destructor ancestor, tagged payload, unsupported index/reference path, unresolved layout or pending Drop glue
- **THEN** the compiler SHALL retain an explicit located rejection before publishing an object
- **AND** it SHALL NOT substitute whole-aggregate cleanup or skip the containing body

### Requirement: MIR ownership paths SHALL be explicit, bounded and versioned

The MIR extension SHALL own its complete path representation and exact operation
contract. Consumers SHALL validate the schema, path geometry, concrete types,
scope, budgets and cleanup partition before code emission. A schema-one local
flag identifier SHALL NOT be silently reinterpreted as a field flag.

#### Scenario: HIR facts lower through independent copies

- **WHEN** complete validated HIR is directly lowered, cloned or serialized and restored
- **THEN** each admitted callable SHALL produce the same ordered path and lifecycle facts
- **AND** every relevant ownership action SHALL be accounted for at its actual control-flow point
- **AND** validation SHALL leave original arenas and tracked resources unchanged

#### Scenario: A path table or operation is malformed

- **WHEN** a root, parent, ordinal, type, function, duplicate record, missing owned field, reserved word or budget violates the declared contract
- **THEN** validation SHALL fail before emitting successful bytes
- **AND** a parent and descendant SHALL NOT both own duplicate cleanup obligations
- **AND** old root flags SHALL NOT independently govern the same tracked storage

#### Scenario: A well-typed sibling mapping differs from the checked source

- **WHEN** generated cleanup selects another compatible field instead of the original ownership action's field
- **THEN** the source-to-MIR mapping verification SHALL reject the mismatch
- **AND** an ordinary structural MIR check alone SHALL NOT be reported as proof of source equivalence

### Requirement: Reinitialization SHALL restore exactly its target ownership subtree

When admitted, replacement SHALL clean only old live target storage, initialize
the replacement and restore only its subtree's cleanup state. Evaluation failure
or early exit before the store SHALL not publish initialization.

#### Scenario: A partially moved aggregate is replaced

- **WHEN** a legal assignment replaces a parent whose children have different live states
- **THEN** only the surviving old children SHALL be released before the new value is stored
- **AND** the completed store SHALL initialize all owning leaves of the replacement
- **AND** stale old flags SHALL neither suppress new cleanup nor release moved old storage

#### Scenario: A single field is reinitialized

- **WHEN** legal source initializes one previously moved field
- **THEN** that field's new value SHALL receive exactly one future cleanup obligation
- **AND** moved siblings SHALL remain moved until their own legal initialization
- **AND** canonical zero-disarm action ten SHALL not count as reinitialization

#### Scenario: A loop carries ownership state over a backedge

- **WHEN** admitted source moves, replaces or retains fields over zero or more iterations
- **THEN** runtime state SHALL follow the executed path across every backedge
- **AND** loop-local declarations SHALL establish new state on each execution
- **AND** break, continue, return and propagation SHALL release exactly the live values for their exited scopes
- **AND** bounded-analysis exhaustion SHALL reject rather than assume initialization

### Requirement: Native field state SHALL preserve target layout and existing Drop contracts

Native path flags SHALL be private function-frame metadata. They SHALL not
change language aggregate layout, C parameter/result representation, public
runtime ABI or SDK headers. Typed field cleanup SHALL use exact validated
target offsets and existing resolved Drop glue.

#### Scenario: A flagged aggregate crosses a C boundary

- **WHEN** independently compiled C passes or receives an admitted aggregate
- **THEN** its size, alignment, padding, register/stack classification and hidden result pointer SHALL match the existing ABI
- **AND** C-observed allocation identity and destructor logs SHALL show exact transfer and cleanup

#### Scenario: A skipped field has invalid moved-from bytes

- **WHEN** the field's ownership flag is clear
- **THEN** cleanup SHALL neither read the field payload nor call its destructor
- **AND** it SHALL continue to release independently live sibling leaves in order

#### Scenario: Private flags enlarge the frame

- **WHEN** field state increases the required stack allocation or crosses a probe boundary
- **THEN** checked frame layout, stack alignment, probing and unwind metadata SHALL describe the actual code
- **AND** recursive Drop scratch and outgoing-call areas SHALL remain disjoint and bounded
- **AND** this evidence SHALL not be presented as language exception-unwind cleanup support

#### Scenario: A custom destructor has an unproven nested release

- **WHEN** existing analysis requires pending glue because a destructor directly or transitively destroys a field without a verified inactivation mask
- **THEN** the new path mechanism SHALL retain rejection
- **AND** it SHALL NOT flatten that custom destructor into independent leaves to bypass the missing proof

### Requirement: Standard String qualification SHALL include actual resource semantics

Successful compilation of an imported standard-library body SHALL not qualify
its resource behavior. A claim for `Option<String>::unwrap_or` SHALL require
canonical captured source and the actual registered clone/free implementation,
with both branches and complete resource identity accounting.

#### Scenario: A nonzero selected String is cloned

- **WHEN** Some or None selects a real allocated source and the clone succeeds
- **THEN** the result SHALL own independent storage containing the expected bytes
- **AND** each original owned input and the eventual result SHALL be released exactly once under the qualified source contract
- **AND** complete repeated scopes SHALL restore the live allocation baseline

#### Scenario: Retained source ownership leaves the selected allocation live

- **WHEN** actual execution shows that the selected original loses its cleanup obligation but the clone provider does not consume it
- **THEN** the library resource gate SHALL fail and retain the allocation evidence
- **AND** successful object emission or a main result of 42 SHALL not turn the gate into a pass
- **AND** any proposed observing/source correction SHALL receive a separate source-bound and runtime-verified contract

#### Scenario: Clone fails or a zero placeholder is selected

- **WHEN** an actual-provider fault control returns a failure status, or the canonical source selects zero
- **THEN** the gate SHALL record the exact result representation and original-input lifetime behavior
- **AND** it SHALL distinguish API error behavior from leaks or duplicate release
- **AND** wrappers SHALL delegate ordinary behavior to the captured actual provider
