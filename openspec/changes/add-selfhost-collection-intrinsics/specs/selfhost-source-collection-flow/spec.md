## ADDED Requirements

### Requirement: A fresh scalar-flow source entry SHALL bind every executed collection site

The experimental entry SHALL reconstruct provider, typed module, origin, loan,
effect and scoped ownership facts from a source path and independent pins.
Every selected source call SHALL have one matching constructor, factory or
effect proof. Owning HIR6, checked concrete jobs, verified cleanup bodies and
strict optional MIR SHALL precede immediate object emission.

#### Scenario: Primitive source constructs, pushes and borrows

- **WHEN** a selected parameterless SG function returning i64 constructs a
  canonical Vec of i64 or bool, pushes, creates an iterator and uses canonical
  raw borrowed-next or a proved scalar helper
- **THEN** the actual selected providers and concrete callbacks implement those
  sites, with the published source overloads unchanged
- **AND** Some extraction requires the same immutable carrier and active token,
  None returns the fallback and repeated next preserves iterator ownership
- **AND** actual independent native execution restores resources after each call

#### Scenario: Every source job and call has its own proof

- **WHEN** selection reaches another unproved source call, generic/default body,
  custom owned element, manual handle family or mismatched physical declaration
- **THEN** the bounded profile rejects rather than executing a source placeholder
- **AND** selected functions are actual checked concrete bodies with exact
  declared signatures and the final job set belongs to the validated root set
- **AND** this checkpoint does not admit arbitrary generic helpers, owning
  element effects, constructor-return functions, get/set/remove or full self-build

#### Scenario: Mutating or escaping a borrow is rejected

- **WHEN** source uses a mutable or unguarded Option carrier, stale backing,
  cross-T/wrong-family handle, unknown native handle or moved owning argument
- **THEN** the original causal rejection remains observable through the fresh
  entry and no object is produced
- **AND** origin/loan candidates retain unlowerable one and are not promoted by
  changing serialized fields or by a reduction in diagnostic counts

#### Scenario: Cleanup is verified after the complete module is built

- **WHEN** lowering creates concrete Vec/iterator cleanup functions
- **THEN** the body has the exact typed receiver, provider call, handle/marker
  clearing, block/scope/terminator shape and two-field target layout
- **AND** malformed bodies, extra source jobs or incorrect true/false edge
  metadata fail strict validation without disabling the optional-borrow guard

#### Scenario: Fresh authority cannot be replaced by saved MIR

- **WHEN** the entry emits an object
- **THEN** it replays the independent source/provider/ownership proof against the
  original module, including the complete module-plus-ownership witness
- **AND** only the owned HIR arena receives lowering type instantiations
- **AND** ordinary lower/native entries still reject optional opcode twenty-seven
- **AND** complete owned serialization of new effects/NLL/handle authority remains
  a separate open HIR migration, rather than a capability of existing HIR6 wire
