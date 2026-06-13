## 1. Reference content

- [ ] 1.1 Author the reference structure (lexical grammar, types, expressions,
  statements, ownership/borrowing + `Drop`, generics/traits, pattern matching,
  modules/visibility, attributes, FFI, async, formatting).
- [ ] 1.2 For each construct, record status (Supported / subset / unsupported)
  with a link to a proof example or test.
- [ ] 1.3 Reconcile every claim against the implementation; remove or mark
  unimplemented spec-draft features.

## 2. Doc-tests

- [ ] 2.1 Mark reference code blocks as compile-only or compile-and-run.
- [ ] 2.2 Add a CI job that extracts and compiles/runs the reference code blocks.
- [ ] 2.3 Fail CI when a reference example does not compile/run as documented.

## 3. Versioning and migration

- [ ] 3.1 Define a reference-versioning policy aligned with toolchain releases.
- [ ] 3.2 Mark `Sengoo_Language_Specification.md` historical and link to the new
  reference; fix or drop the corrupted draft.
- [ ] 3.3 Run `openspec validate authoritative-language-reference --strict`.

## Verification

- The doc-test CI job compiles/runs every reference code block (task 2.2)
- A reviewer spot-checks reference claims against `examples/` and tests
- `openspec validate --all --strict` remains green
