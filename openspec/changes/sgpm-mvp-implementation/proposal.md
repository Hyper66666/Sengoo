# Proposal: Implement sgpm Package Manager MVP

## Status

**Proposed** — `tools/sgpm/Cargo.toml` declares the full dependency stack but
`tools/sgpm/src/` is empty. The OpenSpec roadmap
`toolchain-language-runtime-roadmap` records tasks 1.8 / 1.9 / 1.10 as
completed, which conflicts with repository state. This change closes the gap
by shipping a real MVP and reconciling the roadmap.

## Why

The `sgpm` package manager is the single most visible missing piece of the
Sengoo developer experience right now:

- `sgc` already supports `build`, `run`, `check`, `fmt` for individual files.
- `sglsp` and `sgfmt` are functional.
- `runtime/` is mature enough to back real applications.
- But there is no project-level orchestration. Anyone wanting to ship a
  multi-file Sengoo project today must invoke `sgc` per file or write their
  own glue script.

Worse, the project's authoritative spec ledger says the package manager is
done, while the code clearly isn't. That undermines the credibility of the
OpenSpec system itself, and it makes onboarding new contributors painful
because they cannot trust the task ledger.

This proposal corrects both problems with one focused change.

## What Changes

- Implement `tools/sgpm/src/main.rs` and supporting modules (`manifest.rs`,
  `resolver.rs`, `runner.rs`, `scaffold.rs`).
- Define `Sengoo.toml` v1 schema with `[package]`, `[bin]`, `[lib]`,
  `[dependencies]`. Only `path = "..."` dependencies are accepted; any
  non-path entry is rejected with a forward-looking "registry support not
  implemented" diagnostic.
- Ship subcommands `new`, `build`, `check`, `run`, `test`, `fmt`, `tree`.
- Add fixture-based integration tests under `tools/sgpm/tests/`.
- Add `docs/sgpm-quickstart.md` and link from both READMEs.
- Update `openspec/changes/toolchain-language-runtime-roadmap/tasks.md` so
  rows 1.8 / 1.9 / 1.10 reflect reality (path-deps shipped, remote
  registry / lockfile / workspace deferred).

## Capabilities

### New Capabilities

- `package-management-sgpm-mvp`: Project-scoped builds with `Sengoo.toml`,
  path-based dependencies, topological build ordering, and forwarded
  `sgc`/`sgfmt` invocations.

### Modified Capabilities

- `package-management-sgpm` (from `toolchain-language-runtime-roadmap`):
  the original capability claimed full registry support; this change narrows
  the v1 surface to path dependencies and explicitly defers remote-registry
  work to a follow-up.

## Impact

- Affected code: `tools/sgpm/`, `Cargo.lock`, `README.md`, `README.zh-CN.md`,
  `docs/sgpm-quickstart.md` (new), `openspec/changes/toolchain-language-runtime-roadmap/tasks.md`.
- Affected developer interfaces: a new `sgpm` CLI binary becomes available
  via `cargo run -p sgpm -- ...` or `cargo install --path tools/sgpm`.
- Affected workspace: `cargo build --workspace` now actually compiles `sgpm`
  source rather than an empty crate; `cargo test --workspace` adds the
  `sgpm` integration test suite.
- No runtime ABI impact. No changes to `compiler/`, `runtime/`, `sgc`,
  `sgfmt`, or `sglsp` source code.
