# Sengoo Language Reference

Reference version: draft for toolchain `0.1.x` with v0.2 mainstream-core
program evidence on SHA `084b623037f007344d76ce50f2e0d01fac57b565`
([PR #47](https://github.com/Hyper66666/Sengoo/pull/47)).

This is the authoritative entry point for Sengoo language behavior. It records
implemented status first, and links each major surface to a proof example,
test, or deeper document. Historical design notes may describe features that do
not exist yet; this reference wins when they disagree.

Local gate evidence for the v0.2 M1–M4 archive set (language coherence, developer
loop, production stdlib streams/Unicode baseline, stability contract) is pinned
to that SHA. Multi-host installed-release matrices and HTTP production serving
(handlers / keep-alive / streaming / TLS server) are residual and are not
claimed Supported by this citation alone.

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

```sg run
// doctest-stdout: 42
import std::math;

def main() -> i64 {
    let narrowed = checked_i64_to_u8(42);
    let widened = checked_u8_to_i64(narrowed.unwrap_or(0u8));
    println(widened.unwrap_or(0));
    0
}
```

## Lexical Grammar

| Construct | Status | Proof / notes |
| --- | --- | --- |
| Identifiers and keywords | Supported | Parser and lexer tests under `compiler/src/tests/`. |
| Integer literals | Supported | Decimal, `0x`, `0o`, `0b`, `_` separators, and signed/unsigned suffixes; see `compiler/src/tests/cast_semantics_tests.rs`. |
| Float literals | Supported | `f32`/`f64` suffixes preserve their source type through parsing, MIR, and production codegen. |
| String literals and `&str` | Supported | `tools/stdlib/string.sg`, `compiler/src/tests/stdlib_surface_tests.rs`. |
| Character literals | Subset | Unicode scalar value surface exists; broader char casts and iterator item typing remain open. |
| Comments | Supported | Line comments are used throughout examples. |

## Types

| Construct | Status | Proof / notes |
| --- | --- | --- |
| `bool` | Supported | Core examples and typechecker tests. |
| Signed integers | Supported | `i8/i16/i32/i64/isize`; `isize` follows the selected target triple's 32-bit or 64-bit pointer width. |
| Unsigned integers | Supported | `u8/u16/u32/u64/usize`; `usize` follows the selected target and large suffixed `u64`/`usize` literals are supported. |
| Floats | Supported | `f32/f64` IEEE-754 arithmetic, predicates, parsing, precision formatting, and stdlib math helpers on the production backend. |
| `&str` | Supported | Borrowed text view for literals and runtime string pointers. |
| `String` | Supported | Owned UTF-8 handle with move/drop, formatting, comparison, slicing, and push helpers. |
| Structs | Supported | Named fields, literals, methods, derives. |
| Enums | Supported | Unit and payload variants, construction, return values, and `match`. |
| Arrays | Supported | Fixed-array index bounds diagnostics (`array-index-out-of-bounds`), assignment, and `for` iteration lower to MIR; see `compiler/src/tests/m1_language_coherence_tests.rs` and `array_assign_tests`. |
| Generic collections | Supported | `Vec<T>`, `VecDeque<T>`, `HashMap<K,V>`, `HashSet<T>`, `BTreeMap<K,V>`, and `BTreeSet<T>` use owning ABI-v1 storage with exact Drop; see `examples/realworld/default-library-conformance`. |
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
| `match` | Supported | Exhaustive enum/`bool` coverage, wildcard, payload bindings, and guards; non-exhaustive/unreachable report stable diagnostics (`non-exhaustive-match`); see `match_typeck_tests` and `m1_language_coherence_tests`. |

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
| Associated types | Supported | Trait/impl associated types and `Self::Item` projections typecheck when uniquely bound; unbounded projections fail closed; see `generic_typeck_tests` and `m1_language_coherence_tests`. Operator-style `Self::Output` polish remains incremental. |
| Supertraits | Supported | Enforced by typechecking. |
| Conflicting impl diagnostics | Supported | Duplicate trait impl tests. |
| `dyn Trait` | Experimental | See Types section. |
| `#[derive]` | Subset | Clone/Copy/Eq/Ord/Hash/Default/Debug surfaces exist for current named shapes. |
| Static trait functions | Unsupported | Blocks Rust-style `From<T>::from` today. |

## Generic Collections And Iterators

Generic owning collections move values into storage, borrow values on reads,
move values back out on removal, and drop every still-owned element exactly
once. Constructors infer their type arguments from the expected result type:

```sg compile
import std::collections;
import std::string;

struct Row {
    name: String,
}

def main() -> i64 {
    let rows: Vec<Row> = vec_new();
    let by_name: HashMap<String, Row> = hashmap_new();
    rows.len() + by_name.len();
}
```

Owning Vec iteration preserves insertion order. The lazy iterator surface
includes `map`, `filter`, `take`, `skip`, and `enumerate`; consuming terminals
include `count`, accumulator-generic `fold`, `collect() -> Vec<T>`, numeric
`sum() -> T` for `T: SumValue`, `collect_hashset()`, and
`collect_hashmap(projector)`. The map projector returns `MapEntry<K,V>`, which
keeps K/V inference argument-driven rather than relying on return-type-only
generic method inference. Empty numeric sums return the numeric identity.

Mutation that may move collection storage is rejected while an element borrow
or borrowing iterator is live. Existing scalar constructor names remain a
source-compatibility surface and route through the same generic storage ABI.

## Numeric Model

LLVM-text plus clang is the production semantic reference for numeric code.
Debug `+`, `-`, and `*` trap on overflow, release builds wrap, and explicit
`wrapping_*`, `checked_*`, and `saturating_*` operations are independent of the
build mode. The concrete `checked_<source>_to_<target>` family covers every
non-identity integer pair and reports overflow or invalid signedness through
`Result`. Lossless widening is also available through `From`/`Into`; narrowing
requires an explicit cast or checked conversion.

The opt-in Cranelift fast path remains experimental. It must match the
production semantics for accepted primitive programs and reject unsupported
programs explicitly; its intentionally smaller surface does not reduce the
supported LLVM-text language contract.

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
