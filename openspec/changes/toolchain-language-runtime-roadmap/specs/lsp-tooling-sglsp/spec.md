## ADDED Requirements

### Requirement: sglsp SHALL implement core interactive language features
The `sglsp` server SHALL provide completion, go-to-definition, and hover responses compatible with standard LSP clients.

#### Scenario: Completion request returns candidates
- **WHEN** an editor sends `textDocument/completion` for a valid Sengoo source location
- **THEN** `sglsp` returns completion items relevant to in-scope symbols

#### Scenario: Definition request resolves symbol location
- **WHEN** an editor sends `textDocument/definition` for a resolvable symbol
- **THEN** `sglsp` returns the source location where that symbol is defined

#### Scenario: Hover request returns type or symbol information
- **WHEN** an editor sends `textDocument/hover` on a known symbol
- **THEN** `sglsp` returns hover content with actionable semantic information

### Requirement: sglsp SHALL support incremental document synchronization
The language server SHALL process incremental text updates so diagnostics and language features stay consistent without full-document resend.

#### Scenario: Incremental edit updates diagnostics
- **WHEN** a client sends `textDocument/didChange` with incremental ranges
- **THEN** `sglsp` updates its document state and publishes diagnostics matching the new content

### Requirement: sglsp SHALL consume sgc JSON diagnostics
`sglsp` SHALL integrate compiler diagnostics by consuming `sgc --error-format json` output and mapping it to LSP diagnostics.

#### Scenario: Compiler error appears in editor diagnostics
- **WHEN** `sgc --error-format json` emits an error for an open file
- **THEN** `sglsp` publishes an equivalent LSP diagnostic at the correct file range
