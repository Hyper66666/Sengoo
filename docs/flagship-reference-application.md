# Flagship Reference Application

Selected app: **workspace-audit**.

`workspace-audit` is a local CLI package that audits a Sengoo workspace-shaped
directory, reads optional configuration, inspects source/test/manifest presence,
and emits text plus JSON report files. It is intentionally a tool a Sengoo
developer would actually use, not a toy benchmark.

## Why This App

The app exercises the mainstream-readiness surfaces that matter for local
tooling:

- `Result` for fallible file/config/report paths;
- traits for pluggable checks;
- file and directory IO plus JSON config parsing where available;
- test framework fixtures and `test_*` discovery;
- optional async/concurrency for parallel file probes once the safety model is
  ready.

## Feature Checklist

Required for the first implementation:

- Read a config file or use defaults.
- Audit a workspace-shaped root deterministically.
- Count `.sg`, `Sengoo.toml`, and test files.
- Parse a small JSON or config input where available.
- Build JSON/text report files without manual `.free()`, `.drop()`, or
  `.close()` calls in application source.

Current proof: `src/lib.sg` uses `DirWalk` for a real bounded recursive scan,
`HashMap<String, i64>` for report counters, the `AuditCheck` trait for scoring,
owned formatting for the text report, `JsonDoc` for structured output, and
`Result`/`?` across walk/string/JSON/file failures. `sgpm test --locked` runs
both the populated fixture and the missing-tests error path. Four joined
shared-counter workers compute source/test/manifest/byte score components over
`ArcMutex<i64>` and cross-check the trait-based serial score.
- Include unit tests using `test_*` discovery plus `setup`/`teardown`.
- Run through `sgpm check`, `sgpm test`, `sgpm fmt --check`, `sgpm doc`,
  `sgpm build`, and `sgpm run`.

## Phase 1 Refresh

The Phase 1 refresh moved report counters to inferred `hashmap_new()` on the
ABI-v1 generic collection surface. The app now exercises target-defined
numeric casts when serializing counters, recursive `DirWalk`, and owned
formatting without scalar collection constructors or manual resource release.
The locked package loop is therefore evidence for the archived numeric and
generic-collection gates, not only the earlier P0 language surface.

Concurrency remains a supported subset: the useful workload currently uses
the transition `ArcMutex<i64>` shared-counter API. Replacing that compatibility
surface with generic `Arc<Mutex<T>>`, structured task scopes, and an
event-driven all-host reactor remains owned by
`concurrency-safety-and-async-io`.

- Async network or status checks.
- A long-running watch mode.

## Proposed Location

Use `examples/realworld/workspace-audit/` unless a future package registry
layout requires `packages/workspace-audit/`. The realworld location keeps it in
the existing support matrix and locked package loop.

## Success Criteria

- The app builds and runs from a clean checkout.
- Tests are deterministic on Windows and Linux.
- The support matrix records which P0/P1 capabilities it exercises.
- Static review confirms the app source has no manual resource release calls.
