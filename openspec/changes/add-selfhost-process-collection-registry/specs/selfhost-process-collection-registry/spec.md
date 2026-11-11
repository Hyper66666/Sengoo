## ADDED Requirements

### Requirement: Process-profile handles SHALL occupy an explicit distinct domain

The new named APIs SHALL validate their fixed negative-token range, slot,
generation and kind, use zero only as creation failure, and never reset live or
retired generation state. They SHALL NOT replace existing registry ABIs implicitly.

#### Scenario: Mixed profiles and stale tokens
- **WHEN** an operation receives a valid positive context/reference token, a
  wrong-kind token, a closed generation, or an out-of-profile integer
- **THEN** it rejects that token before payload access
- **AND** old positive-profile APIs reject the new negative token.

### Requirement: Global collection algorithms SHALL remain SG

SG SHALL own initialization, slots, generation, kind, mappings, word-block policy,
and ownership cleanup. C SHALL expose only storage addresses, memory/OS operations
and callback ABI primitives. The RawVec wrapper SHALL reuse the qualified SG core.

#### Scenario: Two actual resource families
- **WHEN** a RawVec of actual owning values and an owning word-block are created,
  looked up, mutated, detached and freed under the new process profile
- **THEN** family/mapping checks and exact cleanup succeed with no cross-family access
- **AND** slot exhaustion releases unpublished owned resources.

### Requirement: Metadata synchronization SHALL not overclaim payload safety

Registry metadata SHALL be serialized. Callback/free execution SHALL occur only
after the entry is taken and the registry lock released. Raw payload lookup SHALL
retain an explicit unsafe exclusion of concurrent close/mutation.

#### Scenario: Destructor reenters the registry
- **WHEN** a real owning RawVec Drop callback creates and closes another family
- **THEN** it completes and all resources are released without lock recursion.

#### Scenario: Concurrent independent metadata operations
- **WHEN** actual host threads concurrently register, look up and take their own
  distinct stable payloads
- **THEN** slot counts/generations remain valid and final live entries are zero
- **AND** the result does not claim concurrent use of an unpinned payload.

### Requirement: The actual development deployment SHALL be source-bound

The gate SHALL qualify real storage DLL/import library and OS/tool/header inputs,
preserve `.bss` and real unwind, and use the qualified SG linker for final products.

#### Scenario: Two fresh deployments
- **WHEN** two directories rebuild every declared SG/C/DLL/import artifact
- **THEN** corresponding qualified bytes agree and both empty-PATH SG-linked
  products return 42 without output
- **AND** no final SDK/global-family/self-build or M8 completion is inferred.
