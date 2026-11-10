## ADDED Requirements

### Requirement: Backing leases SHALL depend on verified producer origins

The standalone source-loan layer SHALL validate existing origins using actual
module/typed facts and independent caller pins before recording a lease.

#### Scenario: A canonical iterator borrows a backing vector
- **WHEN** a verified same-element iterator factory consumes a concrete or conditional canonical Vec origin
- **THEN** the lease SHALL retain its backing root, element type, producer, lexical scope and conditional parameter relation
- **AND** an inert, unknown, forged or temporary backing SHALL NOT obtain live-reference authority

#### Scenario: An element borrow outlives an inner iterator scope
- **WHEN** a tail borrowed candidate is stored in an outer binding while the actual backing remains alive
- **THEN** the element lease SHALL be preserved independently of the inner iterator lease
- **AND** a dead local or unsaved temporary backing SHALL produce an explicit source-loan rejection

### Requirement: Live backing effects SHALL conflict regardless of receiver spelling

Shared receiver syntax and scalar handle-copy permission SHALL NOT prove that
backing mutation, release or native escape is harmless.

#### Scenario: Backing is invalidated while a lease is active
- **WHEN** source uses a live backing through push, set, insert, clear, free, explicit Drop, owner transfer or raw-handle escape to an unknown callee
- **THEN** source-loan analysis SHALL record a conflict before any new borrowed-next authority
- **AND** whole iterator moves and reborrows SHALL preserve the same backing relationship

#### Scenario: A source callee affects a parameter other than its result
- **WHEN** a verified source body or transitive call may invalidate one of its actual parameters
- **THEN** every corresponding caller argument SHALL discharge that effect relationship, including scalar result functions
- **AND** unknown parameter effects SHALL remain conditional/conservative rather than acquiring no-effect authority from a reference type

#### Scenario: Effect-summary evidence is necessary
- **WHEN** an actually pure source observer is called with a live backing lease
- **THEN** its independently inferred source summary MAY allow the call
- **AND** the conservative no-summary control SHALL reject the same source call, while changed saved summaries SHALL fail validation

### Requirement: Candidate Option borrows SHALL preserve physical identity and open ABI boundaries

Source candidate analysis SHALL authenticate canonical Option identity/layout
and exact admitted iterator source functions without pretending that the
existing placeholder bodies implement runtime allocation or borrowed pointers.

#### Scenario: Borrow candidate is associated with Option
- **WHEN** the candidate result has Option<&T> shape
- **THEN** it SHALL refer to the actual independently pinned Option ADT and its checked bool-tag/value-T fields
- **AND** spelling-only shadows, mismatched source/type facts or altered lease tables SHALL fail validation without changing typed arenas

#### Scenario: Standalone source-loan facts are serialized or consumed later
- **WHEN** only standalone schema-one candidate facts have been verified
- **THEN** constructor/raw-next ABI bridging SHALL remain explicitly unlowerable and no new safe Ref SHALL be emitted
- **AND** later HIR6/MIR integration SHALL independently verify runtime provider, pointer validity, optional Some/null representation and physical-handle conditional obligations


#### Scenario: Scope-conservative candidate differs from final NLL behavior
- **WHEN** mutation follows the last reachable iterator use but occurs before lexical scope end
- **THEN** the standalone phase MAY conservatively reject the source and SHALL retain that exact compatibility boundary
- **AND** it SHALL NOT claim final canonical borrow compatibility until actual last-use liveness accepts this case
