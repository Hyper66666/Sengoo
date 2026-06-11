# Design: Codegen IR Correctness And Gate Hardening

## 1. Problem

`core-language-correctness` is checked off and CI is green, but the shipping
`sgc` CLI cannot compile the documented core forms on the toolchain this repo
pins for developers. The defects are in codegen IR typing, in the match parser,
and — critically — in the conformance gate that was supposed to catch them.

## 2. Observed Failures (Reproductions)

All reproduced with the release `sgc` CLI on `clang 14.0.0` (the version the
repo's environment blueprint installs). `rustc 1.94.0` per `rust-toolchain.toml`.

| # | Form | Command | Observed |
| --- | --- | --- | --- |
| 1 | Array index | `sgc run examples/04_array.sg` | `examples/build/04_array.ll:36: error: '%u_4' defined with type '[3 x i64]*' but expected 'i64*'` |
| 2 | Array write/for | `sgc run examples/05_loop.sg`, `examples/conformance/03_array_write.sg` | same element-pointer mismatch |
| 3 | Closure capture | `sgc run examples/06_lambda.sg`, `examples/conformance/04_closure_multi_capture.sg` | `'%u_5' defined with type '[1 x i64]*' but expected 'i64*'` |
| 4 | Enum-returning fn | function with `-> EnumType` | `@get` defined with type `{ i64, [8 x i8] } (i64)*` but expected `i64 (i64)*` |
| 5 | Multi-payload match | arm `Variant(b) => ...` followed by any arm | `parse error: invalid pattern: expected identifier` |
| 6 | Local gate | `cargo test -p sgc core_conformance_examples_compile_link_and_run` | FAILED (item 1 inside the test) |

Generated IR for item 1:

```llvm
%u_4 = alloca [3 x i64]                       ; %u_4 : [3 x i64]*
%u_4.elem.0 = getelementptr i64, i64* %u_4, i64 0   ; operand stated as i64*, mismatched
store i64 %t_1, i64* %u_4.elem.0
%t_6 = getelementptr i64, i64* %u_4, i64 %t_5
%t_7 = load i64, i64* %t_6
```

## 3. Root Cause

### 3.1 Typed-pointer inconsistency (items 1–4)

Codegen states pointer operand types (`i64*`) that disagree with the type the
SSA value was defined with (`[3 x i64]*`, or an aggregate function type).

- Under **typed pointers** (LLVM <= 14): a hard verifier error.
- Under **opaque pointers** (LLVM >= 15, every pointer is `ptr`): the pointee
  type is dropped, the mismatch disappears, and the `getelementptr i64, ptr ...`
  form happens to compute the correct address — so it compiles and runs.

CI runs on `clang-19` (opaque pointers), so it never sees the error. The pinned
developer `clang-14` (typed pointers) rejects it.

Two acceptable fixes; the implementation MUST pick one and apply it
consistently:

1. **Decay to the correct pointee type.** For `alloca [N x T]` producing
   `[N x T]*`, compute an element pointer with
   `getelementptr [N x T], [N x T]* %p, i64 0, i64 <idx>` (yielding a real `T*`),
   then index/load/store at `T*`. Apply the analogous decay for closure-captured
   slots and aggregate results.
2. **Commit to opaque pointers.** Emit `ptr` uniformly (target a pinned LLVM that
   supports opaque pointers) so the pointee type is never part of the operand.

Either way the invariant is: *the type a value is defined with and the type it is
used with agree, and the IR passes the verifier of the pinned toolchain.*

### 3.2 Match-arm parser defect (item 5)

The match-arm parser does not correctly terminate a payload pattern
`Variant(bindings)` and resynchronize to the next `Pattern => Expr` arm. When a
payload arm is not last, parsing the following arm starts mid-pattern and fails
with `expected identifier`. The committed examples always place the single
payload arm last, so the path is never exercised.

### 3.3 Gate blind spots (item 6)

`tools/sgc` `core_conformance_examples_compile_link_and_run` calls the in-crate
`#[cfg(test)] compile_source()` helper, which routes through `Codegen` directly,
and links with whatever `clang` is on the runner. It does not invoke the shipping
`sgc` CLI, and it pins no LLVM contract. So it can pass while `sgc run` fails, and
while the pinned developer toolchain rejects the IR.

## 4. Approach

1. **IR typing fix** in `compiler/src/codegen/` for array places, closure
   captures, and aggregate function results, holding the §3.1 invariant.
2. **Parser fix** in `compiler/src/parser/` so a payload-binding arm parses in any
   position; add accepted/rejected parser tests.
3. **Gate hardening** in `tools/sgc`: the conformance harness shells out to the
   built `sgc` binary (`build`/`run`) for each pinned core form and asserts exit
   code / stdout; add new forms (multi-payload match, enum-returning fn).
4. **Toolchain contract**: declare the minimum `clang`/LLVM version and the
   opaque-pointer expectation; pin the CI `clang` and align the developer
   blueprint; emit a clear diagnostic when the toolchain is below contract.

## 5. Toolchain Contract

Chosen implementation: the native backend targets the opaque-pointer LLVM
contract. `sgc build` and native `sgc run` require `clang`/LLVM 15 or newer; core
conformance CI pins clang 19, and the developer blueprint points to the same
major for reproducible native behavior. `sgc build --emit-llvm` remains
available below the native contract because it does not invoke native object
generation. The implementation fails fast with an actionable diagnostic when
the detected native `clang` major is below 15 instead of letting users see only a
raw LLVM verifier error.

- The native backend targets a pinned LLVM/`clang` major version (>= the first
  version whose behavior the gate validates). CI and the developer blueprint use
  the same major version.
- If §3.1 fix (1) (typed-pointer decay) is chosen, the IR additionally remains
  valid on older typed-pointer toolchains; fix (2) sets a hard `clang >= 15`
  floor. The chosen floor is documented and enforced.

## 6. Verification Strategy

- The conformance gate runs the **real `sgc` CLI** for every pinned core form on
  the pinned toolchain in CI; a wrong exit code, link failure, or IR verifier
  error fails the build.
- `cargo test -p sgc core_conformance_examples_*` passes on the pinned toolchain
  with the harness driving the CLI.
- New regression cases: array read/write/for, single- and multi-capture closure,
  enum value as argument **and** as return value, and a match with two or more
  payload-carrying arms.
- `cargo test` is green on Linux; parser tests cover payload arms in first,
  middle, and last positions.

## 7. Risks And Trade-offs

- **Backend churn.** Touching array/closure/aggregate lowering risks regressions
  in async aggregate results (the SysV ABI work in `core-language-correctness`).
  Mitigation: keep the existing aggregate-result tests and add the enum-return
  case alongside them.
- **Opaque vs typed pointers.** Choosing opaque pointers (fix 2) is simpler but
  sets a hard `clang >= 15` floor; typed-pointer decay (fix 1) is more portable
  but touches more emission sites. The design allows either; the gate enforces
  whichever floor is declared.
- **Slower gate.** Driving the real CLI is slower than the in-crate helper.
  Mitigation: keep the pinned core-form set minimal and run it as a dedicated CI
  job.
