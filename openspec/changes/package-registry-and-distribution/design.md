## Context

The codebase is ahead of the original proposal: manifests and resolver paths
already include semver requirements, local/remote registry configuration,
aliases, multiversion lockfile v2 data, remote publish/download/cache behavior,
and checksum paths. The missing product closure is a protocol-conformant
reference server, end-to-end ownership/auth/security behavior, released
artifacts, macOS evidence, and a real version tag.

## Decisions

### Decision 1: Reconcile existing implementation before extending it

Each task records current code/tests and the remaining acceptance gap. Existing
resolver or publish logic is not rewritten merely because tasks are stale.

### Decision 2: Registry protocol separates immutable content from mutable metadata

Package version archives are immutable and content-addressed/checksummed.
Yank state and package metadata are mutable, authenticated records. Republishing
an existing name/version with different bytes is rejected.

### Decision 3: Name ownership and tokens are part of the reference server

The reference server supports authenticated publish/yank, first-publisher name
reservation, owner management sufficient for tests, and audit-friendly errors.
Tokens are supplied through configured environment variables and never written
to lockfiles or normal logs.

### Decision 4: Resolution is deterministic and lockfile-first

Unlocked resolution selects the highest compatible non-yanked version using a
documented deterministic order. Locked operations use package id, source,
version, checksum, and dependency alias edges exactly. Offline mode performs no
network request and succeeds only from verified cache contents.

### Decision 5: Archive extraction is hostile-input code

Downloads enforce checksum, compressed/uncompressed size, entry count, path
traversal, absolute path, symlink, duplicate path, and manifest identity limits
before cache publication. Failed staging is removed atomically.

### Decision 6: Release artifacts share one version source

`sgc`, `sgpm`, `sgfmt`, `sglsp`, runtime metadata, archive manifest, installer,
and Git tag report one semver. Archives are checksummed and signed. Install and
upgrade smoke run outside the checkout on Windows, Linux, and macOS x64/arm64.

## Reference-server scope

Required endpoints cover publish, versions/metadata, archive download, yank/
unyank, and ownership checks. A web UI, search ranking, billing, and production
operations are outside this change.

## Archive gate

- protocol conformance tests against the reference server;
- publish -> resolve -> lock -> offline rebuild -> run e2e;
- alias and multiversion consumer e2e;
- malicious archive and auth/name-ownership negatives;
- one real prerelease tag and supported-host install/upgrade matrix.
