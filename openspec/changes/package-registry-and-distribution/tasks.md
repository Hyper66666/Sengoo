## 1. Registry protocol and reference server

- [x] 1.1 Specify the HTTP registry API (publish, yank, versions, metadata,
  download) with checksums and name reservation in `docs/`.
  - `docs/registry-protocol.md` freezes the v1 routes, auth/ownership model,
    status codes, upload limit, lockfile contract, and cache verification.
- [x] 1.2 Implement a reference registry server.
  - `sgpm registry serve` provides filesystem-backed publish, index, metadata,
    download, yank/unyank, immutable versions, and hashed owner reservations.
- [x] 1.3 Make `sgpm publish` upload real artifacts to the reference server
  (replace dry-run for the registry path).
  - The existing remote publish client is covered against the reference server
    rather than only a request-capture stub.

## 2. Registry-backed resolution

- [x] 2.1 Semver version requirements in `Sengoo.toml` for registry deps.
- [x] 2.2 Deterministic resolver producing hash-locked lockfile entries.
  - Lock schema v2 records `source.checksum`; remote caches verify both the
    archive checksum marker and a deterministic extracted-tree hash.
- [x] 2.3 e2e test: publish to the reference server, resolve, build, and run a
  consumer package.
  - `reference_registry_serves_publish_download_and_name_reservation` also
    tampers with the extracted cache and proves a locked run repairs it.

## 3. Binary distribution

- [ ] 3.1 Versioned release artifacts for Linux x86_64 and Windows x64.
- [ ] 3.2 macOS channel (arm64 + x86_64) artifacts.
- [ ] 3.3 One-command installer / version manager.
- [ ] 3.4 Release smoke matrix per platform (install -> `sgc run hello`).

## 4. Release process and docs

- [ ] 4.1 Toolchain semantic versioning + checksummed/signed artifacts.
- [ ] 4.2 Document install/upgrade and the registry workflow.
  - Registry workflow complete in `docs/registry-protocol.md` and
    `docs/sgpm-quickstart.md`; three-platform install/upgrade text remains open.
- [x] 4.3 Run `openspec validate package-registry-and-distribution --strict`.

## Verification

- Publish→resolve→build→run e2e against the reference registry (task 2.3)
- Per-platform install smoke (task 3.4), including macOS
- `cargo test -p sgpm`
