# Sengoo Language Features

Sengoo is a compiled language focused on practical engineering workflows:

- Hybrid Python interoperability for gradual migration
- Fast compile feedback with incremental pipeline reuse
- LLVM-native code generation and executable output
- Optional non-invasive reflection with sidecar metadata

## 1. Current Capability Snapshot

| Capability | Status | Notes |
|---|---|---|
| Core syntax (`def`, `if`, `for`, `while`, `struct`, `impl`) | Available | Use `examples/*.sg` as validated learning surface. |
| Static type-check pipeline | Available | Entry command: `sgc check <file.sg>`. |
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
    let total = 0;
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
- LLVM 原生代码生成
- 按需开启的非侵入式反射

## 1. 能力快照

| 能力 | 状态 | 说明 |
|---|---|---|
| 核心语法（`def`/`if`/`for`/`while`/`struct`/`impl`） | 可用 | 建议以 `examples/*.sg` 为已验证学习面。 |
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
    let total = 0;
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
