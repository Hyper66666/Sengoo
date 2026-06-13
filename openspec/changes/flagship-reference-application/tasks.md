## 1. Selection and scaffolding

- [ ] 1.1 Choose the flagship app and record the rationale + feature checklist
  (which P0/P1 capabilities it must exercise).
- [ ] 1.2 Scaffold it as an `sgpm` package under `packages/` (or `examples/`),
  building under the locked workflow.

## 2. Implementation

- [ ] 2.1 Implement core functionality using owned `String`/formatting, generic
  collections, `Result`/`?`, traits, and at least one IO domain.
- [ ] 2.2 Use async/concurrency where the app benefits (with the safety model).
- [ ] 2.3 Ensure zero manual `.free()/.drop()/.close()` in the app code.

## 3. Tests, CI, docs

- [ ] 3.1 Unit + integration tests using the test framework (fixtures/params).
- [ ] 3.2 CI integration gate: build, test, and run the flagship app on every
  change.
- [ ] 3.3 Document the app as a worked example; link from `README.md` and
  `examples/README.md`.
- [ ] 3.4 Run `openspec validate flagship-reference-application --strict`.

## Verification

- The flagship app builds, tests, and runs in CI (task 3.2)
- A capability checklist confirms it exercises the required P0/P1 features
- Static check / review confirms no manual resource release in app code
