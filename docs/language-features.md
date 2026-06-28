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

Current `dyn Trait` support is a frontend/type-checking skeleton. Object-safety
diagnostics and fixed associated-type validation are implemented, but vtable
representation and runtime dynamic dispatch are still roadmap work.

Core trait names are compiler-known for bounds: `Clone`, `Copy`,
`PartialEq`/`Eq`, `PartialOrd`/`Ord`, `Hash`, `Default`, `Display`, `Debug`,
`Iterator`, and `IntoIterator`. Support types `Ordering`, `Formatter`, and
`Hasher` resolve in signatures. `#[derive(...)]` currently registers core trait
impls for the derivable marker surface, while field-aware clone/compare/hash/
debug/default behavior is still under construction. `Copy` is checked against
`Drop`: a type cannot implement both, and a `Copy` type cannot contain
non-`Copy` fields.

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
  diagnostic. The current checker is lexical and intentionally conservative.
- **`drop` is compiler-called, not user-called.** The compatibility release
  methods (`.drop()` / `.free()` / `.close()`) run cleanup immediately and mark
  the value moved, so the later automatic drop is suppressed and there is no
  double free.

Current surface: the verified auto-drop and move-checking path covers owned
`String`, current concrete stdlib handles (`Buffer`, `Vec<T>`, `JsonDoc`,
process/net handles), and verified `Rc<i64>`/`Rc<bool>`/`Rc<String>` handles.
Some runtime domains still keep
compatibility release methods because older examples and direct FFI-style
stdlib calls use them, but new examples prefer automatic drop.

## 2.10 Text and Strings

Sengoo has two practical text surfaces today:

- `&str` is the borrowed literal/view type. It supports length, concatenation,
  equality/inequality, `contains`, `starts_with`, `ends_with`, and `index_of`
  through the stdlib string helpers.
- `String` is an owning UTF-8 runtime handle. It is move-only, auto-dropped,
  can be cloned, pushed to, copied into a `Buffer`, and compared with another
  `String`.
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
f-string expansion through the same lowering path. Structure-aware Debug,
general Formatter customization, and source-map-perfect f-string diagnostics
remain future work.

## 2.11 Opt-in shared ownership with `Rc`

`Rc<T>` is the single-threaded shared-ownership escape hatch. Cloning an `Rc`
increments a non-atomic reference count, and compiler-inserted `Drop` releases
the shared allocation only after the last handle leaves scope. Plain non-`Rc`
owning values remain move-only by default.

Current verified surface:

- `rc_new_i64(value) -> Rc<i64>`
- `rc_new_bool(value) -> Rc<bool>`
- `rc_new_string(value) -> Rc<String>`
- `RcValue` for generic `value.rc()` construction over the verified payloads
- `clone()`, `get()`, `strong_count()`, and `is_unique()`
- automatic `Drop` for `Rc<i64>`, `Rc<bool>`, and `Rc<String>`

Arbitrary user-defined `Rc<T>` storage/deref still needs a stable value
representation and drop ABI. Until then, `RcValue` is the supported generic
entry point for the concrete payloads backed by the runtime.

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
