## ADDED Requirements

### Requirement: Editor language operations SHALL share one versioned workspace snapshot

Completion, hover, definition, references, rename, signature help, diagnostics,
and code actions SHALL consume one immutable indexed workspace snapshot and the
exact open-document revision used to answer the request.

#### Scenario: Document changes before an edit is applied

- **WHEN** completion resolve, rename, or a code action carries a stale document
  revision
- **THEN** the server refuses the edit with a bounded stale-revision result
- **AND** does not modify the current document

#### Scenario: One source file changes

- **WHEN** an indexed workspace receives a one-file edit
- **THEN** the server updates affected document/package entries
- **AND** does not recursively rescan or reparse the full workspace

### Requirement: Rename SHALL be identity-safe and package-aware

Rename SHALL update only references proven to resolve to one workspace-owned
symbol identity. Dependency and standard-library sources SHALL be read-only, and
ambiguous or incomplete identities SHALL fail without edits.

#### Scenario: Public workspace symbol is renamed

- **WHEN** a symbol and all references resolve uniquely within writable
  workspace packages
- **THEN** rename returns deterministic UTF-16 edits for declarations, imports,
  and references
- **AND** rechecking the package succeeds after applying them

#### Scenario: Symbol comes from a dependency

- **WHEN** rename targets a dependency or stdlib declaration
- **THEN** the server refuses the rename and identifies the read-only origin

### Requirement: Formatter output SHALL be parser-compatible and idempotent

`sgfmt` SHALL format every compiler-supported v0.2 syntax fixture without
changing its parse meaning, and formatting the result again SHALL be
byte-identical.

#### Scenario: A supported file is formatted twice

- **WHEN** `sgfmt` formats a supported source file and formats the result again
- **THEN** the second output equals the first byte-for-byte
- **AND** compiler check results are unchanged

### Requirement: Interactive tooling SHALL meet bounded warm-request targets

On the checked-in representative workspace, warm completion p95 SHALL be no
greater than 80 ms and warm hover/signature/definition p95 SHALL be no greater
than 100 ms under the documented reference-host measurement.

#### Scenario: A change exceeds the warm budget

- **WHEN** retained benchmark samples exceed a pinned threshold
- **THEN** CI fails
- **AND** changing the threshold requires reviewed measurement evidence

### Requirement: Installed tools SHALL be selected explicitly

Editor and cross-repository E2E tests SHALL use configured absolute paths for
`sglsp`, `sgfmt`, `sgc`, and `sgpm` and SHALL fail rather than silently falling
back to another executable on PATH.

#### Scenario: Configured server path is missing

- **WHEN** the selected `SGLSP_PATH` is missing or not executable
- **THEN** startup/E2E fails with the selected path in the diagnostic
- **AND** no PATH binary is launched
