## ADDED Requirements

### Requirement: Collection compiler facts SHALL require independent provider authority

Collection lowering SHALL authenticate selected provider artifacts, physical
prototypes and actual program bindings independently of saved candidate data.

#### Scenario: A declaration resembles a runtime intrinsic
- **WHEN** a source name, nominal layout, raw integer or decoded table resembles an admitted collection operation
- **THEN** lowering SHALL require the actual selected provider/source/type/field relationships before accepting it
- **AND** missing or mismatched authority SHALL fail explicitly without a reference fallback

#### Scenario: A development provider is mapped into a checked program
- **WHEN** the namespace-two provider maps its six exports into actual module-loaded declarations
- **THEN** it SHALL require independently selected root, manifest/object pins, target, ABI and development mode at mapping and replay entrypoints
- **AND** it SHALL bind actual physical source, declaration, binding and callable types while leaving the base SDK's sixteen-entry identity unchanged
- **AND** changed declaration AST flags, name/arity, body or C-block membership SHALL prevent admission despite an unchanged cached binding row

#### Scenario: A symbol has the expected name without an executable definition
- **WHEN** an object lacks a unique in-bounds external function definition in executable code or its mutable bytes differ from the selected object hash
- **THEN** provider admission SHALL reject before granting mapping authority

#### Scenario: A mapping declares an operation's required contract
- **WHEN** the mapping records layout, backing, iterator, release or element-transfer preconditions
- **THEN** those masks SHALL describe obligations for subsequent compiler checks and SHALL NOT stand in for satisfied origin, loan or ownership facts

### Requirement: Constructors SHALL produce real concrete storage contracts

Canonical construction SHALL derive target layout and generated move/drop
callbacks for the actual concrete element without inferring Copy from width.

#### Scenario: An owning or aligned element is stored
- **WHEN** a qualified constructor builds a collection for the concrete element
- **THEN** its callbacks and descriptor SHALL preserve alignment, move transfer, source clearing and exactly-once Drop
- **AND** zero-valued source placeholders and allocation-failure handles SHALL not be mistaken for successful storage allocation

#### Scenario: A generated move or drop body is changed
- **WHEN** callback layout, source-clearing instructions, typed Drop, CFG edges or parameter relationships differ from the actual concrete-element contract
- **THEN** callback validation SHALL reject even if the function still has a valid C signature and valid general MIR
- **AND** a physically valid raw-MIR constructor SHALL still require independent provider and actual source authority before language-level lowering

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
