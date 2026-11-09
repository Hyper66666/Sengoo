## ADDED Requirements

### Requirement: Scalar iterator copying SHALL require concrete typed provenance

A scalar iterator bridge SHALL accept only the actual admitted VecIter<i64> or
VecIter<bool> receiver and its checked handle field projection. Physical slot
width, a function name or an arbitrary i64 value SHALL NOT establish Copy or
borrowing authority.

#### Scenario: A concrete scalar iterator is copied
- **WHEN** an admitted internal wrapper reads a scalar through its genuine shared VecIter<i64> or VecIter<bool> receiver
- **THEN** the checked facts SHALL identify that receiver type, handle field, source wrapper and real runtime callee
- **AND** the result SHALL be an independent scalar copy while the input owner retains its original lifetime and cleanup

#### Scenario: A nominal or generic owner resembles a scalar iterator
- **WHEN** an application supplies an unrelated same-shaped type, an unresolved generic iterator, a non-Copy element or a cast integer handle
- **THEN** the bridge SHALL reject authorization regardless of slot width or matching source spelling

#### Scenario: A non-Copy iterator handle is rewrapped as a canonical scalar iterator
- **WHEN** a source program moves a RawVecIter<Owner> identity into a canonical VecIter<i64> record without a verified scalar producer origin
- **THEN** scalar-copy bridge authorization SHALL reject that value even if its nominal type, physical family and slot width appear compatible
- **AND** an accepted target counterexample SHALL remain an explicit incomplete soundness obligation rather than a completed rejection test

### Requirement: Iterator runtime bindings SHALL carry independent provider authority

The bridge SHALL validate a caller-pinned native provider, exact physical C
signature and actual declaration-to-program binding map. It SHALL keep SDK and
program type/binding namespaces distinct.

#### Scenario: A source declaration shadows an admitted helper
- **WHEN** a source function or another extern has the helper's spelling without its admitted physical declaration identity
- **THEN** it SHALL NOT acquire native observation or scalar-copy authority
- **AND** missing or negative synthetic binding IDs, conflicting declarations and wrong signatures SHALL be rejected

#### Scenario: An admitted provider changes
- **WHEN** the provider content pin, target, ABI, source prototype or selected defined symbol no longer matches
- **THEN** admission SHALL fail before publishing a usable call contract

### Requirement: Observation and serialization SHALL preserve scalar bridge obligations

Call-local observation SHALL be derived from validated typed/provider facts and
remain independently verifiable through HIR and concrete MIR. It SHALL NOT
generalize to unrelated C arguments, raw-handle aliases or Drop operations.

#### Scenario: A wrapper obtains an action-eight certificate
- **WHEN** source inference proves the wrapper's no-retain input behavior through an admitted scalar call
- **THEN** certificate revalidation SHALL start from empty summaries and recheck the call's type/provider relationships
- **AND** caller-owned cleanup SHALL remain exactly once at the original scope

#### Scenario: Saved facts are corrupted or remain generic
- **WHEN** HIR fields reference a wrong receiver, field, binding, source identity, provider or result type, or concrete substitution leaves a non-scalar argument
- **THEN** validation or lowering SHALL reject the facts without treating unresolved types as Copy
- **AND** validators SHALL NOT intern new types while examining untrusted facts

### Requirement: Runtime scalar copies SHALL preserve physical bounds and ownership

The selected implementation SHALL read exactly the admitted scalar width,
preserve fallback and advancement policy, and retain or release neither input.
Development reference implementations SHALL remain separate from SG runtime
algorithm migration and final SDK qualification.

#### Scenario: A helper receives an unavailable or incompatible physical value
- **WHEN** its iterator is invalid, stale, from a wrong producer family, exhausted or physically incompatible
- **THEN** it SHALL return the supplied fallback without advancing or dereferencing incompatible storage
- **AND** independent resource checks SHALL verify original owner cleanup and no leaked provider state

#### Scenario: A target experiment reduces actual compiler diagnostics
- **WHEN** a target-only bridge closes the named scalar wrapper diagnostics
- **THEN** its report SHALL preserve actual core diagnostics, concrete provider/source/tool identities and the remaining failures
- **AND** that result SHALL NOT count as complete core, production bridge, full M3 or final self-hosting qualification

#### Scenario: A typed-reference rewrite works without its call table
- **WHEN** the same ownership input is checked with its validated call table present and with that table cleared
- **THEN** the report SHALL compare the actual diagnostics and distinguish ordinary scalar field copies through references from any effect of provider-authorized observation
- **AND** an unchanged result SHALL NOT qualify contract-driven borrow integration as complete
