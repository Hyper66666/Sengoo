## 1. Reference content

- [x] 1.1 Author the reference structure (lexical grammar, types, expressions,
  statements, ownership/borrowing + `Drop`, generics/traits, pattern matching,
  modules/visibility, attributes, FFI, async, formatting).
- [x] 1.2 For each construct, record status (Supported / subset / unsupported)
  with a link to a proof example or test.
- [x] 1.3 Reconcile every claim against the implementation; remove or mark
  unimplemented spec-draft features.
  - Reconciled operator-trait dispatch, owned `dyn Trait` drop, structural
    `Send`/`Sync`, Formatter support, and remaining subset/unsupported rows
    against current compiler/stdlib evidence. The reference continues to label
    pointer-width, generic collections, async executor/reactor, and static
    trait function gaps as subsets rather than draft promises.

## 2. Doc-tests

- [x] 2.1 Mark reference code blocks as compile-only or compile-and-run.
  - Every Sengoo fence uses `sg compile` or `sg run`; run blocks declare an
    exact `// doctest-stdout:` line.
- [x] 2.2 Add a CI job that extracts and compiles/runs the reference code blocks.
  - `tools/sgc/tests/language_reference_doctests.rs` extracts every Sengoo
    fence and invokes the checked-in `sgc` binary. The existing workspace test
    step in `.github/workflows/core-conformance.yml` executes this integration
    test on CI.
- [x] 2.3 Fail CI when a reference example does not compile/run as documented.
  - Unmarked/unterminated fences, failed `sgc check/run`, and changed run stdout
    are hard test failures. Evidence: `cargo test -p sgc --test
    language_reference_doctests -- --nocapture` (2 passed).

## 3. Versioning and migration

- [x] 3.1 Define a reference-versioning policy aligned with toolchain releases.
- [x] 3.2 Mark `Sengoo_Language_Specification.md` historical and link to the new
  reference; fix or drop the corrupted draft.
- [x] 3.3 Run `openspec validate authoritative-language-reference --strict`.

## Verification

- The doc-test CI job compiles/runs every reference code block (task 2.2)
- A reviewer spot-checks reference claims against `examples/` and tests
- `openspec validate --all --strict` remains green
