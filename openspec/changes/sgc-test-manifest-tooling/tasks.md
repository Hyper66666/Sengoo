## 1. Baseline Inventory

- [ ] 1.1 Validate this change with `openspec validate sgc-test-manifest-tooling --strict`.
- [ ] 1.2 Inventory existing sgpm manifest, lockfile, resolver, registry, cache, scaffold, workspace, and test behaviors.
- [ ] 1.3 Inventory existing `sgfmt`, `sgc doc`, `sgc bench`, and `sglsp` command/feature surfaces.
- [ ] 1.4 Record which behaviors are already implemented, partially implemented, or missing.

## 2. Manifest, Lockfile, Registry, And Cache Stabilization

- [ ] 2.1 Add schema/version diagnostics for `Sengoo.toml` and `Sengoo.lock`.
- [ ] 2.2 Stabilize lockfile source ids for path, git, local registry, and remote registry sources.
- [ ] 2.3 Stabilize registry package metadata, download/upload path, checksum, and cache layout.
- [ ] 2.4 Add stale-lock, missing-cache, corrupted-cache, incompatible-version, and conflict diagnostics.
- [ ] 2.5 Update `docs/sgpm-quickstart.md` with protocol guarantees and compatibility notes.

## 3. Direct sgc test

- [ ] 3.1 Add `sgc test` CLI parsing for the accepted shape in `design.md`.
- [ ] 3.2 Implement test discovery for `tests/**/*.sg`, manifest-declared test targets, filters, exact names, and no-test behavior.
- [ ] 3.3 Capture stdout/stderr by default while supporting `--nocapture`.
- [ ] 3.4 Report pass/fail counts, exit status, per-test duration where stable, and failing command details.
- [ ] 3.5 Add `--format json` with a schema-tested output shape.
- [ ] 3.6 Ensure tests run shell-free through the existing native execution policy.

## 4. sgpm Alignment

- [ ] 4.1 Update `sgpm test` to delegate to `sgc test` or produce equivalent behavior until delegation is possible.
- [ ] 4.2 Preserve existing `sgpm test --release`, library module-map, workspace, and locked-mode behavior.
- [ ] 4.3 Add integration tests proving sgpm and sgc test discovery/reporting agree for package fixtures.

## 5. Formatter, Docs, LSP, Bench, Templates

- [ ] 5.1 Harden `sgfmt` idempotence, check mode, config loading, and unreadable-file diagnostics.
- [ ] 5.2 Harden `sgc doc` public symbol extraction, module comments, examples, and deterministic output paths.
- [ ] 5.3 Harden `sglsp` completion, hover, definition, diagnostics, code actions, formatting, and workspace symbols against real examples.
- [ ] 5.4 Harden `sgc bench` text/JSON output schemas and profile/RSS reporting.
- [ ] 5.5 Add or stabilize CLI, library, and service project templates with tests.

## 6. Verification

- [ ] 6.1 Run `cargo fmt --check`.
- [ ] 6.2 Run `cargo test -p sgc test -- --nocapture`.
- [ ] 6.3 Run `cargo test -p sgpm -- --nocapture`.
- [ ] 6.4 Run `cargo test -p sgfmt -- --nocapture`.
- [ ] 6.5 Run `cargo test -p sglsp -- --nocapture`.
- [ ] 6.6 Run `sgc test` on package and no-manifest fixtures.
- [ ] 6.7 Run `sgpm test`, `sgpm fmt --check`, `sgpm doc`, and `sgpm metadata --format json` on fixtures.

## Done Definition

- [ ] Existing manifest/lock/registry/cache behavior is documented and schema-tested.
- [ ] `sgc test` exists with discovery, filtering, capture, status reporting, and JSON output.
- [ ] `sgpm test` remains compatible and aligns with `sgc test`.
- [ ] `sgfmt`, `sgc doc`, `sglsp`, `sgc bench`, and templates have deterministic CI-ready tests.

## Archive Gate

- [ ] `openspec validate sgc-test-manifest-tooling --strict` passes.
- [ ] `openspec validate --all --strict` passes.
- [ ] All verification commands above pass or have documented, accepted platform skips.
