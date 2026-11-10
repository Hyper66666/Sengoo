## ADDED Requirements

### Requirement: Physical iterator handle obligations SHALL derive from actual scalar helper bodies

The module-level source contract layer SHALL validate independently pinned
collections, Option, provider declarations/artifacts, origins, loans and covered
NLL inputs before recognizing a conditional physical iterator handle contract.

#### Scenario: Canonical scalar helper reads a borrowed next result
- **WHEN** an actual pinned source function calls the canonical raw-next binding with one i64 parameter and its checked instantiated callable returns actual Option<&i64> or Option<&bool>
- **THEN** the layer SHALL record a conditional parameter/element/provider obligation only after checking the actual Some-tag guard and primitive dereference or proven bool-to-i64 conversion body
- **AND** function spelling, physical width, callback presence or raw i64 type alone SHALL NOT supply element or storage authority

### Requirement: Every helper caller SHALL discharge the exact parameter and projection relationship

Every actual call to a covered helper or supported pure forwarding wrapper
SHALL have an explicit receipt for each relevant physical-handle parameter.

#### Scenario: Canonical iterator field is passed by a real caller
- **WHEN** the actual argument resolves to canonical iterator field ordinal zero named handle with exact field owner/type and same element T
- **THEN** a concrete receipt SHALL require actual callable NLL coverage plus a reachable valid iterator token retaining root/T/lease provenance
- **AND** a conditional iterator parameter or raw helper parameter SHALL create a caller obligation and remain state two rather than become concrete

#### Scenario: Reconstructed or unrelated handles are passed
- **WHEN** the argument is ordinary integer data, a reconstructed iterator, an Owner handle, a mismatched element type or an invalidated backing
- **THEN** the receipt SHALL remain rejected or pending and SHALL NOT acquire concrete handle authority

### Requirement: Module coverage and candidate permissions SHALL remain explicit

Coverage SHALL distinguish a completely checked scalar helper/forwarding body
from a caller projection with unresolved conditional lifetime obligations.
The root-only NLL diagnostic query SHALL NOT authorize uncovered module bodies.

#### Scenario: Standard-library receiver has no concrete caller lease
- **WHEN** a canonical source method passes its parameter handle to an admitted helper
- **THEN** the record MAY transfer an exact conditional receiver/T requirement
- **AND** absent actual caller coverage, complete effects or a live backing lease SHALL NOT be treated as successful storage lifetime proof

#### Scenario: Candidate facts are passed to later compiler stages
- **WHEN** only this source-contract schema-one payload has been produced
- **THEN** it SHALL retain unlowerable one and SHALL NOT emit a safe Ref, copy arbitrary owned elements or suppress Drop by itself
- **AND** production borrow/HIR/MIR use SHALL require fresh independently validated provider, source, ownership and lifetime facts

### Requirement: Contract evidence SHALL be causal and purely validated

Validation SHALL reconstruct bounded module facts against independent caller
pins and actual typed graph input without type interning or semantic reruns.

#### Scenario: Obligation, receipt or provider evidence is absent
- **WHEN** a saved obligation, call receipt or mapped provider is cleared or altered
- **THEN** the same call SHALL lose its contract query result and payload validation SHALL reject
- **AND** source/type/member/provider changes SHALL reject while all typed arena contents remain unchanged
