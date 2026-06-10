## 1. Baseline And Conformance Harness

- [x] 1.1 Run `openspec validate core-language-correctness --strict`.
- [x] 1.2 Pin the v1 core-conformance form list (see `design.md`) and record the
  expected result for each form.
- [x] 1.3 Add a conformance harness (extend the `tools/sgc` example smoke path or
  add a dedicated runner) that compiles each form's example and asserts its
  result or process exit code.
- [x] 1.4 Record the baseline status of array index, array `for`, closure
  capture, `let mut`, and enum values. Array/closure were already green on the
  merged implementation base; mutability/enum tests supplied the red/green
  baseline. See `VERIFICATION.md`.
- [x] 1.5 Reconcile the historical 11 failing Linux cases. They were not
  reproducible from the merged implementation base, no snapshot was
  rebaselined, and Linux CI remains the final source of truth.

## 2. Native Array Correctness

- [x] 2.1 Fix fixed-size array value/address lowering so an array place decays to
  an element pointer (`T*`) before `getelementptr`, eliminating the
  `'[N x T]*' ... but expected 'i64*'` IR verification error.
- [x] 2.2 Cover `arr[i]` read and write, and `for v in arr` iteration, with
  runnable examples and executable result assertions.
- [x] 2.3 Restore `examples/04_array.sg`, `examples/05_loop.sg`, and the
  `docs/language-features.md` §2.2 snippet to compiling-and-correct status.
- [x] 2.4 Add negative tests for out-of-form array misuse that must still be
  rejected, with stable diagnostics.

## 3. Closure Capture Correctness

- [x] 3.1 Fix environment-capturing closure lowering so captured-variable slots
  load/store at the correct pointer type.
- [x] 3.2 Restore `examples/06_lambda.sg` and add closure examples covering
  capture-by-value of one and multiple locals with result assertions.
- [x] 3.3 Add negative tests for unsupported closure shapes that remain rejected,
  with stable diagnostics.

## 4. Mutability: `let mut` And Immutable Assignment

- [x] 4.1 Parse `let mut <ident>` (and the `mut` binding pattern) and add parser
  tests for accepted and rejected forms.
- [x] 4.2 Thread a mutability flag through HIR/typeck and reject assignment to an
  immutable binding with a stable diagnostic code.
- [x] 4.3 Prove `sgc` JSON and `sglsp` parity (same range, severity, code/message
  family) for the immutable-assignment diagnostic.
- [x] 4.4 If enforcement is source-incompatible with committed sources/examples,
  add a migration note first and update affected examples in the same change.

## 5. Enum Variants As Values

- [x] 5.1 Resolve `Enum::Variant` in value position; add typeck tests for known
  and unknown variants.
- [x] 5.2 Support payload-carrying variant construction for variants with fields
  and lower to the discriminant/payload representation consumed by `match`.
- [x] 5.3 Add runnable examples constructing enum values and matching on them,
  with result assertions; extend `examples/ergonomics/03_enum_match.sg` so its
  `main` actually constructs and consumes a variant.
- [x] 5.4 Add negative tests for unknown variants and arity/type mismatches with
  stable diagnostics.

## 6. Reproducibility And Doc Reconciliation

- [x] 6.1 Commit `Cargo.lock` and add a `rust-toolchain.toml` pinning the Rust
  version used to build the workspace.
- [x] 6.2 Wire the conformance harness and `cargo test` into CI on Linux.
- [x] 6.3 Correct the backend description (textual LLVM IR + `clang`, plus the
  Cranelift fast path) in `README.md`, `README.zh-CN.md`, and
  `docs/language-features.md`.
- [x] 6.4 Remove the unused `inkwell` / `llvm-sys` / `pyo3` workspace
  dependencies from the root `Cargo.toml` (or document why they are retained).
- [x] 6.5 Update `PROGRESS.md` status flags so completed items reflect verified,
  compiling-and-running behavior.

## 7. Verification

- [ ] 7.1 Conformance harness: every v1 core form compiles, runs, and asserts the
  expected result on Linux.
- [ ] 7.2 `cargo test` is green on Linux (failing cases fixed or rebaselined with
  a recorded reason).
- [x] 7.3 `sgc check` / JSON diagnostic snapshots for representative accepted and
  rejected core forms.
- [x] 7.4 `sglsp` diagnostic parity proven for every new diagnostic code.

## Archive Gate

- [ ] `openspec validate core-language-correctness --strict` passes.
- [ ] Every v1 core-conformance form has a committed example and an executable
  test that asserts its result.
- [ ] Array indexing/iteration, closure capture, `let mut`, immutable-assignment
  rejection, and enum-value construction all compile to valid IR and run
  correctly.
- [ ] `cargo test` is green on Linux, with any rebaselined snapshots reviewed.
- [ ] `Cargo.lock` and `rust-toolchain.toml` are committed and the conformance
  gate runs in CI.
- [ ] Docs and `PROGRESS.md` match verified behavior; unused backend deps are
  removed or justified.
- [ ] Any source-incompatible mutability cleanup has an accepted migration note.
