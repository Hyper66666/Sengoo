# Sengoo

[English](README.md) | [简体中文](README.zh-CN.md)

Sengoo 是一门自研编译型语言，重点放在工程实践结果上：

- 通过 Python 互操作支持从现有生态渐进迁移
- 为日常开发提供快速的全量与增量编译反馈
- 通过 LLVM 后端提供原生执行路径
- 通过 sidecar 元数据提供可选的非侵入式反射能力

Sengoo 仍处于积极开发阶段，但 CLI 工作流已经可以用于真实本地项目。

## AI 交接包（给其他 LLM）

如果要让另一个模型快速接手，优先给它这些内容：

- `docs/AI_FEWSHOT_PLAYBOOK.md`，包含少样本示例、代码和设计理由
- `sgc --error-format json ...`，提供机器可读诊断
- 函数契约（`requires` / `ensures`），在实现前先表达意图
- `--contract-checks auto|on|off`，决定是否插入运行时契约守卫

契约示例：

```sg

def divide(a: i64, b: i64) -> i64
requires b != 0
ensures result * b == a
{
    a / b
}
```

## 实用 Demo（面向开发者）

如果你想看更偏业务证明风格的案例，而不只是合成微基准，可以运行：

```bash
# Sengoo vs Python 热路径运行时演示
python bench/demos/hotpath-risk-scoring/run_demo.py

# Sengoo 自动反射 vs C++ 手写注册表示例
python bench/demos/reflection-auto-vs-cpp/run_demo.py
```

最新 demo 快照（测量时间为 **2026 年 2 月 16 日**）：

- 热路径 demo 报告：
  `bench/demos/hotpath-risk-scoring/results/1771254169774-risk-scoring-demo.json`
- 反射易用性 demo 报告：
  `bench/demos/reflection-auto-vs-cpp/results/1771255074700-reflection-auto-vs-cpp.json`

| Demo | Sengoo | Python / C++ |
|---|---:|---:|
| 热路径平均运行时间 (ms) | 25.23 | Python: 1285.13 |
| 热路径速度比 | 比 Python 快 50.93x | 基线 |
| 反射规则文件 LOC | 28 | C++: 55 |
| 手写注册条目数 | 0 | C++: 2 |
| 缺失动态规则数 | 0 | C++: 1 |

## 为什么是 Sengoo

### 1）混合式 Python 迁移，而不是只支持重写式迁移

Sengoo runtime 暴露了 Python 互操作层（见 `runtime/src/python.rs`），因此团队可以保留 Python 编排层，同时把热路径迁移到原生编译模块。

互操作基准快照（测量时间为 **2026 年 2 月 16 日**）：
`bench/results/1771234431756-python-interop.json`

| Runner | 平均循环时间 (ms) | 每秒调用数 | 相比 Python 原生 |
|---|---:|---:|---:|
| Python native | 0.965 | 5.18M | 基线 |
| Sengoo Runtime (PythonInterop) | 0.665 | 7.52M | -31.14% |
| C++ (CPython C API) | 0.718 | 6.97M | -25.65% |
| Rust (PyO3) | 1.069 | 4.68M | +10.74% |

### 2）通过增量流水线复用获得快速反馈

编译器流水线目前重点在：

- build/run cache 与模块指纹失效机制
- AST 感知的编辑分类（`noop` / `impl_only` / `interface_change`）
- workset 感知的后端编排
- 可选 daemon 模式，用于持久化进程工作流

跨语言场景矩阵快照（测量时间为 **2026 年 2 月 16 日**）：
`bench/results/1771185238357-scenario-matrix.json`

| 指标（平均） | Sengoo | C++ | Rust | Python |
|---|---:|---:|---:|---:|
| 全量编译 (ms) | 835.92 | 1669.41 | 972.98 | 67.48 |
| 编辑后增量编译 (ms) | 33.71 | 1702.23 | 1088.19 | 65.52 |
| 增量降幅 (%) | 95.99% | -2.28% | -4.95% | 2.61% |

高级流水线快照（真实编辑 + 100k/1000k 规模，在 **2026 年 2 月 18 日** 两次运行平均）：
`bench/results/1771390773767-advanced-pipeline.json` + `bench/results/1771392747911-advanced-pipeline.json`

真实增量场景（Sengoo 的 `after_avg_ms`）：

| 场景 | 平均值 (ms) |
|---|---:|
| `loop_body_change` | 39.77 |
| `function_signature_change` | 43.81 |
| `add_new_function` | 36.50 |

100k LOC 全流程（Sengoo）：

| 阶段 | 平均值 (ms) |
|---|---:|
| Frontend (`compile_frontend_llvm_avg_ms`) | 153.87 |
| Codegen object (`codegen_obj_avg_ms`) | 90.61 |
| Link (`link_avg_ms`) | 173.05 |
| End-to-end (`e2e_avg_ms`) | 417.53 |

10k-1000k 四语言 e2e 编译对比（`Sengoo / C++ / Rust / Python`）：

| LOC | Sengoo (ms) | C++ (ms) | Rust (ms) | Python (ms) |
|---|---:|---:|---:|---:|
| 10k | 372.28 | 693.01 | 2246.86 | 157.18 |
| 100k | 417.53 | 1074.84 | 6625.35 | 832.91 |
| 1000k | 1827.84 | 4883.70 | 54642.47 | 8283.46 |

低内存模式 e2e 快照（相同 1000k 工作负载，测量时间为 **2026 年 2 月 18 日**）：

| 模式 | 1000k e2e 平均值 (ms) | 峰值 RSS (MB) |
|---|---:|---:|
| 默认（`sgc build`） | 2331.39 | 1418.61 |
| 低内存（`sgc build --low-memory`） | 1737.71 | 672.10 |

低内存模式收益：
- 在这个 1000k 案例里峰值内存降低约 `52.62%`。
- 由于更激进的不可达函数裁剪，同一案例下 e2e 编译时间提升约 `25.46%`。

低内存模式代价：
- 会禁用或绕过部分增量缓存与会话复用。
- 使用单线程 frontend，并降低 MIR 优化上限。
- 在较小项目或热增量循环里，可能比默认模式更慢。

启用方式：

```bash
sgc build your_file.sg --low-memory
sgc run your_file.sg --low-memory
```

10k-1000k 编译峰值内存对比（RSS MB，仅编译阶段，越低越好）：

| LOC | Sengoo | C++ | Rust | Python |
|---|---:|---:|---:|---:|
| 10k | 18.88 | 75.68 | 70.84 | 41.40 |
| 100k | 140.18 | 118.50 | 337.86 | 288.46 |
| 1000k | 1367.99 | 435.22 | 2681.55 | 2610.90 |

Sengoo 相对 C++ 的 RSS 比值：10k `x0.25`，100k `x1.18`，1000k `x3.14`。

Sengoo 在 1000k 下的阶段拆分：
- Frontend：`1589.02ms`（`86.93%`）
- Codegen object：`76.77ms`（`4.20%`）
- Link：`162.04ms`（`8.86%`）

### 3）接近系统语言等级的运行时性能

场景运行时 p50 平均值（同一矩阵文件 `1771185238357`）：

| 语言 | Runtime p50 avg (ms) |
|---|---:|
| Sengoo | 8.92 |
| C++ | 8.55 |
| Rust | 8.59 |
| Python | 45.14 |

解读：

- 在当前偏循环密集型矩阵场景里，Sengoo 的运行时表现已经接近 C++ / Rust 这一档。
- 在这些样本中，Sengoo 的运行时执行明显快于 Python。

### 4）非侵入式反射（默认自动模式）

Sengoo 的反射设计目标是低基线开销，并通过自动模式尽量减少侵入：

- 默认模式为 `--reflect=auto`
- 只有检测到 reflect import（`import reflect;` / `import std::reflect;`）时才自动开启
- 可通过 `--reflect` 或 `--reflect=on` 强制开启
- 可通过 `--reflect=off` 强制关闭
- 元数据输出为 sidecar JSON（`*.sgreflect.json`）
- 提供带签名检查的类型化运行时调用（`call_i64` / `call_f64` / `call_bool`）
- 在可用时优先走原生 reflection binding 路径，同时保留 fallback handler 路径

反射构建示例：

```bash
sgc build examples/09_method_call.sg -O 2
```

细粒度反射选择：

```bash
sgc build examples/09_method_call.sg -O 2 --reflect=on \
  --reflect-module examples/09_method_call.sg \
  --reflect-symbol examples/09_method_call.sg::main
```

运行时反射使用示例（Rust）：

```rust
use sengoo_runtime::{ReflectValue, ReflectionRuntime};

let rt = ReflectionRuntime::new("target/release/app.sgreflect.json");
let symbols = rt.list_symbols("examples/09_method_call.sg")?;
println!("symbols = {}", symbols.len());

let value = rt.call_i64("examples/09_method_call.sg", "main", &[])?;
println!("result = {}", value);
```

反射开销基准：

```bash
cargo run -p sgc -- bench reflection runtime --warmup 1 --iterations 5
python ./scripts/reflection-perf-gate.py --mode soft --sample bench/results/<latest-reflection-report>.json
```

反射基准场景：

- `disabled`：完全关闭反射编译，作为基线路径
- `enabled-unused`：使用 `--reflect=on` 编译，但运行时不调用反射 API
- `enabled-used`：使用 `--reflect=on` 编译，并进行运行时符号枚举和类型化反射调用

当前 gate 默认值：

- `soft`：enabled-unused 开销 <= `25%`，enabled-used 开销 <= `45%`
- `hard`：enabled-unused 开销 <= `15%`，enabled-used 开销 <= `30%`
- Disabled 回归检查会在可用时对比 `bench/baseline.json` 中 `reflection/<suite>/disabled` 键

### 5）运行时集成栈

最新一轮 runtime 演进增加了可复用的 FFI 包装层和集成工具：

- 数据库 runtime MVP（`runtime/src/reflect/runtime_db.rs`）
  - 生命周期：`open` / `close` / `ping`
  - 查询路径：`exec` / `query`，支持结果句柄
  - 提供结构化状态和错误消息通道，不再只用 `-1/0`
- 完整的 C / C++ 包装路径（`runtime/src/reflect/runtime_ffi.rs`）
  - C 库 open / call / close
  - C++ 对象生命周期包装：create / call / destroy
  - 回调桥：bind / dispatch / unbind
  - 二进制负载桥：托管 buffer handle（`new/from/len/ptr/copy_in/copy_out/free`）
- Lua 桥
  - 轻量运行时子集（`sengoo_lua_*`）
  - 原生 Lua 5.4 动态库桥接 PoC（`sengoo_lua54_*`）
- 集成验证通道
  - Protobuf wire encode/decode FFI 路径（`runtime_proto`）
  - 带 p50/p95/p99 指标的网络 bench runtime 路径（`runtime_net_bench`）

相关文档：
- `docs/runtime-ffi-lua.md`
- `docs/database-runtime.md`
- `docs/runtime-protobuf-ffi.md`
- `docs/runtime-network-bench.md`

## 构建与运行

```bash
cargo build --release
```

```bash
target/release/sgc run examples/01_hello.sg
```

```bash
target/release/sgc build examples/05_loop.sg -O 2
```

常用命令：

```bash
# 类型检查
sgc check <file.sg>

# 类型检查（给自动化和代理使用的 JSON 诊断）
sgc --error-format json check <file.sg>

# 编译并运行
sgc run <file.sg> -O 1

# 编译并运行，同时插入运行时契约守卫（auto：O0/O1 开，O2/O3 关）
sgc run <file.sg> -O 1 --contract-checks auto

# 构建原生二进制
sgc build <file.sg> -O 2

# 强制全量重建
sgc build <file.sg> -O 2 --force-rebuild

# 可选 daemon 模式
sgc daemon --addr 127.0.0.1:48765
```

## Async ??

???? `async def main()` ??`sgc run` ????? async ?????

?????

- `async def`
- `await async_fn(...)`
- `async { ... }` ????????????
- ?? `sgc run` ?? runtime bridge ???? async
- ???????? frame-backed async lowering????????? `if`?`loop`?`match` ????
- ?? `await` ? `sleep(ms)` ?? future
- ?? `await` ? `timeout(future, ms)`??? `Future<bool>`
- `spawn(future)`
- `spawn_task(future) -> i64`
- `cancel_task(task_id) -> bool`
- `task_status(task_id) -> i64`?`0=unknown`?`1=pending`?`2=completed`?`3=canceled`?
- `join(f1, f2)`
- ???????????? future ? `select(f1, f2)`??? `Future<bool>`?`Future<i8/i16/i32/i64>`?`Future<f32/f64>`

?????

- `select` ?????????????????? future ??????? `bool`??????
- `select` ????? future ??????
- `spawn(future)` ????? `await` ? `Future<T>`??????????? `spawn_task/cancel_task/task_status` ????
- timer ???? `sleep` ? `timeout`??????? timer queue / wheel
- ??? IO wakeup
- ???????? awaitable?????? trait-based Future ??

?????

```sg
async def add1(x: i64) -> i64 {
    x + 1
}

async def main() -> i64 {
    let task = spawn(add1(41));
    let value = select(task, add1(1));
    print(value);
    value
}
```

?????????

```sg
async def child() -> i64 {
    await sleep(5);
    7
}

async def main() -> i64 {
    let task = spawn_task(child());
    let before = task_status(task);
    await sleep(10);
    let after = task_status(task);
    if before == 1 && after == 2 { 42 } else { 0 }
}
```

Timer ???

```sg
async def work() -> i64 {
    42
}

async def main() -> i64 {
    let task = work();
    let ready = await timeout(task, 10);
    if ready {
        await task
    } else {
        0
    }
}
```

## VS Code 扩展

- 扩展包位置：`vscode-sengoo/`
- 当前包版本：`1.0.0`

## Benchmark 可复现性

基准套件维护在独立仓库：

- `https://github.com/Hyper66666/bench`

常用命令：

```bash
python ./bench/scenario_matrix_bench.py
python ./bench/advanced_pipeline_bench.py
python ./bench/python_interop_bench.py
python ./bench/bootstrap_generality_bench.py
```

高级流水线比较中使用的公平性配置：

- C++：启用预编译头
- Rust：启用 cargo incremental（`CARGO_INCREMENTAL=1`）

## 文档

- 教程：`docs/sengoo-tutorial.html`
- 语言特性：`docs/language-features.md`
- 开发指南：`docs/DEVELOPMENT_GUIDE.md`

## 仓库结构

```text
Sengoo/
|-- compiler/        # Frontend、类型检查器、HIR/MIR 流水线
|-- runtime/         # 运行时支持、Python 互操作、反射运行时 API
|-- tools/
|   |-- sgc/         # 编译器 CLI
|   |-- sgfmt/       # 格式化工具
|   `-- sglsp/       # 语言服务器
|-- examples/        # 语言示例
|-- docs/            # 教程和开发文档
`-- vscode-sengoo/   # VS Code 扩展
```

## 项目状态

当前阶段：仍然偏早期，但迭代速度很快。

当前重点：

- async phase-2 能力扩展与 runtime 语义完善
- 真实编辑场景下更强的增量一致性
- 更好的 interop 与 reflection 易用性
- 工具链和开发体验打磨

说明：

- 上述所有 benchmark 数字都来自本地机器测量，应视为趋势指标。
- 要在你自己的硬件上验证性能，请结合 benchmark 仓库和 CI gate 一起使用。
