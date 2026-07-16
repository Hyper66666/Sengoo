## Why

`sglsp` already exposes completion, signature help, hover, navigation, and
diagnostics, but its completion path is still a workspace-wide symbol dump. It
rescans source trees while serving requests, adds words found in the document,
does not distinguish member, namespace, import, or attribute positions, and
cannot resolve documentation or auto-import edits lazily. This produces noisy
lists and makes editor behavior depend on client-side regular expressions.

Sengoo also has several syntax surfaces whose accepted form must come from the
compiler and tools rather than IDE assumptions. Import declarations have
simple, alias, selective, and wildcard forms; attributes are split across the
compiler's surface/derive expanders, FFI handling, and `sgc test` discovery.
The language server needs one tested compatibility contract for those surfaces.

## What changes

- Add an incrementally maintained `WorkspaceIndex` for workspace, dependency,
  standard-library, and open-document symbols.
- Classify incomplete source at the cursor into general, member, namespace,
  import-path, and attribute completion contexts.
- Return context-specific, deterministically ranked candidates with correct
  UTF-16 replacement ranges and a versioned `CompletionItem.data` payload.
- Advertise `#` as a trigger and generate attribute candidates from a
  capability catalog whose entries are proven by compiler or `sgc` fixtures.
- Enable completion resolve so documentation and safe auto-import edits are
  computed lazily and rejected when the document revision is stale.
- Add a deliberately small safe Code Action set: deterministic unresolved-symbol
  imports, diagnostic-confirmed unused-import removal, and missing enum match
  arms only when both enum and diagnostic information are complete.
- Improve signature help for receivers, overload selection, nested calls,
  documentation, and active parameters.
- Document the protocol, compatibility fallback, syntax sources of truth, and
  performance expectations, and support a real cross-repository protocol E2E
  that launches the selected server exclusively through `SGLSP_PATH`.

## Impact

- Primary implementation: `tools/sglsp`.
- Syntax authority and test fixtures: compiler parser/expanders, `sgfmt`, and
  `sgc test` discovery; production syntax is not redefined by this change.
- Documentation: editor setup, a versioned LSP capability/compatibility page,
  and an architecture decision record.
- Client impact: clients that understand `sengoo.completionSchemaVersion = 1`
  may trust server categories, ordering, edits, and resolve metadata; older
  clients continue to receive standard LSP completion items.
- Integration impact: Sencoder can pin an exact locally built `sglsp` binary in
  E2E instead of silently testing another binary found on `PATH`.

## Non-goals

- AI-generated inline completion or next-edit prediction.
- Postfix completion, inlay hints, call/type hierarchy, or broad refactoring.
- Replacing the compiler parser/type checker with an editor-only semantic
  engine.
- Inventing attributes or import forms not accepted by compiler/tool fixtures.
