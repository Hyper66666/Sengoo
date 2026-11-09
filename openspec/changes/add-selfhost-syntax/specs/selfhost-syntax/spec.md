## ADDED Requirements

### Requirement: SG tokenization SHALL preserve reference vocabulary and source bytes

The SG lexer SHALL retain ordered token kinds, byte spans and trivia for the
reference vocabulary. It SHALL preserve literal spelling for checked decoding,
emit explicit EOF on success and report malformed input or capacity failures
with a bounded byte-span diagnostic.

#### Scenario: A source is tokenized

- **WHEN** SG tokenization succeeds for supported source
- **THEN** nontrivia tokens and byte spans agree with the reference lexer
- **AND** whitespace/comments remain available for formatting

#### Scenario: A token is malformed or capacity is exhausted

- **WHEN** input contains an invalid token, unterminated literal/comment or exceeds its token bound
- **THEN** lexing reports the failure span without silently dropping input
- **AND** no successful complete-token-stream result is claimed

### Requirement: SG parsing SHALL produce structural program trees

Parsing SHALL consume a complete program into an append-only AST with checked
node IDs and child edges for declarations, statements, expressions, types and
patterns. Pratt binding/associativity and generic closings SHALL match the
reference rather than merely accept balanced delimiters.

#### Scenario: Arithmetic and assignment expressions parse

- **WHEN** a program contains mixed arithmetic, comparisons, bitwise operators or chained assignments
- **THEN** AST operands reflect the reference precedence and associativity
- **AND** node spans retain their source identity and byte range

#### Scenario: Program constructs parse

- **WHEN** supported declarations, generic/reference/function types, control flow, patterns, FFI or attributes are parsed
- **THEN** their AST nodes retain corresponding names/types/parameters/bodies/operands
- **AND** all returned child IDs resolve within the owning AST

#### Scenario: Declaration and binding contexts reject unsupported forms

- **WHEN** a complete input uses bare top-level fn, an ordinary/trait/class/impl function prototype, a top-level bodyless alias, a non-self reference parameter or a destructuring let binding
- **THEN** SG reports a syntax error as the reference does
- **AND** extern-block fn prototypes/statics, associated trait types and ordinary function bodies remain accepted

#### Scenario: A where clause names an undeclared type parameter

- **WHEN** the where clause names a parameter absent from that declaration's generic parameter list
- **THEN** parsing rejects the clause as the reference merge_where_bounds check does
- **AND** a matching declared parameter and its bound parse normally

### Requirement: Parser failures SHALL remain bounded and located

The parser SHALL reject malformed syntax, EOF, invalid capacity values and
excessive nesting with a first byte-span diagnostic and invalid root. It SHALL
check capacity arithmetic before overflow and ensure loops consume input or
fail. Failed and successful parse scopes SHALL reclaim owned resources.

#### Scenario: Input is incomplete or deeply nested

- **WHEN** a required token is missing or parser depth exceeds the supported bound
- **THEN** parsing terminates with a syntax or depth diagnostic at the relevant source span
- **AND** no complete valid root is returned

#### Scenario: Repeated parsing finishes

- **WHEN** positive and negative programs are parsed in repeated native scopes
- **THEN** tracked String, Buffer and opaque resources return to baseline

### Requirement: M2 completion SHALL require transformations and formatter parity

M2 SHALL retain open status until supported declarative macros, cfg/deprecation,
derive, formatted expression parsing and synthesized protocols agree with the
reference and the SG formatter preserves comments, nontrivia tokens and
idempotence. Raw syntax nodes alone SHALL NOT certify expanded semantics.

#### Scenario: Raw macro or attribute syntax is represented

- **WHEN** the AST retains unexpanded macro or attribute syntax
- **THEN** verification identifies that raw-syntax scope
- **AND** macro expansion, cfg filtering, derive or protocol synthesis remains incomplete until its own checks pass

#### Scenario: A source is formatted

- **WHEN** the SG formatter succeeds on supported source
- **THEN** the result parses and preserves the complete nontrivia token sequence and comments
- **AND** formatting that result again yields identical text

#### Scenario: Formatter and Hasher protocol bridges are synthesized

- **WHEN** expanded source has a top-level simple Display/Debug impl with self-taking fmt and no to_string, or a simple Hash impl with self-taking hash_into and no hash
- **THEN** the protocol stage adds the corresponding reference Formatter/Hasher bridge method inside that impl
- **AND** an existing bridge name, a missing self receiver, a qualified trait path or an inherent impl suppresses generation as in the reference
- **AND** generated source maps compose to the original impl site, deprecation metadata remains owned and unchanged, and applying the stage again does not change text

#### Scenario: Differential syntax verification runs

- **WHEN** the syntax gate processes the frozen supported Windows corpus
- **THEN** it records exact inputs and reference/candidate token and parse outcomes
- **AND** any supported discrepancy prevents claiming M2 completion

### Requirement: Reference-accepted incomplete test prefixes SHALL remain explicit negative cases

Only exact reviewed source/literal identities listed in
`selfhost/reference-exceptions.json` SHALL receive incomplete-prefix handling.
Verification SHALL assert reference acceptance, SG rejection and both parsers'
acceptance of explicit completed counterparts. Other reference-accepted fixtures
SHALL be accepted by SG. No general EOF exemption or implicit repair is permitted.

#### Scenario: A reviewed partial fixture is checked

- **WHEN** the fixture's decoded source hash and original literal file/offset/hash match the reviewed list
- **THEN** verification asserts that the reference accepts it and SG reports the named syntax error at EOF
- **AND** reference-token delimiter residue matches the explicitly reviewed missing braces
- **AND** appending the listed completion suffix produces an accepted positive counterpart in both parsers
- **AND** the result is counted as an explicit negative assertion rather than skipped

#### Scenario: An incomplete or changed fixture is not reviewed

- **WHEN** a reference-accepted input differs from a listed source or original literal identity
- **THEN** it receives no exception based only on its EOF error or file category
- **AND** any acceptance discrepancy fails verification pending review
