## 1. Manifest Schema and Parser

- [x] 1.1 Define `Manifest`, `PackageMeta`, `BinTarget`, `LibTarget`, `Dependency` structs in `tools/sgpm/src/manifest.rs` with `serde::Deserialize`.
- [x] 1.2 Parse `Sengoo.toml` via `toml::from_str`; reject unknown top-level keys.
- [x] 1.3 Validate `[package].version` and any future `version = ...` fields with `semver::Version::parse`.
- [x] 1.4 Reject any `[dependencies]` entry that is not exactly `{ path = "..." }` with a `miette::Diagnostic` pointing at the offending entry and the registry follow-up plan.
- [x] 1.5 Add unit tests for: minimal valid manifest, missing `[package]`, invalid semver, unknown key, bare string dep, version-only dep, git-only dep.

## 2. Path Dependency Resolver

- [x] 2.1 Add `tools/sgpm/src/resolver.rs` with `Graph::from_root(manifest_path) -> Result<Graph>`.
- [x] 2.2 Walk path deps, canonicalize each path, deduplicate by canonical path.
- [x] 2.3 Detect cycles via DFS; on cycle, return a diagnostic with the full path trace.
- [x] 2.4 Topologically sort the resolved graph.
- [x] 2.5 Add unit tests for: linear chain (A → B → C), diamond (A → B, A → C, B → D, C → D), self-loop, two-cycle, three-cycle.

## 3. Command Runner

- [x] 3.1 Add `tools/sgpm/src/runner.rs` with `Runner::find_sgc()` and `Runner::find_sgfmt()` using `which::which` first, then walking workspace `target/{debug,release}/`.
- [x] 3.2 Implement `runner::build(graph, release)` that loops topo order and execs `sgc build <entry> [-O 2]`, forwarding stdout/stderr.
- [x] 3.3 Implement `runner::check(graph)` mirroring build but routing to `sgc check`.
- [x] 3.4 Implement `runner::run(graph, args)` that builds, then execs the resulting binary with the user-supplied args.
- [x] 3.5 Implement `runner::test(graph)` discovering `tests/*.sg` per package and running each.
- [x] 3.6 Implement `runner::fmt(graph)` that walks `src/**/*.sg` and shells out to `sgfmt`.

## 4. Scaffolding

- [x] 4.1 Add `tools/sgpm/src/scaffold.rs` with `scaffold::new_project(name)` that creates `name/Sengoo.toml`, `name/src/main.sg` (hello-world body), `name/.gitignore` (excluding `target/`).
- [x] 4.2 Add unit test that scaffolds into a tempdir and verifies file presence and content.

## 5. CLI Surface

- [x] 5.1 Add `tools/sgpm/src/main.rs` with `clap::Parser` derive macro covering `new`, `build`, `check`, `run`, `test`, `fmt`, `tree`, `clean`.
- [x] 5.2 Common flags: `--manifest-path PATH` (default `./Sengoo.toml`), `--release`, `-v`/`--verbose`.
- [x] 5.3 Wire each subcommand to the matching `runner::*` or `scaffold::*` entry.
- [x] 5.4 Use `miette::Result` for top-level error reporting so manifest diagnostics render with source spans.

## 6. Integration Tests

- [x] 6.1 Create fixture trees under `tools/sgpm/tests/fixtures/`: `hello/`, `dep_chain/` (3 packages A → B → C), `cycle/` (A → B → A).
- [x] 6.2 Add `tools/sgpm/tests/integration.rs` with at least:
      - `parses_minimal_manifest`
      - `rejects_cyclic_path_deps`
      - `rejects_remote_dep_without_registry`
      - `resolves_topological_order_three_packages`
      - `sgpm_new_creates_expected_layout`
      - `sgpm_build_invokes_sgc_in_topo_order` (verify via stdout capture)

## 7. Documentation and Roadmap Update

- [x] 7.1 Add `docs/sgpm-quickstart.md` covering `sgpm new`, manifest fields, path deps, common subcommands.
- [x] 7.2 Add a "Package Manager (sgpm)" section to `README.md` and `README.zh-CN.md` linking to the quickstart.
- [x] 7.3 Update `openspec/changes/toolchain-language-runtime-roadmap/tasks.md` rows 1.8 / 1.9 / 1.10 to reflect path-deps shipped, registry/lockfile/workspace deferred.
- [x] 7.4 Add a follow-up note in `toolchain-language-runtime-roadmap/proposal.md` if the registry deferral materially changes the original capability claim.

## 8. Verification and Rollout

- [x] 8.1 `cargo build -p sgpm` succeeds and produces a binary under `target/debug/sgpm`.
- [x] 8.2 `cargo test -p sgpm` passes all integration tests.
- [ ] 8.3 `cargo build --workspace` and `cargo test --workspace` remain green. Blocked on 2026-05-12: `cargo build --workspace` cannot write `target/debug/.fingerprint/.../bin-emit_ir` in this environment (os error 5), and escalation was unavailable.
- [ ] 8.4 Manual smoke test: `sgpm new hello && cd hello && sgpm build && ./target/release/hello` prints expected output. `sgpm new` succeeded on 2026-05-12, but build/run are blocked because the configured `clang.exe` reports `Permission denied`.
