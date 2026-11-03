## ADDED Requirements

### Requirement: The toolchain SHALL make daily editor and project operations reliable

The toolchain SHALL implement the following safe symbol operations and reproducible project workflows.

#### Scenario: Stage deliverables are reviewed

- **WHEN** this stage is considered for completion
- **THEN** the following implementation and evidence obligations are satisfied:

- Resolve lexical bindings and references in versioned AST workspace snapshots.
- Use the shared resolver for definitions, references, prepareRename and conflict-safe rename.
- Reject ambiguous, invalid, readonly and stale edit requests with no name-search fallback.
- Generate formatted bin/lib scaffolds with runnable tests and verify the fresh project loop.
- Implement text/JSON doctor diagnostics and native toolchain probes.
- Document actual editor running and native debugging workflows.

#### Scenario: Two functions have unrelated locals with the same name

- **WHEN** two functions have unrelated locals with the same name
- **THEN** renaming either binding edits only its declaration and bound uses.
- **AND** existing compatible v0.2 projects SHALL retain their documented behavior.

#### Scenario: A newer edit is received while parsing the previous version

- **WHEN** incremental changes arrive before an earlier parser job publishes
- **THEN** the server composes them against pending document text and refuses rename against the older published version.
- **AND** only the latest queued overlay token may publish; closing the document invalidates outstanding tokens before a reopened session restarts its versions.

#### Scenario: Trait method identities cannot be linked safely

- **WHEN** a rename targets a trait implementation member without a complete trait and dispatch identity
- **THEN** the server refuses the edit instead of renaming only the concrete implementation and breaking its trait contract.
