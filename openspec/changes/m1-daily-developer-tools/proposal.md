## Why

Make daily editor and project operations reliable for native CLI and Python developers while preserving v0.2 compatibility.

## What Changes

- Resolve lexical bindings and references in versioned AST workspace snapshots.
- Use the shared resolver for definitions, references, prepareRename and conflict-safe rename.
- Reject ambiguous, invalid, readonly and stale edit requests with no name-search fallback.
- Generate formatted bin/lib scaffolds with runnable tests and verify the fresh project loop.
- Implement text/JSON doctor diagnostics and native toolchain probes.
- Document actual editor running and native debugging workflows.

## Capabilities

### New Capabilities

- `safe-editor-symbol-operations`: Make daily editor and project operations reliable.

## Impact

The approved native daily-development program; existing experimental backend scope is unchanged.
