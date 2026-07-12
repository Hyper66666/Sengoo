## 1. Selection and scaffolding

- [x] 1.1 Choose the flagship app and record the rationale + feature checklist
  (which P0/P1 capabilities it must exercise).
- [x] 1.2 Scaffold it as an `sgpm` package under `packages/` (or `examples/`),
  building under the locked workflow.

## 2. Implementation

- [x] 2.1 Implement core functionality using owned `String`/formatting, generic
  collections, `Result`/`?`, traits, and at least one IO domain.
  - `workspace-audit` now performs a bounded recursive directory walk rather
    than matching fixture names, classifies source/test/manifest paths, sums
    file sizes, stores report counters in `HashMap<String, i64>`, builds text
    with `format`, and writes structured JSON through `JsonDoc`. Fallible walk,
    string growth, JSON, and file-write paths propagate the shared status type
    through `Result`/`?`; `WorkspaceSummary: AuditCheck` supplies the report
    score.
- [x] 2.2 Use async/concurrency where the app benefits (with the safety model).
  - The audit score is split into four deterministic jobs over an
    `ArcMutex<i64>` shared counter. Each worker receives an owned clone, the
    main task joins all RAII job handles, and the app checks the parallel result
    against the `AuditCheck` trait implementation before writing reports.
- [x] 2.3 Ensure zero manual `.free()/.drop()/.close()` in the app code.

## 3. Tests, CI, docs

- [x] 3.1 Unit + integration tests using the test framework (fixtures and
  `test_*` discovery).
  - The fixture tests now exercise real recursive discovery and JSON/text
    report output; the missing-tests case proves the stable `STATUS_NOT_FOUND`
    failure path.
- [x] 3.2 CI integration gate: build, test, and run the flagship app on every
  change.
- [x] 3.3 Document the app as a worked example; link from `README.md` and
  `examples/README.md`.
- [x] 3.4 Run `openspec validate flagship-reference-application --strict`.

## Verification

- The flagship app builds, tests, and runs in CI (task 3.2)
- A capability checklist confirms it exercises the required P0/P1 features
- Static check / review confirms no manual resource release in app code
