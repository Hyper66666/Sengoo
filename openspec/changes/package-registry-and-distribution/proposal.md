## Why

`sgpm` supports local-path/git deps, lockfiles, workspaces, and publish
*dry-run*, but there is no public package registry and no prebuilt binary
distribution. Toolchain archives exist for Windows/Linux as dry-run only, macOS
is deferred, and no real release tag has shipped. Mainstream languages have a
central registry (crates.io / PyPI / npm) and one-command installable toolchains.
Without this, third parties cannot share or consume packages and cannot install
Sengoo easily.

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
- **Release process**: semantic versioning of the toolchain, signed/checksummed
  artifacts, and documented upgrade.

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
