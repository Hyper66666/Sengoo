## Scope

This child change turns the existing package and release surfaces into a
mainstream-default release workflow. It is evidence-focused: where behavior
already exists, implementation work should add deterministic outputs, metadata,
tests, docs, and CI instead of redesigning the command.

## Existing Ownership To Preserve

- Archived `sgpm-alias-multiversion` and `ecosystem-toolchain-maturity` own
  dependency aliases, multiple versions, lockfile v2, metadata alias edges,
  registry yanked/features metadata, cross-compile docs, and LSP dependency
  definition support.
- Archived `toolchain-internal-ux` owns assertion transport, realworld e2e,
  debugger/editor/internal-release docs, and the first real-binary package loop.
- This change owns release-default polish on top of those capabilities.

## Package Artifact Contract

`sgpm publish --dry-run --locked --output <dir>` is the canonical local package
artifact flow. It must:

- validate the selected manifest and current lockfile before packaging;
- include source, `Sengoo.toml`, package docs such as `README*`/`LICENSE*` when
  present, and files required by `[bin]`/`[lib]`;
- exclude build products, VCS metadata, publish output directories, registry
  cache/staging directories, and host temp files;
- create `<name>-<version>.tar.gz` and `<name>-<version>.tar.gz.sha256`;
- produce deterministic archive paths, file ordering, normalized permissions,
  and stable gzip metadata for the same package content;
- optionally emit schema-versioned JSON metadata for CI.

The first JSON schema should include package name/version, selected manifest,
archive path, checksum path, sha256, included file count, excluded file count,
lockfile path/status, registry name when applicable, and workspace package when
selected.

## Registry Publish Contract

`sgpm publish --registry <name> --locked` must be safe for local and remote
registries:

- local file registry publish uses a staging directory and atomic finalization;
- duplicate package versions fail before overwriting existing content;
- remote registry publish uses `[registries.<name>].url` and optional
  `token_env`, never logs bearer tokens, and surfaces HTTP/status/checksum
  failures with stable diagnostics;
- index/checksum data written or returned by publish is consumable by
  `sgpm update`, `sgpm metadata --format json --locked`, and cache refresh/repair
  paths;
- failed publish attempts clean staging files when possible and leave no
  partially selected package as resolvable.

## Release Fixture

Add a committed `examples/realworld/package-release-loop` fixture or extend an
equivalent realworld package so the locked loop proves package release behavior
through public commands:

```text
sgpm update
sgpm metadata --format json --locked
sgpm publish --dry-run --locked --output target/package
sgpm publish --registry local --locked
sgpm update --check --locked
sgpm check --locked
sgpm test --locked
sgpm fmt --check --locked
sgpm doc --locked
sgpm build --locked
```

The fixture must exercise at least one dependency alias, one graph containing
two versions of the same package name, and a local registry dependency. Remote
registry publish can remain integration-test-only using a local test server.

## Toolchain Release Contract

Internal release readiness requires:

- `cargo build --release -p sgc -p sgpm -p sgfmt -p sglsp` on release hosts;
- archive manifest listing tool versions, git SHA, host triple, bundled stdlib
  runtime sources, and sha256 checksums;
- CI or documented manual smoke for realworld locked loops and `sglsp`
  realworld diagnostics on Windows and Ubuntu reference hosts;
- rollback instructions that verify lockfile compatibility with
  `sgpm update --check`, then run the locked project loop.

No hosted updater is required in this change.

## Compatibility

Existing manifests, lockfiles, registry dependencies, and `sgpm publish` command
names remain source/CLI compatible. Any new JSON output is additive and must use
a schema version. Human text may change only where tests assert stable substrings
for diagnostics.

