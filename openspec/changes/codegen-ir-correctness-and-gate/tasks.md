## 1. Baseline And Reproduction

- [ ] 1.1 Run `openspec validate codegen-ir-correctness-and-gate --strict`.
- [ ] 1.2 Record the red baseline on the pinned developer toolchain (`clang-14`):
  `sgc run examples/04_array.sg`, `examples/05_loop.sg`,
  `examples/conformance/03_array_write.sg`, `examples/06_lambda.sg`,
  `examples/conformance/04_closure_multi_capture.sg`, an enum-returning function,
  and a multi-payload match. Capture the exact IR verifier / parse errors.
- [ ] 1.3 Confirm the current gate is blind: show
  `cargo test -p sgc core_conformance_examples_compile_link_and_run` passing
  through the in-crate helper while `sgc run` fails on the same example.

## 2. IR Type Consistency

- [ ] 2.1 Decide and document the pointer model (typed-pointer decay vs opaque
  `ptr`) in `design.md`; the chosen model fixes the LLVM contract floor.
- [ ] 2.2 Fix array place lowering so `arr[i]` read/write and `for v in arr`
  produce IR whose `getelementptr` operand type agrees with the `alloca`
  (`[N x T]*`), eliminating the `'[N x T]*' ... but expected 'i64*'` error.
- [ ] 2.3 Fix closure-captured slot lowering so captured locals load/store at a
  pointer type consistent with their definition.
- [ ] 2.4 Fix aggregate (enum/struct) function-result lowering so a function that
  returns an enum/struct value and its call site agree on the function pointer
  type (no `{ i64, [8 x i8] } (i64)*` vs `i64 (i64)*` mismatch).
- [ ] 2.5 Keep the existing async aggregate-result (SysV ABI) tests green while
  changing aggregate lowering.

## 3. Multi-Payload Match Parsing

- [ ] 3.1 Fix the match-arm parser so a payload-binding arm
  (`Variant(bindings) => ...`) parses in first, middle, and last positions.
- [ ] 3.2 Add parser tests for payload arms in every position and for multiple
  payload-carrying arms in one match; add a negative test for genuinely malformed
  patterns that must still be rejected with a stable diagnostic.

## 4. Conformance Gate Drives The Real CLI

- [ ] 4.1 Change the conformance harness in `tools/sgc` to compile, link, and run
  each pinned core form through the built `sgc` binary (`sgc build` / `sgc run`),
  not the in-crate `compile_source()` helper, asserting exit code and stdout.
- [ ] 4.2 Add conformance examples + executable assertions the previous gate could
  not catch: a match with two or more payload-carrying arms, and a function that
  returns an enum value.
- [ ] 4.3 Ensure the gate fails loudly (with the offending form/example named)
  when any pinned form does not compile, link, or run correctly.

## 5. Toolchain Contract

- [ ] 5.1 Declare the minimum `clang`/LLVM version and the pointer-model
  expectation for the native backend; document it in `docs/language-features.md`.
- [ ] 5.2 Pin the CI `clang`/LLVM version in
  `.github/workflows/core-conformance.yml` to the documented contract and align
  the developer environment blueprint to the same major version.
- [ ] 5.3 Make `sgc` emit a clear, actionable diagnostic when the detected
  toolchain is below the declared contract, instead of surfacing a raw IR
  verifier error.

## 6. Doc And Status Reconciliation

- [ ] 6.1 Update `docs/language-features.md` to document multi-arm matches with
  multiple payload arms and the toolchain requirement.
- [ ] 6.2 Update `PROGRESS.md` so array/closure/enum status reflects
  CLI-verified behavior on the pinned toolchain (not helper-only green).

## 7. Verification

- [ ] 7.1 The conformance gate runs the real `sgc` CLI for every pinned core form
  on the pinned toolchain in CI and is green.
- [ ] 7.2 `sgc run` succeeds for `examples/04_array.sg`, `examples/05_loop.sg`,
  `examples/conformance/03_array_write.sg`, `examples/06_lambda.sg`,
  `examples/conformance/04_closure_multi_capture.sg`, the new enum-returning
  example, and the new multi-payload match example, with documented results.
- [ ] 7.3 `cargo test` is green on Linux on the pinned toolchain.
- [ ] 7.4 Parser tests prove payload match arms parse in first/middle/last
  positions.

## Archive Gate

- [ ] `openspec validate codegen-ir-correctness-and-gate --strict` passes.
- [ ] Array index/write/iteration, environment-capturing closures, and
  enum/struct-returning functions compile to type-consistent IR and run correctly
  via the real `sgc` CLI on the pinned toolchain.
- [ ] Payload-binding match arms parse in any position; multi-payload matches run.
- [ ] The conformance gate drives the shipping `sgc` CLI and pins the LLVM
  contract; it cannot pass while `sgc run` of a pinned form fails.
- [ ] CI and the developer blueprint share the same pinned `clang`/LLVM major
  version; `sgc` reports a clear error below the contract.
- [ ] `cargo test` is green on Linux; docs and `PROGRESS.md` match CLI-verified
  behavior.
