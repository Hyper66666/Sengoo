# Sengoo Language Reference

Reference version: draft for toolchain `0.1.x`.

This is the authoritative entry point for Sengoo language behavior. It records
implemented status first, and links each major surface to a proof example,
test, or deeper document. Historical design notes may describe features that do
not exist yet; this reference wins when they disagree.

Status labels:

- **Supported**: implemented and covered by tests or examples.
- **Subset**: usable, but important cases remain documented as open.
- **Experimental**: implemented behind a limited path or still changing.
- **Unsupported**: reserved or planned, but not implemented.

## Proof Sources

- Core examples: `examples/*.sg`
- Standard-library examples: `examples/stdlib/*.sg`
- Realworld package loop: `examples/realworld/SUPPORT_MATRIX.md`
- Compiler tests: `compiler/src/tests/`
- CLI/tooling tests: `tools/sgc/src/tests.rs`, `tools/sgpm/tests/`
- Feature notes: `docs/language-features.md`
- Async semantics: `docs/runtime-async-semantics.md`
- Native debugging: `docs/debugging-native.md`
- FFI: `docs/ffi.md`

## Executable Reference Examples

Every Sengoo fence in this reference carries a `compile` or `run` mode. The
`language_reference_doctests` integration test extracts these blocks and sends
them through the checked-in `sgc` binary. A run block may declare its exact
stdout with `// doctest-stdout:`; changing syntax or behavior without updating
the reference therefore fails CI.

```sg run
// doctest-stdout: 6
def main() -> i64 {
    let mut total = 0;
    for value in [1, 2, 3] {
        total = total + value;
    }
    println(total);
    0
}
```

```sg run
// doctest-stdout: 42
trait Score {
    def score(self) -> i64 {}
}

struct Answer { value: i64 }

impl Score for Answer {
    def score(self) -> i64 { self.value }
}

def read_score<T: Score>(value: T) -> i64 {
    value.score()
}

def main() -> i64 {
    println(read_score(Answer { value: 42 }));
    0
}
```

```sg run
// doctest-stdout: 7
enum Maybe { Empty, Value(i64) }

def main() -> i64 {
    let selected = Maybe::Value(7);
    let value = match selected {
        Maybe::Empty => 0,
        Maybe::Value(inner) => inner,
    };
    println(value);
    0
}
```

```sg compile
import std::string;

def consume(value: String) -> i64 {
    value.len()
}

def main() -> i64 {
    let owned = string_from_str("sengoo").unwrap_or(string_new());
    consume(owned)
}
```

```sg compile
import std::result;

def propagate(value: Result<i64, i64>) -> Result<i64, i64> {
    let inner = value?;
    Result { is_ok: true, value: inner + 1, error: 0 }
}

def main() -> i64 { 0 }
```

## Lexical Grammar

| Construct | Status | Proof / notes |
| --- | --- | --- |
| Identifiers and keywords | Supported | Parser and lexer tests under `compiler/src/tests/`. |
| Integer literals | Subset | Decimal, `0x`, `0o`, `0b`, `_` separators, signed/unsigned suffixes; see `compiler/src/tests/cast_semantics_tests.rs`. |
| Float literals | Subset | `f32`/`f64` suffixes and basic arithmetic are supported; full edge-case conformance remains under `numeric-type-system`. |
| String literals and `&str` | Supported | `tools/stdlib/string.sg`, `compiler/src/tests/stdlib_surface_tests.rs`. |
| Character literals | Subset | Unicode scalar value surface exists; broader char casts and iterator item typing remain open. |
| Comments | Supported | Line comments are used throughout examples. |

## Types

| Construct | Status | Proof / notes |
| --- | --- | --- |
| `bool` | Supported | Core examples and typechecker tests. |
| Signed integers | Subset | `i8/i16/i32/i64/isize`; current pointer-sized policy is 64-bit native. |
| Unsigned integers | Subset | `u8/u16/u32/u64/usize`; large suffixed `u64`/`usize` literals are supported. |
| Floats | Subset | `f32/f64` arithmetic and stdlib math helpers; exhaustive IEEE edge cases remain open. |
| `&str` | Supported | Borrowed text view for literals and runtime string pointers. |
| `String` | Supported | Owned UTF-8 handle with move/drop, formatting, comparison, slicing, and push helpers. |
| Structs | Supported | Named fields, literals, methods, derives. |
| Enums | Supported | Unit and payload variants, construction, return values, and `match`. |
| Arrays | Subset | Fixed array syntax is covered in examples; collection work focuses on `Vec<T>`. |
| References | Subset | Borrowing and move blocking are lexical and conservative. |
| `dyn Trait` | Experimental | Single-trait `&self`/`&mut self` dispatch and owned vtable-drop glue exist; `Box<dyn>`, multi-trait objects, value receivers, and Cranelift dispatch remain open. |

## Expressions And Statements

| Construct | Status | Proof / notes |
| --- | --- | --- |
| `let` / `let mut` | Supported | Immutable assignment diagnostics are shared by `sgc` and `sglsp`. |
| Blocks and tail expressions | Supported | Core examples. |
| `if` / `else` | Supported | Snapshot and conformance tests. |
| `while` / `for` / `loop` | Subset | Common loops compile; drop coverage is verified for current lowering paths. |
| `return`, `break`, `continue` | Subset | Implemented in current MIR/drop paths; edge cases stay under AMM follow-up tests. |
| Method calls | Supported | Inherent methods and trait methods. |
| Operators | Supported | Primitive intrinsics and user-defined `Add`/`Sub`/`Mul`/`Div`/`Rem`/`Neg` dispatch are covered by `numeric_operator_traits`. |
| `as` casts | Subset | Integer/float/bool cast matrix exists; unsupported pairs report diagnostics. |
| `?` | Supported | Result propagation is implemented with drop on early-return paths. |
| `match` | Subset | Payload matches are supported; exhaustiveness and guard polish remain active language work. |

## Ownership, Borrowing, And Drop

Sengoo uses move-based ownership plus compiler-inserted cleanup for `Drop`
types. Drop order is reverse declaration order. Moving an owned value invalidates
the source; moving while a lexical borrow is live is rejected.

Status: **Subset**.

Proof:

- `docs/language-features.md#29-ownership-moves-and-automatic-drop`
- AMM compiler tests under `compiler/src/tests/`

Known open work:

- Leak-check harnesses for more runtime domains.
- Richer temporary/borrow lifetime precision.
- Some owned aggregate values inside generic wrappers still need move/drop
  polish.

## Generics, Traits, And Derive

| Construct | Status | Proof / notes |
| --- | --- | --- |
| Generic functions/structs/enums/impls | Supported | Monomorphized instances in compiler tests. |
| Trait bounds and `where` clauses | Supported | `unsatisfied-trait-bound` tests. |
| Associated types | Subset | Type-parameter projections such as `T::Item` work; `Self::Output` operator-trait style remains open. |
| Supertraits | Supported | Enforced by typechecking. |
| Conflicting impl diagnostics | Supported | Duplicate trait impl tests. |
| `dyn Trait` | Experimental | See Types section. |
| `#[derive]` | Subset | Clone/Copy/Eq/Ord/Hash/Default/Debug surfaces exist for current named shapes. |
| Static trait functions | Unsupported | Blocks Rust-style `From<T>::from` today. |

## Modules And Visibility

| Construct | Status | Proof / notes |
| --- | --- | --- |
| `import std::...` | Supported | Standard library examples. |
| Relative/source imports | Supported | `tools/sgc/src/source_imports.rs` tests. |
| Package manifests | Supported | `sgpm` path/git/workspace tests. |
| Fine-grained visibility modifiers | Subset | Public FFI exports exist; broader module visibility remains minimal. |

## Attributes

| Attribute | Status | Proof / notes |
| --- | --- | --- |
| `#[derive(...)]` | Subset | Compiler derive tests. |
| `#[test]` / `#[case]` | Supported | `sgc test` generated harness tests. |
| `#[export_name]` / `#[no_mangle]` | Supported | FFI/export tests. |
| `#[link(name = "...")]` | Supported | Stdlib math/runtime bridges. |
| `#[cfg]` / deprecation diagnostics | Subset | LSP parity work remains open in active specs. |

## FFI

Status: **Subset**.

Supported:

- `extern "C"` declarations.
- Exported `pub extern "C" fn`.
- ABI/type checks for scalar, pointer/string, and supported aggregate surfaces.
- Runtime status taxonomy for stdlib fallible bridges.

Proof: `docs/ffi.md`, `examples/ffi/README.md`, compiler FFI tests.

Unsupported or limited:

- Arbitrary C ABI aggregates.
- Full unsafe boundary model.
- Some high-arity/polymorphic FFI signatures.

## Async

Status: **Subset**.

Supported:

- `async def`, async blocks, `await`.
- Runtime sleep/timeout, spawn/task handles, cancellation helpers, and select
  over homogeneous futures.
- User future `Poll<T>` / `AsyncContext` subset.

Proof: `docs/runtime-async-semantics.md`, `compiler/src/tests/async_tests.rs`,
`tools/sgc/src/tests.rs`.

Open:

- Generic bounds across every thread-spawn API and an explicit owned-handle
  policy beyond the current structural `Send`/`Sync` model.
- Generic channels and structured concurrency.
- Work-stealing executor and all-host reactor completion.

## Formatting

Status: **Subset**.

Supported:

- `format` owned `String` builder.
- `{}`, `{:?}` for scalar/current derived shapes.
- Positional placeholders, right alignment, and f64 precision such as `{:.2}`.
- f-string lowering through the same formatting path.

Proof: formatting tests in `compiler/src/tests/` and `docs/language-features.md`.

Open:

- Additional `Formatter` customization beyond the current object protocol.
- Complete float formatting grammar.
- Source-map-perfect f-string diagnostics.

## Standard Library Surface

The language reference only defines language behavior. Standard-library module
availability is tracked in `tools/stdlib/README.md` and the active stdlib
OpenSpec changes. Important current surfaces include:

- `std::string`
- `std::math`
- `std::collections`
- `std::json`
- `std::file` / `std::dir` / `std::path`
- `std::process`
- `std::net`
- `std::async`
- `std::assert`
- `std::status`

## Versioning Policy

Reference version `0.1.x` tracks the active toolchain line. A release that
changes source semantics must update this file in the same change or explicitly
mark the behavior as experimental/unsupported. All Sengoo code blocks in this
file are compile/run doctests enforced by the `sgc` workspace test gate.
