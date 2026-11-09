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
