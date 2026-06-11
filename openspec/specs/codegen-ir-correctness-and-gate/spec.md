# codegen-ir-correctness-and-gate Specification

## Purpose
TBD - created by archiving change codegen-ir-correctness-and-gate. Update Purpose after archive.
## Requirements
### Requirement: Documented core forms SHALL compile and run via the real `sgc` CLI

The pinned core forms SHALL compile to valid LLVM IR, link, and run with the
documented result when built through the shipping `sgc` CLI (`sgc build` /
`sgc run`) on the project's pinned toolchain. A form is conformant only when it
has a committed runnable example and an executable test that drives the `sgc`
CLI and asserts its result or process exit code.

#### Scenario: A core form is exercised through the shipping CLI

- **WHEN** the conformance gate builds and runs a pinned core form through the
  shipping `sgc` CLI on the pinned toolchain
- **THEN** the `sgc` invocation exits successfully and the produced program runs
  with the documented exit code or stdout
- **AND** the gate does not substitute an in-crate compile helper for the CLI
- **AND** any deviation fails the gate and names the offending form and example

#### Scenario: The CLI fails where a helper would pass

- **WHEN** the shipping `sgc` CLI cannot compile, link, or correctly run a pinned
  core form on the pinned toolchain
- **THEN** the conformance gate fails the build
- **AND** the gate's result does not depend on the in-crate `compile_source`
  helper path

### Requirement: Core forms SHALL emit type-consistent LLVM IR

Core forms SHALL emit type-consistent LLVM IR: array element places,
closure-captured slots, and aggregate (enum/struct) function results SHALL be
emitted as IR in which the type a value is defined with and the type it is used
with agree, and which passes the verifier of the project's pinned LLVM/`clang`
toolchain. The IR SHALL NOT rely on the incidental `clang` version of the runner
to mask a pointee-type mismatch.

#### Scenario: Array element address is computed with a consistent pointer type

- **WHEN** a program reads or writes `arr[i]` for a fixed-size array allocated as
  `[N x T]`
- **THEN** the emitted `getelementptr` operand type agrees with the array place's
  defined type rather than indexing `[N x T]*` as if it were `T*`
- **AND** the IR passes the pinned toolchain's verifier
- **AND** `examples/04_array.sg`, `examples/05_loop.sg`, and
  `examples/conformance/03_array_write.sg` compile, link, and run with their
  documented results via the `sgc` CLI

#### Scenario: A closure captures a local and compiles on the pinned toolchain

- **WHEN** a program defines `let x = ...; let f = |y| x + y;` and calls `f`
- **THEN** captured slots load and store at a pointer type consistent with their
  definition
- **AND** `examples/06_lambda.sg` and
  `examples/conformance/04_closure_multi_capture.sg` compile and run with their
  documented results via the `sgc` CLI

#### Scenario: A function returns an enum value

- **WHEN** a function declares an enum (or struct) return type and is called
- **THEN** the function's emitted function-pointer type and its call site agree,
  with no `{ i64, [8 x i8] } (i64)*` versus `i64 (i64)*` mismatch
- **AND** the program compiles, links, and runs with the documented result via the
  `sgc` CLI

### Requirement: Payload-binding match arms SHALL parse in any position

Payload-binding match arms SHALL parse in any position. A match arm whose pattern
binds variant payload fields (`Variant(bindings) => ...`) SHALL parse correctly
regardless of its position in the match, including when it is followed by
additional arms, and matches with multiple payload-carrying arms SHALL compile
and run.

#### Scenario: A payload arm is followed by another arm

- **WHEN** a match places a payload-binding arm before a later arm, e.g.
  `match e { E::Z => 0, E::A(n) => n, E::Y => 1 }`
- **THEN** parsing succeeds without `invalid pattern: expected identifier`
- **AND** the match type-checks and runs, selecting the correct arm

#### Scenario: Multiple payload arms in one match

- **WHEN** a match has two or more payload-carrying arms, e.g.
  `match s { Shape::Circle(r) => r, Shape::Square(w) => w }`
- **THEN** all arms parse and the match runs with the correct per-variant binding
- **AND** a committed conformance example covers this with a result assertion

#### Scenario: A genuinely malformed pattern is still rejected

- **WHEN** a match arm uses a syntactically invalid pattern
- **THEN** the parser rejects it with a stable diagnostic
- **AND** the rejection is independent of the arm's position

### Requirement: The native backend SHALL declare and enforce a toolchain contract

The native backend SHALL declare the minimum `clang`/LLVM version (and pointer
model) it targets. CI and the developer environment blueprint SHALL use the same
pinned major version, and `sgc` SHALL report a clear, actionable error when the
detected toolchain is below the contract rather than surfacing a raw IR verifier
error.

#### Scenario: CI and developer toolchains match

- **WHEN** the conformance gate runs in CI and a developer builds locally per the
  environment blueprint
- **THEN** both use the same pinned `clang`/LLVM major version that satisfies the
  declared contract
- **AND** a core form that passes the gate also compiles via `sgc run` for the
  developer

#### Scenario: The toolchain is below contract

- **WHEN** `sgc` runs against a `clang`/LLVM version below the declared contract
- **THEN** `sgc` emits a clear diagnostic identifying the toolchain requirement
- **AND** it does not surface only a raw LLVM IR verifier error

