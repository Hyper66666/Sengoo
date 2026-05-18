## Context

The `tools/sgpm` crate exists in `Cargo.toml` and the workspace member list,
but `src/` is empty. The dependency stack is already provisioned for a full
package manager (network, archive, hashing, semver, manifest), so the design
question is not "what tools do we need" but "what surface do we ship in v1
without overcommitting to ABI shapes that we will regret later".

The compiler-side tooling (`sgc`, `sgfmt`) already exposes the verbs that a
package manager would shell out to. The missing layer is project-aware
orchestration: a manifest format, a dependency graph, and per-package
invocations of those verbs.

## Goals / Non-Goals

**Goals:**
- Ship a working `sgpm` binary that handles the 80% project-scope case:
  one binary or library, with optional path dependencies.
- Define a stable manifest schema that can be extended later without
  breaking v1 manifests.
- Provide clear forward-looking errors for any v1 limitation (no remote
  registry, no workspace, no lockfile).
- Add enough integration coverage that the next change can refactor
  internals safely.

**Non-Goals:**
- Designing the remote-registry protocol or upload workflow.
- Building a lockfile or solving multi-version dependency constraints.
- Workspace semantics (`[workspace]` with multiple members).
- Any network calls in v1; all `reqwest`/`tokio` deps stay unused but
  available so v2 can add a registry without manifest format change.
- Build-script execution analogous to Cargo's `build.rs`.
- Replacing `sgc` argument forwarding with a re-implementation of the
  compile pipeline inside `sgpm`.

## Decisions

### 1. Path-only dependencies in v1

`[dependencies]` accepts entries shaped `name = { path = "..." }`. Any other
shape (`version = "..."`, `git = "..."`, bare strings) is rejected with a
diagnostic that explicitly says "registry support not yet implemented" and
references the follow-up change.

**Alternative considered**: accept `version = "..."` and silently skip
resolution. Rejected because it is a footgun — users would think their
remote dep was being resolved.

### 2. Topological build, no parallelism in v1

`sgpm build` walks the dep graph in topological order and invokes `sgc
build` per package serially. v1 does not parallelize; serial output is
trivially debuggable, and cross-package parallelism is a separate
optimization that requires careful diagnostics interleaving.

**Alternative considered**: parallel build pool. Deferred until v2 has a
diagnostic aggregator.

### 3. Forward to `sgc`/`sgfmt` rather than embed compiler API

`sgpm` shells out to the compiler binaries instead of linking against
`sengoo-compiler` directly. This keeps `sgpm` independent of compiler
internals and lets the compiler refactor without breaking the package
manager.

**Alternative considered**: link the compiler library and call API
directly. Rejected because it locks `sgpm` to compiler version, doubles
binary size, and conflicts with the goal of keeping the compiler crate
free from CLI concerns.

### 4. Fixture-based integration tests over unit-only

`tools/sgpm/tests/` contains real on-disk fixture project trees that the
test binary copies to a tempdir and invokes `sgpm` against. Unit tests are
added only where parser logic is genuinely unit-testable in isolation.

**Alternative considered**: pure unit tests with mocked filesystem.
Rejected because path-resolution bugs are exactly what fixture tests
catch, and all the major bugs in this domain are filesystem-shaped.

### 5. Standalone `Sengoo.toml`, no Cargo.toml fusion

`Sengoo.toml` is the single source of truth for a Sengoo project. `sgpm`
does not read or write `Cargo.toml`. The two coexist in the workspace root
because the workspace itself is a Cargo workspace, but Sengoo projects
authored with `sgpm new` only have `Sengoo.toml`.

**Alternative considered**: piggyback on `Cargo.toml` with `[package.metadata.sengoo]`.
Rejected because Sengoo projects should not require Rust toolchain.

## Risks / Trade-offs

- **Windows path handling.** `path = "../shared"` on Windows must round-trip
  through `Path::new` and `canonicalize`. Mitigation: Windows CI run, plus
  an integration test that uses backslashes in the manifest.
- **Binary discovery.** `sgc` and `sgfmt` may not be on `PATH` for an
  end-user. Mitigation: `which::which` first, fall back to walking up from
  the current working directory looking for `target/{debug,release}/sgc`.
- **Roadmap update touches a separate change.** Updating
  `toolchain-language-runtime-roadmap/tasks.md` from `[x]` to a more
  truthful state could surface other unshipped roadmap items. Mitigation:
  scope the update tightly to rows 1.8 / 1.9 / 1.10 and call out the rest
  as out of scope for this change.
- **`reqwest`/`tokio` stay unused in v1.** They show up as compile-time
  dependencies but produce no runtime calls. Mitigation: keep them in
  `Cargo.toml` so v2 can add the registry path without a dependency change,
  but document explicitly that v1 is offline-only.

## Migration Plan

1. Land `manifest.rs` + parser + unit tests first; this is purely additive
   and risk-free.
2. Layer `resolver.rs` on top, behind unit tests.
3. Add `runner.rs` with command forwarding; gated by integration tests.
4. Add `scaffold.rs` for `sgpm new`.
5. Wire `main.rs` clap surface.
6. Update `toolchain-language-runtime-roadmap/tasks.md` last so the
   ledger flip is the final commit in the change.

Rollback: `tools/sgpm/src/` deletion. The crate goes back to "declared but
empty"; nothing else in the workspace depends on `sgpm`.

## Open Questions

- Should `sgpm new` ship a default `examples/` directory with a sample
  `.sg` file, or stay minimal? **Tentative answer**: minimal in v1, expand
  with `examples-coverage-expansion` change.
- Do we add a `sgpm clean` subcommand in v1? **Tentative answer**: yes,
  trivially `rm -rf target/`. Add to v1.
- Should `Sengoo.toml` allow a `[package].edition` field even though there
  is only one Sengoo edition today? **Tentative answer**: yes, parse and
  store it but only validate against the value `"2026"`. Future-proofing
  for free.
- How do we communicate the "remote registry not implemented" error?
  **Tentative answer**: `miette::Diagnostic` with a helpful message that
  links to the roadmap change.
