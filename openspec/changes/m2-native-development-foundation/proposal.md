## Why

Verify ownership composition and native artifact builds for native CLI and Python developers while preserving v0.2 compatibility.

## What Changes

- Run owning aggregate/generic/borrow/control-flow/async cleanup composition tests and correct discovered defects.
- Add explicit shared-library builds whose selected exports are reachability roots.
- Link runtime and package sources with correct PIC and explicit export boundaries.
- Invalidate native object caches on source/transitive header/compiler/target/options/output-mode changes.
- Keep existing source/ABI/diagnostic compatibility and document rejected boundaries.

## Capabilities

### New Capabilities

- `native-artifact-builds`: Verify ownership composition and native artifact builds.

## Impact

The approved native daily-development program; existing experimental backend scope is unchanged.
