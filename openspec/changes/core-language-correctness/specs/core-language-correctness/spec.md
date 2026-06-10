## ADDED Requirements

### Requirement: Documented core forms SHALL compile to valid IR and run

The pinned v1 core-conformance form list SHALL compile to valid LLVM IR, link,
and run with the documented result. A form is conformant only when it has a
committed runnable example and an executable test that asserts its result or
process exit code.

#### Scenario: A core form is exercised by the conformance gate

- **WHEN** the conformance gate compiles and runs a pinned core form (integer or
  `f64` arithmetic, `bool`, string `print`, `if`/`while`/`for v in 0..N`,
  recursion, `struct` + methods, native array index and `for v in arr`,
  environment-capturing closure, `let mut` local, or enum-value construction)
- **THEN** compilation produces valid LLVM IR that passes `clang` verification
- **AND** the program runs and produces the documented result or exit code
- **AND** any deviation fails the gate

#### Scenario: A core form regresses

- **WHEN** a previously-conformant core form stops compiling, stops running, or
  produces a wrong result
- **THEN** the conformance gate fails the build
- **AND** the failure identifies the specific form and example

### Requirement: Native fixed-size arrays SHALL index and iterate correctly

Native fixed-size arrays (`[T; N]`) SHALL support literal construction, element
index read and write (`arr[i]`), and `for v in arr` iteration, lowering to valid
LLVM IR with correct results. Array places SHALL decay to an element pointer
before pointer indexing.

#### Scenario: Array element is read by index

- **WHEN** a program evaluates `arr[i]` for a fixed-size array
- **THEN** lowering computes an element pointer of type `T*` before
  `getelementptr` rather than indexing the aggregate pointer `[N x T]*`
- **AND** the emitted IR passes `clang` verification
- **AND** the read returns the element value at index `i`

#### Scenario: Array is iterated with for-in

- **WHEN** a program runs `for v in arr` over a fixed-size array
- **THEN** the loop binds each element in order
- **AND** `examples/04_array.sg`, `examples/05_loop.sg`, and the
  `docs/language-features.md` §2.2 snippet compile and produce their documented
  results

#### Scenario: Growable arrays remain out of scope

- **WHEN** a program needs a growable/dynamic sequence
- **THEN** it uses runtime-backed stdlib `Vec<T>` owned by the stdlib changes
- **AND** this capability does not add growable arrays to the language core

### Requirement: Environment-capturing closures SHALL compile and run

Closures that capture enclosing locals (`|x| ...`) SHALL lower to valid LLVM IR
and run with correct results, loading and storing captured slots at the correct
pointer types.

#### Scenario: A closure captures and uses a local

- **WHEN** a program defines `let x = ...; let f = |y| x + y;` and calls `f`
- **THEN** the emitted IR passes `clang` verification
- **AND** `examples/06_lambda.sg` compiles and produces its documented result
- **AND** the call returns the value computed from the captured local and the
  argument

#### Scenario: An unsupported closure shape is rejected

- **WHEN** a program uses a closure shape that remains unsupported
- **THEN** the compiler rejects it with a stable diagnostic
- **AND** `sglsp` reports the same code/message family at the closure site

### Requirement: Mutability SHALL be expressible and enforced

`let mut <ident>` SHALL be accepted as the way to declare a reassignable local,
and assignment to an immutable `let` binding SHALL be rejected with a stable
diagnostic that is consistent across the parser, type checker, `sgc` JSON
diagnostics, and `sglsp`.

#### Scenario: A mutable local is declared and reassigned

- **WHEN** a program declares `let mut i = 0;` and later assigns `i = i + 1;`
- **THEN** the declaration parses without a pattern error
- **AND** the reassignment is accepted
- **AND** the program runs with the updated value

#### Scenario: An immutable binding is assigned

- **WHEN** a program declares `let s = 0;` and later assigns `s = s + 1;`
- **THEN** type checking rejects the assignment with a stable diagnostic code
- **AND** `sglsp` mirrors the diagnostic severity, code, and source range

#### Scenario: Immutability enforcement is source-incompatible

- **WHEN** committed sources or examples rely on assigning an immutable binding
- **THEN** a migration note is added before enforcement lands
- **AND** the affected examples are updated to `let mut` in the same change

### Requirement: Enum variants SHALL be usable as values

Enum variants SHALL be constructible in value position as `Enum::Variant` (and
with payload arguments for variants that declare fields), lowering to the
discriminant and payload representation already consumed by `match`.

#### Scenario: A fieldless variant is used as a value

- **WHEN** a program evaluates `Color::Green` in value position and passes it to
  a function or matches on it
- **THEN** type checking resolves the variant instead of reporting
  `undefined variable`
- **AND** lowering produces the correct discriminant
- **AND** a `match` over the value selects the corresponding arm

#### Scenario: A payload-carrying variant is constructed

- **WHEN** a program constructs a variant that declares fields with payload
  arguments
- **THEN** construction is type-checked against the variant's field types
- **AND** lowering stores the payload in the representation consumed by `match`

#### Scenario: An invalid variant use is rejected

- **WHEN** a program references an unknown variant or constructs a variant with
  the wrong arity or argument types
- **THEN** type checking rejects it with a stable diagnostic
- **AND** `sglsp` reports the same code/message family at the use site

### Requirement: The build SHALL be reproducible and CI-gated

The workspace SHALL build reproducibly from a committed lockfile and pinned Rust
toolchain, and the conformance gate plus `cargo test` SHALL run in CI on Linux.

#### Scenario: A clean build resolves pinned dependencies

- **WHEN** the workspace is built from a clean checkout
- **THEN** a committed `Cargo.lock` determines dependency versions
- **AND** a committed `rust-toolchain.toml` selects the Rust version
- **AND** the build does not require resolving newer dependencies than the pinned
  toolchain supports

#### Scenario: CI enforces conformance and tests

- **WHEN** CI runs on Linux
- **THEN** the conformance gate builds and runs every v1 core form
- **AND** `cargo test` runs, with any rebaselined snapshot reviewed and justified
- **AND** a failure in either blocks the build

### Requirement: Documentation SHALL match the implementation

User-facing documentation and status tracking SHALL describe the actual backend
and verified feature status.

#### Scenario: Backend description is corrected

- **WHEN** documentation describes code generation
- **THEN** it states that `sgc` emits textual LLVM IR compiled by `clang` (with a
  Cranelift fast path), rather than implying an in-process LLVM/`inkwell` backend
- **AND** unused `inkwell` / `llvm-sys` / `pyo3` workspace dependencies are
  removed or their retention is documented

#### Scenario: Progress flags reflect verified behavior

- **WHEN** `PROGRESS.md` marks a feature complete
- **THEN** the feature has a conformance example and test proving it compiles and
  runs
- **AND** features that do not yet pass are not marked complete
