# Daily-development candidate hardening

## Why

The Windows development implementation has local installation, ownership,
package and wheel evidence, but no fixed candidate revision or executed remote
matrix. Composition regressions and real application workflows need continuing
coverage before treating the toolchain as ready for everyday work.

## What Changes

- Add deterministic generated ownership combinations and fix reproduced cleanup defects.
- Extend the CSV index with bounded, atomic incremental updates and usable errors.
- Exercise realistic Python batch work, conversion costs and resource stability.
- Fix candidate CI and collect exact-revision results without weakening stable release gates.
- Prepare a verifiable participant kit and actual observation records for an independent pilot.

## Impact

Keeps edition 2026, runtime ABI 1 and compatible existing manifests/lockfiles.
The existing M0-M5 changes own their original deliverables. This change owns
the additional stress, incremental-use and candidate qualification requirements.
Public release and merge are separate from collecting reviewable evidence.
