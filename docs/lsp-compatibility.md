# sglsp capability and compatibility reference

## Completion metadata schema v1

`initialize` advertises:

```json
{ "experimental": { "sengoo": { "completionSchemaVersion": 1 } } }
```

Schema-aware clients may decode `CompletionItem.data` with these required
fields:

| Field | Contract |
| --- | --- |
| `schemaVersion` | Integer `1` |
| `symbolId` | Stable symbol identity within the indexed origin |
| `origin` | `currentDocument`, `workspace`, `dependency`, or `standardLibrary` |
| `category` | Completion ordering category |
| `documentUri` | Canonical serialized URI used for the request |
| `documentRevision` | Exact integer LSP version of the open document |
| `resolveKind` | `none`, `documentation`, `autoImport`, or `documentationAndAutoImport` |

Fields may be added within schema v1. Consumers must ignore unknown fields;
the server decoder preserves them during a decode/encode round trip. Removing
a required field, changing its type or meaning, or changing URI/revision
identity requires a new schema version. Standard clients may ignore both the
experimental capability and `data`; labels, kinds, details, and ordinary LSP
responses remain usable.

The Sencoder Sengoo extension negotiates this capability explicitly. When the
capability is absent it retains the legacy client middleware for context
filtering, ranking, and replacement ranges. When version `1` is present the
server is authoritative: the client preserves item order, UTF-16 edits,
snippets, resolve data, and auto-import edits without a second ranking pass.
Attribute completion after `#` / `#[` is server-only in both modes, preventing
client/server duplication. Unknown future schema versions are not treated as
schema v1 and therefore cannot silently enable v1-only edit behavior.

Items produced without a versioned open-document overlay omit schema data
rather than inventing a revision. Completion resolve supplies Markdown
documentation and, for a unique unimported project origin, a stable
`additionalTextEdits` import. Resolve requires the exact schema-v1 URI,
revision, symbol ID, and origin; stale items never receive edit-producing
metadata. Existing simple, alias, selective, or wildcard imports are
de-duplicated, name conflicts suppress edits, and ambiguous origins remain
separate visible candidates.

Workspace and dependency paths come from a `ModuleIdentity`, never a file
stem. The canonical package root supplies `[package].name` from
`Sengoo.toml` (`-` becomes `_`), and files below `src/` append their relative
path segments. `src/lib.sg` is the package root; for example,
`packages/sggame/src/snake_logic.sg` is `sggame::snake_logic`. Selective
imports use this same identity for workspace, dependency, and `std::` exports.
Dependency edges may expose a different alias; the root manifest's
`[dependencies]` path mapping wins over the dependency package name. Manifest
strings accept legal single or double quotes and ignore trailing line comments.

The standard completion provider advertises `resolveProvider: true` and the
`.`, `:`, and `#` trigger characters. Clients that ignore experimental schema
metadata still receive normal labels, kinds, `sortText`, snippets, and UTF-16
`textEdit` ranges.

## Cancellation and document versions

The index rejects changes whose integer document version is not newer than the
current overlay. Parsing is performed before snapshot publication, and a stale
result cannot replace a newer overlay. Initialization and document refresh use
a drop-cancel guard around `spawn_blocking`: dropping the LSP handler future
sets the same token checked at scan, read, parse, and pre-publication
boundaries. Starting a newer full-index generation cancels the older build, and
only the current generation may swap a snapshot. Invalid incomplete overlays
keep the last good semantic entry while preserving the new text and revision.

Each URI has independent disk and overlay epochs. Open/change captures the
overlay epoch plus LSP revision; save/refresh/failure captures only the disk
epoch. After lock-free read/parse and a cancellation recheck, publication
compare-and-swaps its owned epoch while merging into the latest snapshot.
Consequently, a disk save/refresh cannot invalidate or replace an in-flight
higher-revision overlay change, and an overlay change cannot discard a valid
disk refresh. Results for different URIs also merge independently. Full
workspace rebuilds retain their separate global build-generation guard.

Create/change refresh failures (including invalid UTF-8, missing files, and
permission/read errors) are recorded per URI and retain the last-good disk
entry. A successful refresh clears the failure. Only an explicit LSP `Deleted`
event removes an indexed file.
Explicit deletion also clears a failure-only entry that never produced a valid
disk document. Delete advances the disk epoch and preserves an open overlay;
close advances the overlay epoch and restores the newest disk entry, or removes
the effective document if the disk entry was deleted.

Project completion identity is based on the canonical definition URI,
container, an explicit protocol kind spelling, name, and normalized signature
or semantic detail. A deterministic ordinal is appended only for otherwise
identical duplicates. This identity is stable across whitespace and positional
line changes; changing the definition URI, container, kind, name, or normalized
signature/detail intentionally changes it. It never uses the requesting URI for
a symbol defined in another file. Standard-library symbols and signatures are
cached in the document index instead of being reparsed during warm completion,
hover, or signature queries.

## Import syntax authority

The compiler parser and `sgfmt` are authoritative. The accepted forms covered
by shared fixtures are:

```sengoo
import std::io;
import std::collections as coll;
import toolkit { alpha, beta };
import legacy * from;
```

IDE implementations must not substitute the older reverse-order forms found
in historical comments or documentation.

## Attribute capability provenance

Every advertised attribute or nested value carries an `evidence:` identifier
in its detail. `tools/sglsp/attribute-evidence.json` maps every distinct ID
one-to-one to a package, Cargo filter, and exact test name. The cross-platform
gate is `cargo run -p sglsp --bin attribute-evidence-verifier --
tools/sglsp/attribute-evidence.json`; it executes each owner test and rejects
missing, ignored, or failed evidence. The PowerShell entry is only a wrapper,
and the sglsp unit test rejects catalog/manifest drift.
Built-in derive values are exactly `Clone`, `Copy`, `PartialEq`, `Eq`,
`PartialOrd`, `Ord`, `Hash`, `Debug`, and `Default`. External derive commands
are intentionally not presented as built-ins.
