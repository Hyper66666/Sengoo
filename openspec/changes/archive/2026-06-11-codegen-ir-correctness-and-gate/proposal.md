## Why

The `core-language-correctness` change marked arrays, closures, and enum values
as delivered, and CI is green. But building the toolchain and exercising the
**real `sgc` CLI** (`sgc run` / `sgc build`, the textual-LLVM-IR-plus-`clang`
path a user actually invokes) on the toolchain this repo pins for developers
(`clang-14`) shows that those core forms still fail to compile:

- Native array index/iteration emits `getelementptr i64, i64* %u_4` while `%u_4`
  is `[3 x i64]*` (the `alloca`). `examples/04_array.sg`, `examples/05_loop.sg`,
  and `examples/conformance/03_array_write.sg` fail with
  `'%u_4' defined with type '[3 x i64]*' but expected 'i64*'`.
- Environment-capturing closures emit the same element-pointer mismatch.
  `examples/06_lambda.sg` and `examples/conformance/04_closure_multi_capture.sg`
  fail with `'%u_5' defined with type '[1 x i64]*' but expected 'i64*'`.
- A function that returns an enum value emits a function pointer whose type
  disagrees with its call site: `@get` defined with type
  `{ i64, [8 x i8] } (i64)*` but expected `i64 (i64)*`.

The emitted IR is **not type-consistent under typed pointers**: it only compiles
because opaque-pointer LLVM (`clang >= 15`, where every pointer is `ptr`) ignores
the pointee type. CI runs on `clang-19`, so it passes; the pinned developer
`clang-14` rejects it, and a local `cargo test -p sgc
core_conformance_examples_compile_link_and_run` **fails**.

The gate hides this for two reasons: (1) it compiles through the in-crate
`#[cfg(test)]` `compile_source()` helper instead of driving the shipping `sgc`
CLI, and (2) it validates under whatever single `clang` the runner happens to
have, with no pinned/declared LLVM contract. So the gate can stay green while the
compiler is broken for real users.

Separately, the parser mis-handles multi-arm enum matches: a payload-binding arm
(`Variant(bindings)`) only parses when it is the **last** arm. Any arm after it
fails with `parse error: invalid pattern: expected identifier`. Every committed
conformance example places its single payload arm last, so the gate stays green
while idiomatic multi-variant matches do not parse.

This change owns making the documented core forms produce IR that compiles and
runs on the toolchain the project pins, fixing the multi-payload match parse,
and hardening the conformance gate so it can no longer go green while the
shipping compiler is broken.

## What Changes

- Emit **type-consistent LLVM IR** for array places, closure-captured slots, and
  aggregate (enum/struct) function results, so the documented core forms compile
  under the project's pinned LLVM contract (opaque pointers) and do not depend on
  the runner's incidental `clang` version. Either decay array/aggregate places to
  the correct pointee type before `getelementptr`, or emit opaque `ptr` operands
  consistently; in both cases the IR a value is defined with and the IR it is
  used with MUST agree.
- Fix the **multi-payload match parser bug**: a payload-binding match arm
  (`Variant(bindings) => ...`) SHALL parse in any position, including when
  followed by further arms, so multi-variant matches with multiple
  payload-carrying arms compile and run.
- Make the **conformance gate drive the real `sgc` CLI**: the gate compiles,
  links, and runs each pinned core form through `sgc build` / `sgc run` (the
  shipping driver), not the in-crate `compile_source()` test helper, and asserts
  the documented exit code / stdout.
- **Pin the toolchain LLVM contract**: declare and enforce a minimum `clang`/LLVM
  version (and the opaque-pointer expectation) for the native backend, align the
  developer blueprint with the CI toolchain, and fail fast with a clear message
  when the toolchain does not satisfy the contract.
- Add **conformance examples that the previous gate could not catch**: a match
  with two or more payload-carrying arms, and a function that returns an enum
  value, each with an executable result assertion.

## Capabilities

### New Capabilities

- `codegen-ir-correctness-and-gate`: a contract that the documented core forms
  emit type-consistent IR that compiles and runs on the pinned toolchain, that
  multi-payload matches parse, and that the conformance gate exercises the real
  `sgc` CLI under a declared LLVM contract.

### Modified Capabilities

- None in canonical `openspec/specs/`. This change strengthens the verification
  and codegen guarantees that `core-language-correctness` introduced; it cites
  that change and does not re-specify the array/closure/enum/mutability semantics
  themselves.

## Impact

- Implementation touches `compiler/src/codegen/` (IR emission for array places,
  closure captures, aggregate results), `compiler/src/parser/` (match-arm
  pattern parsing), `tools/sgc/` (conformance harness driving the real CLI;
  toolchain version check), `.github/workflows/core-conformance.yml` (pinned
  `clang`/LLVM), and the environment blueprint / `docs` (toolchain contract).
- Parent umbrella: `mainstream-default-readiness` (core-language readiness arm),
  continuing `core-language-correctness`.
- Docs touched: `docs/language-features.md` (multi-arm match guidance, toolchain
  requirement) and `PROGRESS.md` (status reflects CLI-verified behavior).

## Non-Goals

- No new language features: this change does not add trait bounds/objects,
  first-class Option/Result, `as` casts, or generic collections (those are
  separate proposed directions).
- No re-specification of the array/closure/enum/mutability semantics owned by
  `core-language-correctness`; this change only makes them compile and run on the
  pinned toolchain and adds the missing gate coverage.
- No change to match exhaustiveness or guard semantics beyond fixing the
  multi-payload-arm parse defect.
- No switch of the backend away from textual LLVM IR + `clang`; the Cranelift
  fast path is unchanged.
