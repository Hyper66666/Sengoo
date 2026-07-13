## Why

`sgpm` has advanced beyond the original baseline: it now contains semver
registry requirements, local/remote registry configuration, aliases,
multiversion lockfile data, remote publish/download/cache paths, and checksum
verification tests. The remaining gap is product closure: a protocol-conformant
reference server, authenticated ownership/yank behavior, full publish-to-run
e2e, malicious archive defenses, real tagged releases, and macOS install
evidence. The task list must be reconciled with code before more resolver logic
is added.

## Proposal

- **Registry protocol**: a documented HTTP registry API for publish, yank,
  version listing, metadata, and download, with package name reservation and
  checksum verification. Provide a **reference registry server** and make
  `sgpm publish` / dependency resolution work against it (not just dry-run).
- **Dependency resolution from a registry**: semver version requirements,
  registry-backed lockfile entries with content hashes, and a deterministic
  resolver (alongside existing path/git deps).
- **Binary distribution**: real, versioned toolchain release artifacts for
  Linux x86_64 and Windows x64, plus a **macOS channel** (arm64/x86_64), with a
  one-command installer/version-manager and a smoke matrix.
- **Release process**: semantic versioning of the toolchain, checksummed
  artifacts with GitHub build-provenance attestations, and documented upgrade.

## What changes

- ADDED: registry protocol spec + reference server + real `sgpm publish`/install.
- ADDED: semver registry resolution with hash-locked entries.
- ADDED: versioned binary releases for Linux/Windows/macOS + installer.
- MODIFIED: `sgpm` publish/resolve paths move from dry-run to real against the
  reference registry (existing path/git flows unchanged).

## Non-goals

- Operating a production hosted registry service (this delivers the protocol +
  reference server; hosting/ops is a separate concern).
- A package web UI / search frontend.
- Treating mock HTTP upload/download tests as proof that the reference server or
  a production-hosted registry exists.
