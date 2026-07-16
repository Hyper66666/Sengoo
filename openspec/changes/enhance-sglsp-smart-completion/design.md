## Context

The current server stores open documents as unversioned text, recursively reads
all `.sg` files when workspace information is requested, reparses every
document into top-level symbols, and merges those symbols with imported stdlib
metadata. Completion then appends keywords and arbitrary identifier-shaped
words. `resolveProvider` is false and completion triggers are only `.` and `:`.

This change must improve that path without changing Sengoo syntax. The compiler
parser and formatter are authoritative for imports. The compiler surface and
derive expanders, FFI collector, and `sgc test` discovery are authoritative for
attributes. Documentation describes those behaviors but cannot create a
capability on its own.

## Decisions

### 1. Maintain a snapshot-based WorkspaceIndex

`WorkspaceIndex` owns immutable query snapshots containing document revisions,
module/import facts, scopes, declarations, members, signatures, source
locations, documentation, and symbol origins. It indexes workspace roots,
resolved direct dependency roots, and stdlib metadata once during startup.

Open documents are versioned overlays over disk state. `didOpen` and
`didChange` update only that document; `didSave`, `didClose`, and watched-file
create/change/delete events refresh or remove the affected entry. A close
restores the current disk entry. Re-index work is computed outside the write
lock and an atomic snapshot swap keeps requests non-blocking. Events carrying
an older document version cannot replace a newer overlay.

The existing directory exclusions remain in force. Index failures are logged
per file and retain the last valid entry when safe; they do not invalidate the
whole workspace. A bounded initial-index progress notification is allowed, but
completion must remain available from open documents while indexing continues.

### 2. Use tolerant cursor classification, then semantic filtering

`CompletionContext` is a closed internal enum:

- `General`
- `Member { receiver }` after `.`
- `Namespace { path }` after `::`
- `ImportPath { form, path }`
- `Attribute { target, nesting }` after `#`, `#[`, or inside an attribute

Classification uses tokens and a small delimiter-aware recovery layer around
the cursor so incomplete code remains useful. It must not require the entire
document to parse. Comments, strings, numeric literals, and invalid delimiter
regions return no context rather than leaking global candidates.

Semantic filtering is authoritative after classification: member completion
uses the resolved receiver type; namespace completion uses the resolved module,
type, or associated namespace; import completion returns modules and symbols
valid for the detected import form; attribute completion filters by declaration
target. If resolution is incomplete, the server may return a clearly bounded
fallback for the same context, but it may not fall back to all workspace words.

### 3. Follow the real import grammar

Import recognition and tests are derived from compiler/parser plus `sgfmt`
fixtures for the accepted forms: simple path, `as` alias, selective braces,
and wildcard/`from`. The language server must not implement a separate guessed
grammar.

Auto-import inserts the smallest stable import that makes the selected symbol
available, normally a simple module import. It preserves the formatter's import
grouping and newline style, sorts within the existing import block, and avoids
duplicates and alias conflicts. It never edits comments or strings. If several
exports with the same name are viable, completion exposes their origins as
separate candidates and does not guess one during resolve.

### 4. Make the attribute catalog evidence-backed

`AttributeCapability` records the spelling, insertion template, valid target
kinds, parameter form, documentation, owner (`compiler`, `derive`, `ffi`, or
`sgc-test`), and a fixture identifier. The initial catalog includes only
attributes demonstrably accepted by those owners, such as supported surface
attributes, supported derives, FFI/link attributes, and test/case attributes.

Catalog tests compile or run the owning reduced fixture and reject entries that
are documentation-only or unsupported for the declared target. `#` and `#[`
show attribute names; nested positions such as `derive(...)` show only values
proven by the derive implementation. Unsupported or externally configured
derive macro names are not advertised as built-ins.

### 5. Make server ordering and edits authoritative

Every completion item receives a stable category and `sortText`. Category order
is local variable, parameter, field, imported symbol, project symbol, standard
library, then keyword. Prefix quality and stable lexical/origin tie-breakers
apply inside a category. Duplicate labels remain separate only when origin or
required import differs and their detail makes that distinction visible.

All insertions use UTF-16 `TextEdit` replacement ranges calculated from the
current document revision. Snippet insertion is limited to syntax where a
snippet materially represents required structure; symbol candidates insert
identifiers or calls without unexpectedly consuming following tokens.

The initialize response advertises experimental capability
`sengoo.completionSchemaVersion: 1`. Each item carries JSON `data` with:

```text
schemaVersion, symbolId, origin, category, documentUri,
documentRevision, resolveKind
```

`resolveKind` is `none`, `documentation`, `autoImport`, or
`documentationAndAutoImport`. `documentUri` is the canonical serialized URI of
the text document for which the item was produced. `documentRevision` is the
exact integer LSP document version from the server's open-document overlay, not
an index timestamp or content hash; edit-producing completion is unavailable
when no current versioned overlay exists.

Fields may be added within schema v1. Schema-v1 consumers must ignore unknown
fields, and the server's resolve decoder must tolerate and preserve them while
reading the required identity fields. Removing a required field, changing a
field's meaning/type, or changing URI/revision identity semantics requires a
new schema version.

### 6. Resolve lazily and reject stale edits

The server advertises `resolveProvider: true`. The initial response contains
enough label, kind, detail, ordering, replacement range, and data for display.
Resolve looks up `symbolId` in the current index and supplies markdown
documentation and, where needed, `additionalTextEdits` for auto-import.

If URI, revision, symbol, or origin no longer matches, resolve returns the item
without edit-producing fields. It must never adapt an import edit silently to a
new document version. Clients may request completion again to obtain a fresh
item.

### 7. Offer only diagnostic- and index-proven Code Actions

The first Code Action set is intentionally smaller than general completion:

- unresolved-symbol auto-import is offered only when the diagnostic range maps
  to one symbol and the index yields one unambiguous import origin;
- unused-import removal is offered only for a compiler/`sglsp` unused-import
  diagnostic and removes the exact parsed import declaration without touching
  comments or neighboring imports;
- missing enum match arms are offered only when the non-exhaustive-match
  diagnostic identifies the match, the enum definition is fully indexed, and
  the complete missing-variant set is known.

Every action is computed against the current document revision and carries the
triggering diagnostic. Ambiguity, stale revisions, incomplete enum information,
wildcard uncertainty, or parse recovery suppresses the action. These actions
never rewrite unrelated source and are independently testable as `WorkspaceEdit`
results.

### 8. Improve signature selection without changing call semantics

Signature help uses delimiter-aware nested-call recovery and the index's
function/method signatures. It resolves receiver methods, preserves overloads,
selects the best active signature from known argument types/count, clamps the
active parameter safely, and includes callable and parameter documentation.
Unresolved calls return no signature rather than an unrelated name match.

### 9. Make cross-repository E2E select the server explicitly

The Sengoo verification lane builds one known `sglsp` executable and exposes
its absolute path to the Sencoder protocol harness as `SGLSP_PATH`. The harness
must launch exactly that file, fail if the variable is missing/unexecutable,
and must not fall back to `PATH`. Its transcript records the executable path,
server version/build identity, initialize capabilities, and schema version.

The real E2E opens a Sengoo fixture and exercises context completion, `#`
attributes, resolve plus auto-import, signature help, and the safe Code Action
guards. This is a coordination/acceptance contract: this change supplies the
server behavior and fixture; the Sencoder change owns its client harness.

## Performance and failure behavior

- Warm local completion p95 is below 80 ms on the checked-in representative
  workspace fixture, measured separately from initial indexing.
- A single document edit reparses/reindexes that document, not the workspace.
- Cancellation is checked before parsing, candidate expansion, and resolve IO.
- Invalid UTF-8 disk files, broken syntax, deleted dependencies, and index
  refresh failures degrade to bounded results and logs without crashing the
  server.

## Compatibility and rollout

Standard LSP fields remain valid for clients unaware of the experimental
capability. Sencoder detects schema v1 before disabling its legacy completion
middleware; no server setting is required for old clients. The new index can be
landed behind internal parity tests, but schema v1 is advertised only after
ordering, UTF-16 edits, resolve, additive-field compatibility, and
stale-revision tests pass.

## Documentation record

Implementation updates `docs/editor-setup.md`, adds a single LSP
capability/compatibility reference, and records this design under
`docs/architecture/`. Those documents name compiler/tool fixtures as syntax
truth, list the schema-v1 fields, describe client fallback, and reconcile any
existing contradictory attribute claims. They also document the fixed
`SGLSP_PATH` cross-repository E2E contract.
