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

#### Scenario: A Future declaration exposes a poll contract
- **WHEN** a Future trait declares poll with a shared, owning or absent self receiver
- **THEN** semantic analysis SHALL reject the declaration with a located Future-contract diagnostic even when no impl exists
- **AND** a mutable self receiver SHALL retain the declared output and context contract

#### Scenario: Pending polling requires a wake on its execution path
- **WHEN** a user Future constructs an evident pending Poll
- **THEN** its current path SHALL have registered a wake on the resolved context parameter before constructing that Poll
- **AND** optional loop bodies, uncalled lambdas and later wake calls SHALL NOT establish that prior wake
- **AND** canonical user-Future await SHALL retain the ordered bool-readiness and output-payload layout contract

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
- **THEN** HIR schema four SHALL preserve the callable, expected type, actual type and source node as owned, validated facts across serialization
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


#### Scenario: An intrinsic bridges a borrowed language value to a runtime ABI
- **WHEN** a checked intrinsic call exposes a borrowed view through a runtime function
- **THEN** its facts SHALL distinguish the language callable signature, real external runtime declaration and exact ABI signature, receiver type/layout and source-owner provenance
- **AND** missing, incompatible or corrupted authorization SHALL NOT be replaced by source-name dispatch or an arbitrary integer-to-safe-reference cast
- **AND** HIR schema four SHALL encode the six-word authorization table with unique source-call records, bounded lengths and pure validation of each language/runtime signature and receiver layout
- **AND** MIR opcode 26 SHALL accept only a shared String receiver, its field-zero signed-i64 handle load, the exact declared external C call returning pointer bits and a shared str result; flags SHALL be zero and auxiliary words SHALL identify kind one and that runtime function
- **AND** the pointer bridge SHALL retain the receiver SSA operand as its lifetime origin, reject unrelated receivers and forged call results, and SHALL NOT authorize a general integer-to-reference cast

#### Scenario: An owner passes through a projected-only wrapper
- **WHEN** a proved wrapper projects a legacy owner's scalar identity and its result is immediately consumed by a proved noescape observer
- **THEN** the caller SHALL preserve its owner and the callee SHALL suppress owning-parameter cleanup under a validated projected-parameter certificate
- **AND** ordinary raw returns, alias storage, unknown retaining calls and incompatible C prototypes SHALL retain conservative ownership transfer
- **AND** projected certificate kind nine SHALL be serialized with the parameter declaration, identity and scope, and SHALL be proved from an initially empty inference state rather than authorizing a recursive proof from the supplied certificate itself
- **AND** independent native execution SHALL verify two observations and exactly one release of each source owner while rejecting corrupted profile facts

#### Scenario: A generic callable retains a deferred association equality
- **WHEN** a generic body relates a declared type to an unresolved associated projection
- **THEN** the located per-callable equality SHALL survive HIR serialization and SHALL be rechecked after concrete substitution
- **AND** a mismatched or still-unresolved concrete obligation SHALL be rejected before native emission

#### Scenario: An owned Buffer calls an audited native getter repeatedly
- **WHEN** a callable already owns a Buffer and reads its scalar handle through the exact fixed-arity C buffer_ptr contract
- **THEN** the non-retaining getter input SHALL NOT move that Buffer before later getter use or its required cleanup
- **AND** projecting that output through a source wrapper SHALL retain the conservative caller transfer rule outside a proved noescape context
- **AND** audited bounded-copy sinks SHALL require the real external fixed-arity signed-i64 prototype and observed destination position; source-name shadows, wrong arity and variadic prototypes SHALL NOT authorize the exception
- **AND** independent native execution SHALL verify stored bytes and exactly-once cleanup against the actual registered Buffer runtime, preserving diagnosed frozen-reference cleanup failures as differences

#### Scenario: A runtime pointer provider returns an error status
- **WHEN** an authorized borrowed-view runtime call returns zero or a negative pointer status
- **THEN** native lowering SHALL fail before passing that value to a safe borrowed-view consumer
- **AND** a successful valid-handle runtime probe SHALL NOT count as verification of this error path
