## 1. Baseline and protocol contract

- [x] 1.1 Add golden protocol tests that lock current completion, signature,
  UTF-16 position, workspace/dependency, stdlib, and broken-syntax behavior
  before refactoring.
- [x] 1.2 Define versioned document state, symbol origin/category identifiers,
  `CompletionContext`, `SengooCompletionDataV1`, and the experimental initialize
  capability, including canonical `documentUri`, exact integer
  `documentRevision`, and additive unknown-field compatibility tests.
- [x] 1.3 Add compiler/`sgfmt` fixtures for every accepted import form used by
  completion tests; remove IDE assumptions not accepted by those fixtures.

## 2. Incremental WorkspaceIndex

- [x] 2.1 Introduce a snapshot-based `WorkspaceIndex` for workspace roots,
  resolved dependency roots, stdlib metadata, open-document overlays, symbols,
  scopes, members, signatures, documentation, and import facts.
- [x] 2.2 Build the initial index once and add per-document updates for open,
  incremental change, save, close, and watched-file create/change/delete.
- [x] 2.3 Protect index publication against stale document versions, parse work
  outside the write lock, retain bounded last-good entries, and honor request
  cancellation.
- [x] 2.4 Prove with instrumentation tests that warm completion and a one-file
  edit do not recursively rescan or reparse the full workspace.

## 3. Context-aware completion

- [x] 3.1 Implement delimiter-aware tolerant classification for general,
  member, namespace, import-path, and attribute contexts, including comments,
  strings, incomplete code, and invalid-syntax recovery.
- [x] 3.2 Add semantic candidates for locals, parameters, fields,
  receiver-compatible members, namespaces/associated items, imported symbols,
  project symbols, stdlib, and keywords.
- [x] 3.3 Implement the fixed category ordering and deterministic prefix/origin
  tie-breakers; remove arbitrary document-word candidates and context leaks.
- [x] 3.4 Return correct UTF-16 `textEdit` replacement ranges and appropriate
  snippets without consuming adjacent tokens.

## 4. Imports, attributes, and resolve

- [x] 4.1 Complete import-path candidates for simple, alias, selective, and
  wildcard forms using compiler/formatter-backed fixtures.
- [x] 4.2 Create the evidence-backed attribute capability catalog and fixtures
  for compiler surface attributes, built-in derives, FFI/link, and `sgc test`
  attributes; add `#` to server trigger characters.
- [x] 4.3 Filter attribute names and nested values by declaration target and
  owner capability, and fail tests for catalog entries without executable
  evidence.
- [x] 4.4 Enable completion resolve and lazily add markdown documentation and
  safe auto-import `additionalTextEdits`.
- [x] 4.5 Implement stable import-block insertion, sorting, de-duplication,
  alias/conflict handling, ambiguous-origin display, and stale-revision refusal.

## 5. Signature help

- [x] 5.1 Replace name-only call detection with nested delimiter-aware call and
  receiver resolution.
- [x] 5.2 Return all viable signatures with callable/parameter documentation,
  deterministic active-signature selection, and safely clamped active
  parameters.
- [x] 5.3 Add tests for nested calls, receiver methods, overloads, commas inside
  nested expressions, incomplete calls, Unicode source, and unresolved calls.

## 6. Safe Code Actions

- [x] 6.1 Add unresolved-symbol auto-import only when the triggering diagnostic,
  range, symbol, and unique import origin agree on the current revision.
- [x] 6.2 Add exact unused-import removal only for the corresponding diagnostic,
  preserving comments, neighboring imports, and source newline style.
- [x] 6.3 Add missing enum match arms only when the non-exhaustive diagnostic,
  fully indexed enum, and complete missing-variant set agree.
- [x] 6.4 Add negative tests for ambiguous imports, stale revisions, parse
  recovery, wildcard imports, incomplete enums, missing diagnostics, comments,
  and Unicode edit ranges.

## 7. Documentation and verification gates

- [x] 7.1 Update `docs/editor-setup.md` with context completion, `#` attributes,
  resolve/auto-import, safe Code Actions, signature help, and troubleshooting behavior.
- [x] 7.2 Add the LSP capability/compatibility reference documenting
  `sengoo.completionSchemaVersion = 1`, every data field, old-client fallback,
  additive-field handling, cancellation, and versioning rules.
- [x] 7.3 Add an architecture record for WorkspaceIndex, syntax authority, and
  attribute-catalog provenance; reconcile contradictory language/attribute
  documentation in the same implementation change.
- [x] 7.4 Run focused protocol/golden tests, `cargo test -p sglsp`, relevant
  compiler and `sgc test` fixture tests, `cargo fmt --check`, and clippy for all
  touched Rust crates with warnings denied.
- [x] 7.5 Measure warm local completion p95 below 80 ms on a checked-in
  representative workspace and retain a regression threshold in CI.
- [x] 7.6 Build one known `sglsp` binary and run the Sencoder real-protocol E2E
  with its absolute path fixed in `SGLSP_PATH`; fail on missing/unexecutable
  paths or any fallback to `PATH`, and record path, build identity, initialize
  capabilities, and schema version in the transcript.
- [x] 7.7 In that E2E, verify context completion, `#` attributes, resolve with
  auto-import, signature help, and positive/negative safe Code Action cases
  against the shared Sengoo fixture.
- [ ] 7.8 Run strict OpenSpec validation when the CLI is available and record
  the exact verification evidence before archiving the change.
