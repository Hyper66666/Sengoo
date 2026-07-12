## 1. Registry protocol and reference server

- [x] 1.1 Specify the HTTP registry API (publish, yank, versions, metadata,
  download) with checksums and name reservation in `docs/`.
- [ ] 1.2 Implement a reference registry server.
- [ ] 1.3 Make `sgpm publish` upload real artifacts to the reference server
  (replace dry-run for the registry path).
  - Partial: remote publish request construction, token headers, package
    archive/checksum output, and mock-server integration tests exist. Closure
    requires protocol conformance against task 1.2, name ownership, yank auth,
    duplicate version, and replay/error behavior.

## 2. Registry-backed resolution

- [x] 2.1 Semver version requirements in `Sengoo.toml` for registry deps.
  - Evidence: manifest/resolver unit and integration tests cover local and
    remote registries, highest compatible selection, aliases, and multiple
    selected versions.
- [ ] 2.2 Deterministic resolver producing hash-locked lockfile entries.
  - Partial: lockfile v2 records package ids and alias edges; local/remote
    registry caches and checksums are implemented. Closure requires a reviewed
    deterministic ordering contract, locked/offline differential e2e, and
    corruption/yank behavior against the reference server.
- [ ] 2.3 e2e test: publish to the reference server, resolve, build, and run a
  consumer package.

## 3. Binary distribution

- [ ] 3.1 Versioned release artifacts for Linux x86_64 and Windows x64.
  - Partial: distribution workflows and dry-run/install transcripts exist, but
    no real release tag has published the artifacts.
- [ ] 3.2 macOS channel (arm64 + x86_64) artifacts.
- [ ] 3.3 One-command installer / version manager.
- [ ] 3.4 Release smoke matrix per platform (install -> `sgc run hello`).

## 4. Release process and docs

- [ ] 4.1 Toolchain semantic versioning + checksummed/signed artifacts.
- [ ] 4.2 Document install/upgrade and the registry workflow.
- [x] 4.3 Run `openspec validate package-registry-and-distribution --strict`.

## 5. Security and protocol conformance

- [ ] 5.1 Test authentication, first-publisher name reservation, owner-only
  publish/yank, token redaction, duplicate version, and immutable archive bytes.
- [ ] 5.2 Reject checksum mismatch, archive traversal/absolute/symlink paths,
  duplicate entries, excessive entry/byte counts, and incomplete cache staging.
- [ ] 5.3 Run one protocol-conformance suite against the reference server and
  both local and remote `sgpm` clients.

## Verification

- Publish→resolve→build→run e2e against the reference registry (task 2.3)
- Per-platform install smoke (task 3.4), including macOS
- `cargo test -p sgpm`
