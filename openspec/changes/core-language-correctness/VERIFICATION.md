# Verification Record

## Implementation Base

- Base revision: `b026b8a79a13dcee0fed71294ecebee4113ce960`
- Rust: `rustc 1.94.0`
- Cargo: `cargo 1.94.0`
- Clang: `19.1.7`
- OpenSpec CLI: `1.3.1`

## Baseline Reconciliation

The implementation base already contained the array and closure pointer-lowering
repairs described by the original reproduction. Its compiler library suite was
green at 681/681 tests. The historical Linux 11-failure report could not be
reproduced from this base, so no snapshot was rebaselined and no test was
ignored.

The conformance harness records arrays and closures as required passing cases.
The mutability and enum-value tests were added before their implementations and
served as the failing implementation baseline.

## Conformance Matrix

| Form | Example | Expected |
| --- | --- | --- |
| Scalars, print, `if`, `while`, range `for`, `let mut` | `examples/conformance/01_scalars_control.sg` | stdout `core`, exit 9 |
| Recursion | `examples/conformance/02_recursion.sg` | exit 13 |
| Struct construction and field access | `examples/08_struct.sg` | exit 30 |
| `impl` method call | `examples/09_method_call.sg` | exit 43 |
| Fixed array read | `examples/04_array.sg` | exit 20 |
| Fixed array iteration | `examples/05_loop.sg` | exit 15 |
| Fixed array write | `examples/conformance/03_array_write.sg` | exit 42 |
| Single-capture closure | `examples/06_lambda.sg` | exit 15 |
| Multi-capture closure | `examples/conformance/04_closure_multi_capture.sg` | exit 18 |
| Fieldless enum value and match | `examples/ergonomics/03_enum_match.sg` | exit 2 |
| Payload enum value and match | `examples/conformance/05_enum_payload.sg` | exit 42 |
| Multi-field enum payload and match | `examples/conformance/06_enum_multi_payload.sg` | exit 42 |

## Focused Evidence

- `cargo test -p sengoo-compiler core_language_correctness_tests -- --nocapture`
  passes 16 tests, including exhaustive enum-match CFG and JIT enum lowering.
- `cargo test -p sgc core_conformance_examples_compile_link_and_run -- --nocapture`
  compiles, links, runs, and checks all conformance matrix rows.
- `cargo test -p sgc async_native_runtime_preserves_payloadless_enum_across_resume
  -- --nocapture` proves the enum byte-payload ABI remains valid across async
  frame spill/reload.
- `cargo test -p sgc jit_enum_match_ir_is_accepted_by_clang -- --nocapture`
  sends JIT-generated payload-enum match IR through `clang -c`, covering
  construction, discriminant extraction, payload extraction, switch, phi, and
  unreachable-default lowering.
- `cargo test -p sgfmt preserves_mutable_local_bindings -- --nocapture` proves
  formatting preserves `let mut`.
- `sgc` JSON and `sglsp` tests assert stable codes and exact source ranges for
  immutable assignment, enum-value errors, array misuse, and duplicate closure
  parameters.

## Local Full-Workspace Gate

- `cargo clippy --workspace --all-targets --locked -- -D warnings` passes.
- `cargo test --workspace --locked` passes after the async/JIT enum review
  fixes; the compiler library reports 699 passing tests in the final run.

An independent read-only review found and drove regression coverage for two
adjacent enum paths: payloadless enum values crossing `await`, and the public
JIT enum-match path. A follow-up review additionally identified an invalid
exhaustive-match default edge into a phi join. All three findings now have
executable regression tests.

The final independent follow-up review reported no remaining P0, P1, or P2
findings.

## Dependency Decisions

- `inkwell` and `llvm-sys` were removed because the workspace emits textual LLVM
  IR and invokes `clang`.
- `pyo3` remains a workspace dependency because `sengoo-runtime` consumes it
  through the optional `python` feature.

The final full-workspace and Linux CI results are recorded by the PR checks.
