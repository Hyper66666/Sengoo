## ADDED Requirements

### Requirement: SG type identity SHALL preserve checked structural meaning

The semantic core SHALL maintain deterministic type identities for primitive,
qualified, nominal and ordered compound types. Invalid IDs, argument edges and
capacity values SHALL fail before unchecked indexing or arithmetic overflow.
Repeated analysis scopes SHALL reclaim their owned arenas.

#### Scenario: Equivalent and distinct types are interned
- **WHEN** types are constructed repeatedly with the same or different kind, qualifier, nominal identity and ordered arguments
- **THEN** equivalent types reuse an identity and distinct types retain distinct identities
- **AND** all type arguments and metadata remain accessible through checked IDs

#### Scenario: Type storage receives invalid input
- **WHEN** a type edge references an absent ID or a storage limit is exceeded
- **THEN** construction fails without publishing a partial type
- **AND** prior type identities and owned resource counts remain valid

### Requirement: SG semantic analysis SHALL enforce the supported edition contract

Name/type/generic/trait/ownership analysis SHALL preserve the edition-2026
Windows Stable and Supported-subset source and rejection behavior. CLI and LSP
SHALL consume the same resolved results and structured located diagnostics.
Soundness corrections SHALL have named independent regressions and evidence.

#### Scenario: A supported program or rejection case is analyzed
- **WHEN** the frozen semantic corpus is checked by the SG frontend
- **THEN** accepted programs retain their documented observable behavior
- **AND** rejected programs retain the required diagnostic family and source range

### Requirement: Typed SG lowering SHALL make control flow and cleanup explicit

Checked programs SHALL lower to HIR and typed MIR with validated identities,
target/FFI contracts, basic blocks, places, calls, aggregates and ownership
cleanup on each supported exit. Backend input SHALL NOT rely on hidden Rust
frontend state.

#### Scenario: A value leaves through control flow
- **WHEN** a supported program branches, returns, propagates an error, moves an owner or cancels a task
- **THEN** the typed representation preserves the required result and exactly-once ownership cleanup
- **AND** malformed MIR is rejected before native emission

#### Scenario: A destructor explicitly destroys a contained owner
- **WHEN** a resolved user destructor directly or transitively calls another destructor, or performs a projected drop, without a verified field-inactivation mask
- **THEN** its type SHALL carry pending Drop glue kind four and SHALL NOT authorize recursive field destruction
- **AND** forging resolved glue kind one for that destructor SHALL be rejected by MIR validation before native emission

#### Scenario: Low-level MIR moves overlapping byte ranges
- **WHEN** MIR opcode 22 requests an unsafe overlapping memory move
- **THEN** its third operand SHALL be a u64/usize byte count, and the operation SHALL use byte alignment one and preserve memmove overlap behavior
- **AND** mutable destination and source SHALL be raw pointers with the same pointee type, the result SHALL be unit, flags SHALL equal one, and reserved auxiliary words SHALL equal zero
- **AND** signed counts, typed references, immutable destinations, incompatible pointees or a missing unsafe flag SHALL be rejected by MIR validation
- **AND** the unsafe producer SHALL guarantee the validity of both complete byte extents; the operation SHALL NOT implicitly transfer owned values or discharge their Drop obligations

#### Scenario: Saved typed facts contain deferred associated type equalities
- **WHEN** an unresolved associated projection is deferred within a generic callable
- **THEN** HIR schema three SHALL preserve the callable, expected type, actual type and source node as owned, validated facts across serialization
- **AND** invalid owners, types, origins, table lengths, incompatible outer shapes and unsupported bridge versions SHALL be rejected without repeating semantic analysis
- **AND** every concrete MIR callable instance SHALL substitute and normalize both sides and require exact resolved equality before lowering its body
- **AND** unresolved or unequal concrete obligations SHALL fail with a located generic lowering diagnostic instead of treating a projection as a wildcard

#### Scenario: Checked arithmetic selects a user implementation
- **WHEN** checked arithmetic carries a resolved operator method and instantiated callable signature
- **THEN** lowering SHALL preserve its receiver, argument, result and calling convention through an actual method call
- **AND** generic trait dispatch SHALL select the matching concrete implementation or reject an unresolved or ambiguous instance
- **AND** poisoned operator targets or signatures SHALL be rejected before emission

#### Scenario: An anonymous borrowed result outlives temporary storage
- **WHEN** a function returns a borrowed call result whose receiver or argument is local or temporary storage
- **THEN** ownership checking SHALL reject the escape regardless of whether that storage requires Drop
- **AND** a borrowed result consumed within its full expression or retained by a valid local alias SHALL preserve its storage lifetime
- **AND** synthetic borrowed receiver signatures SHALL retain the same shared borrowing contract as the supported operation

#### Scenario: Runtime contracts have no lowering implementation
- **WHEN** a checked callable declares requires or ensures and executable contract lowering is unavailable
- **THEN** lowering SHALL explicitly reject that callable as unsupported instead of silently discarding its runtime checks
- **AND** generic where clauses SHALL retain their separate type obligation meaning
