## ADDED Requirements

### Requirement: sglsp SHALL maintain an incremental workspace symbol index

`sglsp` SHALL build a versioned index of workspace, direct dependency,
standard-library, and open-document symbols and SHALL update only affected
documents for normal editor and watched-file events.

#### Scenario: A developer edits one open document

- **WHEN** a newer incremental document change is received
- **THEN** the open-document overlay and its index entry are updated
- **AND** unchanged workspace and dependency documents are not reread or reparsed
- **AND** an older event cannot replace the newer indexed revision

#### Scenario: A workspace file cannot be indexed

- **WHEN** a file is temporarily invalid, unreadable, or deleted during refresh
- **THEN** the failure is isolated and logged without crashing the server
- **AND** completion remains available from valid indexed and open documents

### Requirement: Completion SHALL be constrained by syntactic and semantic context

The server SHALL distinguish general, member, namespace, import-path, and
attribute contexts and SHALL return candidates valid for that context rather
than arbitrary identifier-shaped document words.

#### Scenario: A receiver member is requested

- **WHEN** completion is requested after `.` on a resolvable receiver
- **THEN** only fields and methods compatible with the receiver type are returned
- **AND** unrelated global, namespace, and keyword candidates are excluded

#### Scenario: Source is incomplete while typing

- **WHEN** a request occurs in a recoverable incomplete declaration or expression
- **THEN** cursor-local token and delimiter recovery selects the bounded context
- **AND** comments, strings, and irrecoverably invalid regions do not leak global candidates

### Requirement: Import completion SHALL follow compiler-accepted Sengoo syntax

Import-path completion and auto-import SHALL be tested against the import forms
accepted by the compiler parser and rendered by `sgfmt`, including simple,
alias, selective, and wildcard forms.

#### Scenario: A user completes an import path

- **WHEN** completion is requested in an accepted import declaration form
- **THEN** modules or symbols valid at that grammar position are returned
- **AND** their replacement ranges do not consume punctuation owned by the form

#### Scenario: An imported candidate is resolved

- **WHEN** a unique unimported symbol needs an import
- **THEN** resolve supplies an `additionalTextEdit` that preserves the existing import block and newline style
- **AND** the edit is sorted, non-duplicating, and does not modify comments or strings
- **AND** ambiguous same-name origins remain separate choices rather than being guessed

### Requirement: Attribute completion SHALL be backed by executable capability evidence

The server SHALL advertise `#` completion and SHALL return only attributes and
nested attribute values whose target and form are proven by compiler, derive,
FFI, or `sgc test` fixtures.

#### Scenario: A user types an attribute introducer

- **WHEN** completion is requested after `#` or `#[` before a declaration
- **THEN** the server returns only catalog entries valid for that declaration target
- **AND** each entry identifies an executable owner fixture
- **AND** documentation-only or unsupported attributes are not returned

#### Scenario: A user completes inside derive

- **WHEN** completion is requested inside `#[derive(...)]`
- **THEN** built-in derive values proven by the derive implementation are returned
- **AND** externally configured or unknown derive names are not presented as built-ins

### Requirement: Completion ordering and replacement SHALL be deterministic

The server SHALL rank categories in the order local variables, parameters,
fields, imported symbols, project symbols, standard library, and keywords, and
SHALL use UTF-16 `TextEdit` ranges for the current document revision.

#### Scenario: Several categories match one prefix

- **WHEN** candidates from multiple categories share the typed prefix
- **THEN** their `sortText` follows the required category order
- **AND** prefix, label, and origin tie-breakers produce the same order across repeated requests

#### Scenario: Unicode precedes the replacement token

- **WHEN** non-BMP or multibyte characters occur before the cursor
- **THEN** the replacement range is expressed in correct LSP UTF-16 positions
- **AND** applying the edit replaces only the intended token

### Requirement: Completion resolve SHALL be version-safe

The server SHALL advertise completion resolve and SHALL compute documentation
and auto-import edits lazily without applying edit metadata to a changed
document.

#### Scenario: The document remains unchanged before resolve

- **WHEN** a schema-v1 item is resolved against the same URI, revision, symbol, and origin
- **THEN** markdown documentation is populated when available
- **AND** a required safe auto-import edit is populated when unambiguous

#### Scenario: The document changes before resolve

- **WHEN** the item's document revision no longer matches the indexed overlay
- **THEN** resolve returns no new edit-producing fields
- **AND** the server does not silently relocate or rewrite the stale import edit

### Requirement: sglsp SHALL expose a versioned completion metadata contract

The initialize result SHALL advertise
`sengoo.completionSchemaVersion = 1`, and completion data SHALL contain
`schemaVersion`, `symbolId`, `origin`, `category`, `documentUri`,
`documentRevision`, and `resolveKind`. `documentUri` SHALL be the canonical
serialized source-document URI, and `documentRevision` SHALL be the exact
integer version of the open LSP document used to calculate the item.

#### Scenario: A schema-aware client receives completion

- **WHEN** the client detects completion schema version 1
- **THEN** it can trust server category, ordering, replacement, and resolve identity
- **AND** it can disable equivalent legacy client-side filtering without losing completion

#### Scenario: A schema-v1 producer adds metadata

- **WHEN** a completion item contains unknown fields in addition to the required schema-v1 fields
- **THEN** schema-v1 consumers ignore the unknown fields without rejecting the item
- **AND** server resolve continues to use the required URI, revision, symbol, and origin identity
- **AND** removing or redefining a required field requires a new schema version

#### Scenario: A standard LSP client does not understand the schema

- **WHEN** the client ignores experimental capabilities and item data
- **THEN** standard labels, kinds, details, ordering, and text edits remain usable
- **AND** no Sencoder-specific request is required to obtain completion

### Requirement: Signature help SHALL resolve the active call and receiver

Signature help SHALL recover nested calls, resolve receiver methods and viable
signatures, include callable and parameter documentation, and report stable
active signature and parameter indexes.

#### Scenario: A nested receiver call is edited

- **WHEN** signature help is requested inside a method call containing nested argument expressions
- **THEN** the receiver-compatible signatures are returned
- **AND** commas inside nested delimiters do not advance the outer active parameter
- **AND** the best viable signature and clamped active parameter are selected

#### Scenario: The call cannot be resolved

- **WHEN** no indexed callable matches the active call and receiver
- **THEN** the server returns no unrelated same-name signature

### Requirement: Code Actions SHALL require deterministic diagnostics and semantic evidence

The server SHALL offer only unresolved-symbol import, unused-import removal,
and missing enum match-arm actions whose triggering diagnostic, current
document revision, parsed source, and indexed semantic facts agree.

#### Scenario: One unresolved symbol has one import origin

- **WHEN** an unresolved-symbol diagnostic covers one symbol and the index yields exactly one valid import origin
- **THEN** the server offers an auto-import action for that origin
- **AND** the edit follows the same import preservation and de-duplication rules as completion resolve

#### Scenario: An import is diagnosed as unused

- **WHEN** the current diagnostics identify one parsed import declaration as unused
- **THEN** the server offers removal of exactly that declaration
- **AND** comments, adjacent imports, aliases, and newline style remain unchanged

#### Scenario: A match is diagnostically known to miss enum variants

- **WHEN** a non-exhaustive-match diagnostic identifies a match and the complete enum definition and missing-variant set are indexed
- **THEN** the server offers insertion of the missing arms
- **AND** existing arms and surrounding expressions remain unchanged

#### Scenario: Required safety evidence is incomplete

- **WHEN** the revision is stale, an import is ambiguous, parsing recovered, a wildcard obscures usage, the enum is incomplete, or the required diagnostic is absent
- **THEN** the corresponding Code Action is not offered

### Requirement: Smart completion SHALL meet documented quality gates

The change SHALL ship protocol golden tests, syntax-owner fixtures,
documentation, compatibility guidance, and a repeatable warm-completion
performance gate.

#### Scenario: The change is prepared for archive

- **WHEN** implementation tasks are reported complete
- **THEN** `sglsp`, relevant compiler, and `sgc` fixture tests pass
- **AND** formatting and warnings-denied lint gates pass for touched crates
- **AND** warm local completion p95 is below 80 ms on the checked-in representative workspace
- **AND** editor setup, capability compatibility, architecture, and attribute claims agree with executable behavior

#### Scenario: Sencoder runs the real cross-repository protocol E2E

- **WHEN** the integration test is launched with an absolute `SGLSP_PATH`
- **THEN** the harness starts exactly that executable and never falls back to `PATH`
- **AND** it records the executable/build identity and schema-v1 initialize capability
- **AND** real protocol assertions cover context completion, attributes, resolve/auto-import, signature help, and guarded Code Actions
- **AND** a missing or unexecutable `SGLSP_PATH` fails the test explicitly
