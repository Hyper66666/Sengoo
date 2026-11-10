## ADDED Requirements

### Requirement: HIR origin payloads SHALL own source candidates without minting compiler authority

The HIR origin codec SHALL preserve every bounded origin/loan scalar, string,
vector and source diagnostic as independently owned candidate data. Caller
source pins and actual typed module/ownership inputs SHALL remain independent
of decoded payloads.

#### Scenario: Actual module candidates are lifted and decoded
- **WHEN** actual checked physical modules, actual ownership and independently supplied collection and Option source pins validate the saved candidates
- **THEN** HIR SHALL own their complete origin/loan and physical-module witness data without aliasing the input arenas
- **AND** read-only replay SHALL compare actual source, AST, checked arenas, ownership, physical configuration and all module maps without rerunning type checking or interning types
- **AND** source diagnostics SHALL survive cloning and codec roundtrip even when candidate validation returns zero
- **AND** candidate validation or decoding SHALL NOT grant constructor, provider, safe-reference or native lowering authority

#### Scenario: A module has no actual intrinsic references
- **WHEN** the actual physical-module maps identify no nonstandard constructor, iterator or raw-next candidate references, including aliases
- **THEN** the module SHALL use an explicit absent candidate form with its actual module witness
- **AND** unused standard-library declarations alone SHALL NOT imply an active nonstandard site
- **AND** missing source pins or candidate tables at an actual intrinsic site SHALL fail explicitly rather than becoming absent

#### Scenario: A payload, caller pin or source relationship is altered
- **WHEN** a candidate scalar/table/string, module witness, actual arena/map or external caller pin differs from the independently checked inputs
- **THEN** validation SHALL fail without changing those input arenas
- **AND** malformed schemas, UTF-8/scalars, widths, lengths, truncation, trailing data and global budgets SHALL reject
- **AND** all owned resources SHALL recover from the first successful or rejected call

#### Scenario: A caller tries to lower a saved candidate directly
- **WHEN** the ordinary public HIR lowering entry receives a present candidate or module witness without fresh compiler/provider authority
- **THEN** it SHALL reject 5008 before executing placeholder bodies
- **AND** changing unlowerable or an absence flag SHALL NOT bypass that rejection
- **AND** legacy flattened modules lacking physical authority SHALL retain explicit unsupported boundaries
