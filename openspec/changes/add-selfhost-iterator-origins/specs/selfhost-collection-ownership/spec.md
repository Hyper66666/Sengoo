## ADDED Requirements

### Requirement: Fresh collection ownership SHALL use an explicit finalized typed input

Before source origin, loan, NLL or physical-handle facts are frozen, the
compiler SHALL materialize the actual reachable field and parent type
substitutions needed by ownership under explicit allocation/work limits.

#### Scenario: Generic function-valued fields require concrete types
- **WHEN** actual MapIter mapper or FilterIter predicate field substitution needs missing callable or reference TypeIDs
- **THEN** explicit finalization SHALL append them before producer facts are generated
- **AND** subsequent ownership analysis and same-context validation SHALL leave the original type words and arguments unchanged

#### Scenario: Unused forward placeholders exist in the type arena
- **WHEN** an early uninstantiated ADT placeholder is not reachable from actual checked bindings or value expressions
- **THEN** finalization SHALL NOT treat it as a complete actual generic instance
- **AND** a reachable malformed type, exceeded cap or failed append SHALL remain a fatal result rather than a readiness certificate

### Requirement: The new ownership entry SHALL retain existing schema and source diagnostics

Fresh collection ownership SHALL consume actual graph/checked source,
independent provider and collection/Option pins, and validated origin, loan,
NLL and physical-handle facts. It SHALL return the existing ShOwnership shape
without new saved trust flags or changed contract/action meanings.

#### Scenario: A complete primitive helper is analyzed under its condition
- **WHEN** its fresh exact-body proof establishes two primitive parameters and one non-owning borrowed Option local with no owned escape
- **THEN** independent ownership analysis MAY check that function body under the explicit physical-handle precondition
- **AND** the precondition SHALL remain pending until actual callers discharge it

#### Scenario: A physical handle projection is copied at an exact helper call
- **WHEN** a fresh validated call receipt matches the actual argument, parameter, canonical member, element and provider
- **THEN** ownership MAY analyze that scalar projection as observation while checking the rest of the source normally
- **AND** partial standard callers, unknown integers, fake records, Owner fields, cross-element and invalidated backings SHALL NOT become unconditional reference authority

#### Scenario: A module still has ownership errors
- **WHEN** fresh all-function analysis retains standard-library diagnostics
- **THEN** the output SHALL retain those real diagnostics and error status
- **AND** it SHALL NOT be made successful by deleting diagnostics or clearing an earlier error

### Requirement: Function-scope ownership coverage SHALL be independently produced

An explicit bounded function-set entry SHALL start a new ownership analysis
for the exact requested actual functions. Its sidecar SHALL identify actual
declarations, module source, declared callable type, analysis mode, pending
conditions and function errors.

#### Scenario: Entry source is clean while unused standard code is not
- **WHEN** an actual entry function is analyzed independently with complete prior contract and liveness initialization
- **THEN** it MAY produce clean scoped ownership while full-module diagnostics remain separately visible
- **AND** this SHALL NOT be called full standard-library cleanliness or a monomorphic generic-job certificate

#### Scenario: Scope evidence or original ownership facts are altered
- **WHEN** requested functions, declared FnType, pending conditions, actions, private vectors, scalars or diagnostics differ from fresh same-context reconstruction
- **THEN** validation SHALL reject
- **AND** missing, duplicate, abstract or malformed requested functions SHALL NOT acquire coverage

### Requirement: Legacy behavior and runtime authority boundaries SHALL remain explicit

The legacy ownership entry and its 22-field shape, actions8/9/10 and contract
bits SHALL remain intact. Source candidate contracts SHALL NOT emit a runtime
borrowed pointer, suppress owning cleanup or bypass MIR admission on their own.

#### Scenario: Context evidence is absent or corrupted
- **WHEN** receipt, obligation, provider, type or physical source evidence fails fresh validation
- **THEN** the new ownership entry SHALL fail before using collection exceptions
- **AND** the ordinary ownership entry SHALL retain its previous behavior

#### Scenario: Source lowering consumes scoped ownership
- **WHEN** only a clean scoped source result exists
- **THEN** lowering SHALL still require the same fresh context, complete reachable job coverage, concrete generic substitutions and independent constructor/provider/optional-borrow proofs
- **AND** ordinary candidate-only HIR lowering SHALL remain denied
