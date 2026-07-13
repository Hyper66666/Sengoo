# Sengoo Language Features

Sengoo is a compiled language focused on practical engineering workflows:

- Hybrid Python interoperability for gradual migration
- Fast compile feedback with incremental pipeline reuse
- Textual LLVM IR compiled and linked by `clang` 15+ (core CI pins clang 19), plus a Cranelift fast path
- Optional non-invasive reflection with sidecar metadata

## 1. Current Capability Snapshot

| Capability | Status | Notes |
|---|---|---|
| Core syntax (`def`, `if`, `for`, `while`, `struct`, `impl`) | Available | Use `examples/*.sg` as validated learning surface. |
| Immutable-by-default locals (`let mut` for reassignment) | Available | `immutable-assignment` is shared by `sgc` JSON and `sglsp`. |
| Enum variants as values | Available | Fieldless and payload constructors, enum-returning functions, and multi-payload `match` arms are covered by the core CLI conformance gate. |
| Static type-check pipeline | Available | Entry command: `sgc check <file.sg>`. |
| API documentation generation | Available | `sgc doc <file.sg> --output target/doc`. |
| Incremental compile pipeline | Available | Fingerprint + workset-based invalidation/rebuild strategy. |
| Daemon compile service | Available | `sgc daemon --addr 127.0.0.1:48765`. |
| Python interop runtime path | Available | Runtime integration in `runtime/src/python.rs`. |
| Non-invasive reflection | Available (opt-in) | Enabled only with `--reflect`; default path stays lean. |
| VS Code extension | Available | Current package version: `1.0.0`. |

## 2. Language Surface Highlights

## 2.1 Function-oriented syntax

```sg
struct Point {
    x: i64,
    y: i64,
}

def add(a: i64, b: i64) -> i64 {
    a + b
}
```

## 2.2 Control flow + loops

```sg
def sum(arr: [i64; 4]) -> i64 {
    let mut total = 0;
    for v in arr {
        total = total + v;
    }
    total
}
```

## 2.3 Method call surface

```sg
impl i64 {
    def abs(self) -> i64 {
        if self < 0 { -self } else { self }
    }
}

let x = (-21).abs();
```

## Numeric Scalar Model

The P1 numeric model has one production semantic reference (LLVM-text plus
clang) and an explicitly experimental Cranelift primitive fast path.

Supported today:

- `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64`, `isize`, `usize`,
  `f32`, and `f64` can appear in source-level type annotations where the
  existing type checker and production code generator carry their width.
  `isize`/`usize` follow the selected target triple, including 32-bit range
  checking, MIR layout, casts, overflow helpers, and diagnostics.
- Explicit `as` casts are accepted for integer/float conversions and
  `bool` <-> integer conversions. Integer narrowing truncates to the
  destination width, signed integer widening sign-extends, unsigned integer
  widening zero-extends, signed integer/float casts use signed conversions, and
  unsigned integer/float casts use unsigned conversions. Float casts use
  IEEE-sized extension/truncation. `bool` <-> float and `char` casts are
  rejected until the backends have dedicated lowering for them.
  Float-to-integer casts are saturating and defined for every input: `NaN`
  becomes zero, negative values converted to unsigned become zero, and values
  outside the destination range clamp to its minimum or maximum.
- Signed integer literal suffixes (`i8`, `i16`, `i32`, `i64`, `isize`),
  unsigned integer suffixes (`u8`, `u16`, `u32`, `u64`, `usize`), and float
  suffixes (`f32`, `f64`) are parsed by desugaring the literal through the
  explicit `as` cast pipeline. Based integer literals (`0x`, `0o`, `0b`) and
  `_` digit separators are accepted by the lexer. Unsigned literal tokens carry
  `u64` payloads, so suffixed `u64`/`usize` values above `i64::MAX` compile;
  unsuffixed values above `i64::MAX` are rejected with a diagnostic requiring
  an unsigned suffix.
- Mixed-width signed integer arithmetic widens operands to a common signed
  integer width. Mixed-width fixed unsigned integer arithmetic widens operands
  to a common unsigned width. Unsigned comparison, division, remainder, and
  right shift lower to unsigned LLVM/JIT opcodes.
- `std::math` exposes f32/f64 absolute/min/max, root/power/exponential/log,
  floor/ceil/round, core trig helpers, and predicates for `NaN`, finite, and
  infinite values.
- `std::math` exposes `checked_<source>_to_<target>` for every non-identity
  pair across the ten integer types. Failed magnitude narrowing reports
  `STATUS_OVERFLOW`; negative signed-to-unsigned conversions report
  `STATUS_INVALID_ARGUMENT`. Pointer-sized destinations use the selected
  target width rather than host limits.
- `std::math` adds explicit `i64` overflow helpers as inherent methods:
  `wrapping_add/sub/mul`, `checked_add/sub/mul -> Option<i64>`, and
  `saturating_add/sub/mul`. The same method family is also available on
  `i32`, `i16`, `i8`, `isize`, `u64`, `usize`, `u32`, `u16`, and `u8`; the
  small-width implementations use widened arithmetic plus casts, while
  `u64` routes through runtime helpers, while `isize`/`usize` derive their
  bounds from the selected target width so checked and saturating behavior is
  correct on both 32-bit and 64-bit targets.
- LLVM-text codegen receives an integer overflow mode from compiler options.
  `O0/O1` currently materialize `llvm.*.with.overflow` checks for integer
  `+`, `-`, and `*` and pass the overflow flag to the runtime trap helper;
  `O0/O1` also check integer `/` and `%` divisors through the runtime
  zero-divisor trap helper. The legacy JIT IR path mirrors those debug helper
  calls. `O2/O3` keep plain wrapping/division IR and do not emit unused
  debug-only overflow helper declarations. The opt-in
  `sgc run --cranelift-fast-jit` path now directly emits and executes the
  primitive bool/integer subset as Cranelift IR: O0/O1 arithmetic traps on
  overflow, O2/O3 wraps, and division by zero traps in an isolated process.
- `std::strconv` exposes `Result`-returning `f32`/`f64` parse and
  fixed-precision format helpers alongside the existing `i64` helpers.
- `std::math` defines `Add/Sub/Mul/Div/Rem<Rhs, Output>` and `Neg<Output>`;
  the final type parameter is Sengoo's currently expressible equivalent of an
  associated `Output`. User-defined arithmetic operators select a unique static
  trait impl, including inside generic functions with an exact operator bound.
  Missing impls, ambiguous outputs, malformed operator traits, and method/output
  mismatches have stable diagnostics. Primitive arithmetic retains its direct
  intrinsic path as a compiler-known implementation of the same contract, so
  primitive types also satisfy exact generic operator bounds.
- `std::math` defines mirrored `Into<T>` and `From<T>` widening traits. Since
  `from` is reserved by import syntax, the associated constructor is spelled
  `Target::from_value(source)`. `.into()` uses an annotated `let`, the enclosing
  return type, or a concrete function parameter to select its target. The
  portable lossless matrix covers signed widening, unsigned widening,
  unsigned-to-signed destinations that represent every source value on all
  supported targets, `f32 -> f64`, and safe `isize`/`usize` transitions.
  Target-dependent `u32 -> isize` intentionally has no `From`/`Into` impl.
  Semantic target identity is preserved even where MIR shares an ABI width
  (`i64` versus `isize`, `u64` versus `usize`). Narrowing intentionally has no
  `From`/`Into` impl and remains explicit through `as` or checked helpers.
- `std::math` also exposes trait-bound `numeric_abs`, `numeric_min`,
  `numeric_max`, and `numeric_clamp` helpers for every supported signed,
  unsigned, pointer-sized integer and f32/f64 family. These monomorphize through
  `NumericOrder<T>` and coexist with the source-compatible `abs_i64`,
  `min_i64`, `max_i64`, and `clamp_i64` entry points.

Backend and extension policy:

- LLVM-text plus clang is the production semantic reference. The Cranelift
  fast JIT accepts only its documented primitive subset and rejects calls,
  aggregates, floats, and other unsupported constructs explicitly.
- Operator traits use an explicit `Output` parameter until qualified
  `Self::Output` syntax is added by a future language change.
- Arbitrary-precision integers, decimals, SIMD, and full Cranelift MIR parity
  are separate future capabilities rather than gaps in this numeric contract.

## 2.4 Generics, Traits, Associated Types, And Derive

Sengoo supports generic `def`, `struct`, `enum`, and `impl` declarations with
monomorphized concrete instances. Generic bounds can use direct type parameter
syntax and `where` clauses:

```sg
trait Show {
    def show(self) -> i64;
}

def score<T: Show>(value: T) -> i64 {
    value.show()
}
```

The compiler checks bounds at instantiation sites. If a concrete type does not
implement a required trait, type checking reports the stable
`unsatisfied-trait-bound` diagnostic.

Traits can declare associated types, and generic code can refer to them through
the bounded type parameter:

```sg
trait Iterator {
    type Item;
}

def choose<T: Iterator>(owner: T, value: T::Item) -> T::Item {
    value
}
```

Each `impl Trait for Type` must define the trait's required associated types.
For trait objects, associated types must be fixed in the object type:

```sg
def takes_iter(value: dyn Iterator<Item = i64>) -> i64 {
    0
}
```

Current `dyn Trait` support includes parsing/type checking, object-safety
diagnostics, fixed associated-type validation, and LLVM-text/JIT dynamic
dispatch for single-trait `&self` and `&mut self` receivers through a fat
pointer plus vtable. Vtables carry `drop`, size, and align prefix slots, and
the compiler emits erased drop thunks for concrete implementations, but
source-level owned `dyn Trait` drop/early-drop lowering is still incomplete.
Multi-trait dyn objects, owning `Box<dyn Trait>`, value receivers, and the
native Cranelift path remain roadmap work.

Core trait names are compiler-known for bounds: `Clone`, `Copy`,
`PartialEq`/`Eq`, `PartialOrd`/`Ord`, `Hash`, `Default`, `Display`, `Debug`,
`Iterator`, and `IntoIterator`. Support types `Ordering`, `Formatter`, and
`Hasher` resolve in signatures. `#[derive(...)]` currently registers core trait
impls for the derivable marker surface. Debug formatting is field-aware for
structs and basic enums; `#[derive(Clone)]` and `#[derive(PartialEq)]` on named
structs generate field-aware `clone(&self)` and `eq(&self, other: &Self)`
methods, with struct `==`/`!=` lowering through the generated equality method;
`#[derive(PartialOrd)]` / `#[derive(Ord)]` on named scalar-field structs
generate lexicographic `compare/lt/le/gt/ge` helpers, with `< <= > >=`
lowering through `compare`; `#[derive(Hash)]` generates a deterministic
`hash() -> i64` helper; and `#[derive(Default)]` on named structs generates a
callable `Type::default()` constructor. Scalar fields are handled directly, and
nested fields work when their own derived helper is available. Custom
`impl Hash` bodies may define `hash_into(&self, h: &mut Hasher)`; the compiler
synthesizes `hash() -> i64` by creating a runtime-backed `Hasher`, driving
`hash_into`, and consuming `finish()`. `#[derive(Hash)]` still emits its direct
deterministic `hash()` helper rather than a generated `hash_into` body. Generic
collection-field derives beyond the generated method calls and the general
Formatter object protocol are still under construction. `Copy` is
checked against `Drop`: a type cannot implement both, and a `Copy` type cannot
contain non-`Copy` fields.

Impls follow the package-local orphan rule: an `impl Trait for Type` is allowed
only when the trait or the target type is defined in the current package.

## 2.5 Contracts (`requires` / `ensures`)

```sg
def divide(a: i64, b: i64) -> i64
requires b != 0
ensures result * b == a
{
    a / b
}
```

Current behavior:
- `requires` must be `bool`.
- `ensures` must be `bool`.
- `ensures` can reference `result`.
- Some obvious contradictions are rejected during type-check (for constant-return cases).
- Runtime guards are controlled by `--contract-checks`:
  - `auto`: enabled for `-O 0/1`, disabled for `-O 2/3`
  - `on`: always emit runtime contract checks
  - `off`: never emit runtime contract checks

For AI-assisted workflows, this lets you generate intent first (contract) and implementation second.

Command examples:

```bash
sgc run examples/09_method_call.sg -O 1 --contract-checks auto
sgc run examples/09_method_call.sg -O 2 --contract-checks on
```

## 2.6 Enum payload matches

Payload-carrying enum arms can appear in any match position, and one match can
bind multiple payload-carrying variants:

```sg
enum Event { Number(i64), Pair(i64, bool), Empty }

def main() -> i64 {
    let event = Event::Pair(42, true);
    match event {
        Event::Number(value) => value,
        Event::Pair(number, enabled) => if enabled { number } else { 0 },
        Event::Empty => 0,
    }
}
```

The native conformance gate also covers functions that return enum values and
then match on the returned aggregate.

## 2.7 C FFI (`extern "C"`)

Sengoo supports a focused FFI MVP surface:

- `extern "C" { ... }` declarations
- `pub extern "C" fn ... { ... }` exported functions
- `#[export_name = "..."]` and `#[no_mangle]` on exported extern functions
- compile-time ABI/type checks for FFI signatures

Example:

```sg
extern "C" {
    pub fn c_add(a: i64, b: i64) -> i64;
    pub fn c_strlen(value: &str) -> i64;
}

#[export_name = "sengoo_add_export"]
pub extern "C" fn sengoo_add(a: i64, b: i64) -> i64 {
    a + b
}
```

For end-to-end reproducible commands (Sengoo -> C and C -> Sengoo), see:

- `examples/ffi/README.md`

## 2.8 Async execution

`sgc run` now has a native async path when the entrypoint is `async def main()`.

Currently supported:

- `async def`
- awaiting futures produced by async functions, async blocks, and supported async builtins
- native execution through the runtime bridge used by `sgc run`
- frame-backed async lowering for sequential control flow, `if`, `loop`, and `match`-shaped code paths covered by the current tests
- `sleep(ms)` as an awaitable timer future
- `timeout(future, ms)` as an awaitable `Future<bool>`
- `spawn(future)`
- `spawn_task(future) -> i64`
- `cancel_task(task_id) -> bool`
- `task_status(task_id) -> i64` (`0=unknown`, `1=pending`, `2=completed`, `3=canceled`)
- `join(f1, f2)`
- `select(f1, .., fN)` for 2..8 futures with the same result type, including scalar, tuple, and struct results covered by the current tests
- `select_cancel(f1, .., fN)` for 2..8 homogeneous futures; the first ready future wins and loser futures are canceled/dropped before the call returns

Current limitations:

- `select` remains the non-canceling variant; use `select_cancel` when losing branches must not continue.
- `spawn(future)` still returns an awaitable `Future<T>`; task lifecycle management is exposed separately through `spawn_task/cancel_task/task_status`.
- plain `await Future<T>` returns `T`; cancellation-aware status results are exposed by dedicated futures such as `timeout_cancel`.
- timer support currently covers `sleep` and `timeout`, but not a general timer queue or wheel.
- IO wakeups are limited to the documented reactor subset.
- user-defined awaitables are limited to the documented `Poll<T>` / `AsyncContext` subset.

## 2.9 Ownership, moves, and automatic drop

Sengoo manages memory with move-based ownership and compiler-inserted cleanup
(RAII); there is no garbage collector. A type that has an `impl Drop` is
*affine*: it has a single owner, and the compiler runs cleanup automatically
when the owner goes out of scope.

- **Drop order is reverse declaration order.** When a scope exits, the owning
  locals that still hold a value are dropped last-declared-first. Early exits
  (`return`, `?` propagation) drop exactly the locals that are live on that path,
  tracked with per-binding drop flags, and a value already moved out is not
  dropped again.
- **Moves transfer ownership.** Binding (`let b = a`), passing an owned value by
  value (named-call and method-call arguments), assigning an owned value, and
  returning it all *move* it. After a move the source is dead; reading it is a
  compile error with the stable `use-after-move` diagnostic (also surfaced in
  `sgc --error-format json` and `sglsp`).
- **An active borrow prevents ownership transfer.** Moving an owned root while
  it is still borrowed is rejected with the stable `cannot-move-borrowed`
  diagnostic. Returning a borrowed view derived from a local owner is rejected
  with `borrow-escapes-scope`. The current checker is lexical and intentionally
  conservative.
- **`drop` is compiler-called, not user-called.** The compatibility release
  methods (`.drop()` / `.free()` / `.close()`) run cleanup immediately and mark
  the value moved, so the later automatic drop is suppressed and there is no
  double free.

Current surface: the verified auto-drop and move-checking path covers owned
`String`, current concrete stdlib handles (`Buffer`, `Vec<T>`, `JsonDoc`,
process/net handles), verified `Rc<i64>`/`Rc<bool>`/`Rc<String>` handles, and
compiler-lowered `Rc<T>` storage for monomorphized owning payloads. Some runtime domains still keep
compatibility release methods because older examples and direct FFI-style
stdlib calls use them, but new examples prefer automatic drop.
Drop glue is verified on the LLVM-text/native path and the compiler crate's
LLVM-text JIT emitter. The separate Cranelift fast-JIT now executes a genuine
primitive bool/integer IR subset, but it is not yet a general MIR backend and
does not support calls, aggregates, floats, runtime handles, or drop glue.

## 2.10 Text and Strings

Sengoo has two practical text surfaces today:

- `&str` is the borrowed literal/view type. It supports length, concatenation,
  equality/inequality, comparison trait bounds, `contains`, `starts_with`,
  `ends_with`, and `index_of` through the stdlib string helpers.
- `String` is an owning UTF-8 runtime handle. It is move-only, auto-dropped,
  can be cloned, pushed to, copied into a `Buffer`, and compared with another
  `String` through `eq`/`ne` and byte-order `lt`/`le`/`gt`/`ge`/`compare`
  methods plus `PartialEq`/`Eq`/`PartialOrd`/`Ord` bounds. `String + &str`
  produces a new owned `String`, and `String += &str` appends in place.
  `String.get(start, end)` and `str_get(value, start, end)` copy a byte range
  into a new owned `String` only when both offsets are UTF-8 scalar boundaries;
  invalid ranges return `STATUS_INVALID_ARGUMENT`. The infallible
  `value[start..end]` syntax is available for `String` and `&str`, returns an
  owned `String`, and panics on invalid ranges.
- `String.bytes()` and `String.chars()` create concrete iterators over a copied
  snapshot. `bytes().next()` returns byte values and `chars().next()` returns
  Unicode scalar codepoints as `Option<i64>`. `bytes()` and `chars()` satisfy
  the current generic `Iterator<Item = i64>` bound surface. These iterators
  currently use an explicit `free()` method; source-level
  `Iterator<Item = char>` and split iterators as `Iterator<Item = String>`
  remain future work.
- `char` is represented as a Unicode scalar value in the source language and
  lowers to an `i32` C ABI scalar. `String.push_char(value)` appends the scalar
  as UTF-8 and returns an error-shaped `Result` if the runtime rejects the
  handle or codepoint.

Current stdlib helpers include `str_trim`, `str_to_ascii_upper`, and
`str_to_ascii_lower`, each returning an owned `String`. The case conversion is
deliberately ASCII-only for now; Unicode-aware case folding, normalization, and
locale collation remain future work.

`format` builds an owned `String` from a literal template. The supported subset
includes `{}`, scalar `{:?}`, positional placeholders such as `{1}`, right
alignment such as `{:>8}`, f64 fixed precision such as `{:.2}` / `{:>8.2}`, and
f-string expansion through the same lowering path. `{:?}` also renders current
struct values in field order, for example `Point { x: 7, ok: true }`, and a
user `impl Debug for Type { def to_string(&self) -> String { ... } }` takes
precedence over that structural fallback for structs and enums. Derived enum
Debug renders unit and tuple-payload variants. General Formatter customization
and source-map-perfect f-string diagnostics remain future work.

## 2.11 Opt-in shared ownership with `Rc`

`Rc<T>` is the single-threaded shared-ownership escape hatch. Cloning an `Rc`
increments a non-atomic reference count, and compiler-inserted `Drop` releases
the shared allocation only after the last handle leaves scope. Plain non-`Rc`
owning values remain move-only by default.

Current verified surface:

- `rc_new_i64(value) -> Rc<i64>`
- `rc_new_bool(value) -> Rc<bool>`
- `rc_new_string(value) -> Rc<String>`
- `rc_new<T>(value) -> Rc<T>` for compiler-lowered monomorphized payloads
- `RcValue` for generic `value.rc()` construction over the verified payloads
- `clone()`, `get()`, `strong_count()`, and `is_unique()`
- automatic `Drop` for `Rc<i64>`, `Rc<bool>`, `Rc<String>`, and generic `Rc<T>`

Generic `Rc<T>` stores a moved payload in the runtime control block and invokes
a compiler-generated typed drop thunk when the final clone is released.
Temporary payload expressions are materialized into hidden storage before the
runtime copy, so `rc_new(21)` follows the same move/copy path as
`let value = 21; rc_new(value)`.
`borrow() -> &T` has an initial compiler-known read path for generic payloads:
the runtime exposes the shared payload address and the compiler casts it to the
monomorphized reference type. Borrowed aggregate scalar fields can be read via
`(*rc.borrow()).field`. Moving an `Rc<T>` owner while a borrow produced by
`borrow()` is live is rejected with the stable `cannot-move-borrowed`
diagnostic. Richer projection of owned fields through borrows remains a broader
reference-field/lending limitation. `RcValue` remains the ergonomic generic
entry point for the concrete scalar/string helpers backed by the runtime.

`Rc` deliberately does not collect cycles. If two or more future `Rc`-backed
objects retain each other, that cycle leaks until the process exits. Break such
graphs manually or avoid cyclic ownership; `Rc` is not a tracing garbage
collector.

## 3. Non-Invasive Reflection (Opt-In)

Reflection is designed to avoid polluting the default hot path:

- Disabled by default
- Enabled only with explicit `--reflect`
- Emitted as sidecar metadata (`*.sgreflect.json`)
- Runtime typed invocation API validates signature and argument types
- Native reflection binding can be used when available for lower invoke overhead

## 3.1 CLI example

```bash
# baseline reflected build
sgc build examples/09_method_call.sg -O 2 --reflect

# narrowed reflection scope
sgc build examples/09_method_call.sg -O 2 --reflect \
  --reflect-module examples/09_method_call.sg \
  --reflect-symbol examples/09_method_call.sg::main
```

Generated artifact pattern:

- `<binary>.sgreflect.json` (metadata)
- `<binary>.sgreflect.<dll|so|dylib>` (optional native reflection library)

## 3.2 Runtime API example (Rust)

```rust
use sengoo_runtime::{ReflectionRuntime, ReflectValue};

let rt = ReflectionRuntime::new("target/release/app.sgreflect.json");
let symbols = rt.list_symbols("examples/09_method_call.sg")?;
println!("symbols = {}", symbols.len());

let value = rt.call_i64("examples/09_method_call.sg", "main", &[])?;
println!("result = {}", value);
```

## 3.3 Performance gate example

```bash
cargo run -p sgc -- bench reflection runtime --warmup 1 --iterations 5
python ./scripts/reflection-perf-gate.py --mode soft --sample bench/results/<latest-report>.json
```

## 4. Tooling Workflow

Recommended local loop:

1. Edit source file(s)
2. `sgc check <file.sg>`
3. `sgc run <file.sg> -O 1`
4. For release: `sgc build <file.sg> -O 2`
5. If reflection is needed: add `--reflect` and optional filters

## 4.1 Native Toolchain Contract

The native LLVM backend requires `clang`/LLVM 15 or newer because generated IR
uses the opaque-pointer contract (`ptr`-compatible verifier behavior). The core
conformance CI pins clang 19 and runs the real `sgc` binary against the pinned
examples. When `sgc build` or native `sgc run` detects an older clang, it reports
an actionable toolchain error before surfacing raw LLVM verifier diagnostics.

## 4.2 Native Debug Information

`sgc build <file.sg> -O 0 --debug-info` (or `-g`) emits source locations and
named scalar locals for native debugging. Windows-hosted MSVC builds use
CodeView and produce a `.pdb` beside the executable; Linux/macOS targets retain
DWARF. Cross-host Windows links do not yet promise PDB production. Debug mode
has its own artifact-cache dimension, while builds without `-g` retain
byte-identical debug-metadata-free LLVM IR. See
[`debugging-native.md`](debugging-native.md) for CDB/WinDbg, LLDB, and VS Code
launch workflows plus the current host-evidence boundary.

## 5. Best-Fit Scenarios

- Python services with native-speed hotspot requirements
- CLI/automation tools requiring short edit-build-run loops
- Compiler/runtime experimentation with measurable benchmark gates
- Mixed-language systems that need low-risk incremental migration

---

# Sengoo 语言特性（中文版）

Sengoo 是一门面向工程落地的编译型语言，当前重点：

- Python 互操作与渐进迁移
- 快速编译反馈（增量链路）
- 由 `clang` 编译和链接文本 LLVM IR，并提供 Cranelift 快路径
- 按需开启的非侵入式反射

## 1. 能力快照

| 能力 | 状态 | 说明 |
|---|---|---|
| 核心语法（`def`/`if`/`for`/`while`/`struct`/`impl`） | 可用 | 建议以 `examples/*.sg` 为已验证学习面。 |
| 默认不可变局部变量（重赋值使用 `let mut`） | 可用 | `sgc` JSON 与 `sglsp` 共用 `immutable-assignment`。 |
| 枚举变体作为值 | 可用 | 无 payload 与带 payload 构造均降级为 `match` 使用的表示。 |
| 静态类型检查流水线 | 可用 | 命令：`sgc check <file.sg>`。 |
| 增量编译链路 | 可用 | 基于指纹 + workset 感知失效与重编译。 |
| Daemon 编译服务 | 可用 | `sgc daemon --addr 127.0.0.1:48765`。 |
| Python 互操作运行时路径 | 可用 | 实现在 `runtime/src/python.rs`。 |
| 非侵入式反射 | 可用（按需） | 仅在 `--reflect` 开启时生效，默认路径保持轻量。 |
| VS Code 插件 | 可用 | 当前打包版本 `1.0.0`。 |

## 2. 语法亮点

## 2.1 函数中心语法

```sg
struct Point {
    x: i64,
    y: i64,
}

def add(a: i64, b: i64) -> i64 {
    a + b
}
```

## 2.2 控制流与循环

```sg
def sum(arr: [i64; 4]) -> i64 {
    let mut total = 0;
    for v in arr {
        total = total + v;
    }
    total
}
```

## 2.3 方法调用面

```sg
impl i64 {
    def abs(self) -> i64 {
        if self < 0 { -self } else { self }
    }
}

let x = (-21).abs();
```

## 2.4 契约（`requires` / `ensures`）

```sg
def divide(a: i64, b: i64) -> i64
requires b != 0
ensures result * b == a
{
    a / b
}
```

当前行为：
- `requires` 必须是 `bool`。
- `ensures` 必须是 `bool`。
- `ensures` 可以引用 `result`。
- 对“明显矛盾”的后置条件会在类型检查阶段直接报错（常量返回场景）。
- 运行时检查由 `--contract-checks` 控制：
  - `auto`：`-O 0/1` 开启，`-O 2/3` 关闭
  - `on`：始终开启运行时契约检查
  - `off`：始终关闭运行时契约检查

示例命令：

```bash
sgc run examples/09_method_call.sg -O 1 --contract-checks auto
sgc run examples/09_method_call.sg -O 2 --contract-checks on
```

## 2.5 C FFI（`extern "C"`）

当前 FFI MVP 支持：

- `extern "C" { ... }` 外部声明
- `pub extern "C" fn ... { ... }` 导出函数
- 导出属性：`#[export_name = "..."]`、`#[no_mangle]`
- 编译期 ABI/类型检查（含 unsafe 边界诊断）

示例：

```sg
extern "C" {
    pub fn c_add(a: i64, b: i64) -> i64;
}

#[export_name = "sengoo_add_export"]
pub extern "C" fn sengoo_add(a: i64, b: i64) -> i64 {
    a + b
}
```

可直接复现的双向调用命令（Sengoo -> C / C -> Sengoo）见：

- `examples/ffi/README.md`

## 2.6 所有权、移动与自动 drop

Sengoo 采用基于移动的所有权 + 编译器插入清理（RAII）管理内存，没有垃圾回收器。
带有 `impl Drop` 的类型是*仿射*的：只有唯一所有者，所有者离开作用域时编译器自动执行清理。

- **drop 顺序为声明逆序。** 作用域退出时，仍持有值的所属局部按“后声明先 drop”清理；
  提前退出（`return`、`?` 传播）只 drop 该路径上仍存活的局部，使用每绑定的 drop 标志跟踪，
  已移动走的值不会重复 drop。
- **移动转移所有权。** 绑定（`let b = a`）、按值传递所属值（具名调用与方法调用实参）、
  对所属值赋值，以及返回它，都会*移动*它。移动后源变量失效，再次读取是编译错误，
  诊断码为稳定的 `use-after-move`（同样出现在 `sgc --error-format json` 与 `sglsp`）。
- **活动借用阻止所有权转移。** 所属根变量仍被借用时，按值移动会以稳定诊断码
  `cannot-move-borrowed` 被拒绝。当前检查器按词法作用域工作，因此有意保持保守。
- **drop 由编译器调用，而非用户调用。** 兼容用的释放方法（`.drop()` / `.free()` / `.close()`）
  会立即执行清理并把值标记为已移动，从而抑制后续的自动 drop，不会二次释放。

当前覆盖面：已验证的自动 drop 与移动检查路径覆盖所属 `String`；通用 `Copy`/移动分析、
部分移动，以及其余运行时资源（`Buffer`、`Vec<T>`、`JsonDoc`、进程/网络句柄）的自动 drop
正在逐步落地，因此部分示例仍调用显式释放方法。

## 3. 非侵入式反射（按需开启）

反射设计目标是“不打扰默认热路径”：

- 默认关闭
- 仅显式传入 `--reflect` 才开启
- 以 sidecar 元数据形式输出（`*.sgreflect.json`）
- 运行时提供类型化调用 API，并校验签名与参数
- 在可用时可绑定原生反射动态库以降低调用开销

## 3.1 CLI 示例

```bash
# 基础反射构建
sgc build examples/09_method_call.sg -O 2 --reflect

# 精确筛选模块与符号
sgc build examples/09_method_call.sg -O 2 --reflect \
  --reflect-module examples/09_method_call.sg \
  --reflect-symbol examples/09_method_call.sg::main
```

产物模式：

- `<binary>.sgreflect.json`（元数据）
- `<binary>.sgreflect.<dll|so|dylib>`（可选原生反射动态库）

## 3.2 运行时 API 示例（Rust）

```rust
use sengoo_runtime::{ReflectionRuntime, ReflectValue};

let rt = ReflectionRuntime::new("target/release/app.sgreflect.json");
let symbols = rt.list_symbols("examples/09_method_call.sg")?;
println!("symbols = {}", symbols.len());

let value = rt.call_i64("examples/09_method_call.sg", "main", &[])?;
println!("result = {}", value);
```

## 3.3 性能门禁示例

```bash
cargo run -p sgc -- bench reflection runtime --warmup 1 --iterations 5
python ./scripts/reflection-perf-gate.py --mode soft --sample bench/results/<latest-report>.json
```

## 4. 工具链工作流

推荐本地循环：

1. 编辑源码
2. `sgc check <file.sg>`
3. `sgc run <file.sg> -O 1`
4. 发布构建：`sgc build <file.sg> -O 2`
5. 需要反射时：追加 `--reflect` 与可选筛选参数

## 5. 典型适用场景

- Python 服务中的热点模块加速
- 需要短反馈周期的 CLI/自动化工具
- 具备可观测基准门禁的编译器/运行时工程实验
- 需要低风险渐进迁移的混合语言系统
