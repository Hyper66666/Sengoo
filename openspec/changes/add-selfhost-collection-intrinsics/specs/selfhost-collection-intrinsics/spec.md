## ADDED Requirements

### Requirement: Collection compiler facts SHALL require independent provider authority

Collection lowering SHALL authenticate selected provider artifacts, physical
prototypes and actual program bindings independently of saved candidate data.

#### Scenario: A declaration resembles a runtime intrinsic
- **WHEN** a source name, nominal layout, raw integer or decoded table resembles an admitted collection operation
- **THEN** lowering SHALL require the actual selected provider/source/type/field relationships before accepting it
- **AND** missing or mismatched authority SHALL fail explicitly without a reference fallback

### Requirement: Constructors SHALL produce real concrete storage contracts

Canonical construction SHALL derive target layout and generated move/drop
callbacks for the actual concrete element without inferring Copy from width.

#### Scenario: An owning or aligned element is stored
- **WHEN** a qualified constructor builds a collection for the concrete element
- **THEN** its callbacks and descriptor SHALL preserve alignment, move transfer, source clearing and exactly-once Drop
- **AND** zero-valued source placeholders and allocation-failure handles SHALL not be mistaken for successful storage allocation

### Requirement: Borrowed iterator results SHALL preserve active payload and backing lifetime

Borrowed-next lowering SHALL bind semantic element, producer, backing loan and
selected pointer provider, with explicit optional representation.

#### Scenario: The selected next provider returns null or a live element
- **WHEN** a fully authorized call returns its actual pointer result
- **THEN** null SHALL form None with no standalone zero Ref, while valid nonnull SHALL form Some under the checked backing relationship
- **AND** inactive payload extraction or invalidated backing SHALL not acquire safe reference authority

### Requirement: Serialization SHALL preserve authority boundaries and compatibility

HIR/MIR integration SHALL own its bounded candidate facts and revalidate actual
independently supplied authority before native lowering.

#### Scenario: A candidate is decoded or an authority table is removed
- **WHEN** saved facts are decoded, corrupted or stripped of provider/origin/loan relationships
- **THEN** decoding alone SHALL not authorize runtime operations and missing required relationships SHALL prevent lowering
- **AND** old HIR5 SHALL reject while existing builtin/deferred record widths and ownership actions retain their meanings
