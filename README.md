# Sengoo

Sengoo is a self-developed compiled language focused on practical engineering outcomes:

- Python interoperability for gradual migration from existing ecosystems
- Fast full/incremental compile loops for day-to-day development
- Native execution path through an LLVM backend
- Optional non-invasive reflection with sidecar metadata

Sengoo is still in active development, but the CLI workflow is already usable for real local projects.

## Practical Demos (Developer-Oriented)

If you want business-style proof points instead of only synthetic microbenchmarks, run:

```bash
# Sengoo vs Python hot-path runtime demo
python bench/demos/hotpath-risk-scoring/run_demo.py

# Sengoo auto reflection vs C++ manual registry demo
python bench/demos/reflection-auto-vs-cpp/run_demo.py
```

Latest demo snapshots (measured on **February 16, 2026**):

- Hot-path demo report:
  `bench/demos/hotpath-risk-scoring/results/1771254169774-risk-scoring-demo.json`
- Reflection ergonomics demo report:
  `bench/demos/reflection-auto-vs-cpp/results/1771255074700-reflection-auto-vs-cpp.json`

| Demo | Sengoo | Python / C++ |
|---|---:|---:|
| Hot-path runtime avg (ms) | 25.23 | Python: 1285.13 |
| Hot-path speed ratio | 50.93x faster than Python | baseline |
| Reflection rule file LOC | 28 | C++: 55 |
| Manual registry entries | 0 | C++: 2 |
| Missing dynamic rules | 0 | C++: 1 |

## Why Sengoo

## 1) Hybrid Python Migration, Not Rewrite-Only Migration

Sengoo runtime exposes a Python interop layer (see `runtime/src/python.rs`) so teams can keep Python orchestration while moving hot paths to compiled native modules.

Interop benchmark snapshot (measured on **February 16, 2026**):
`bench/results/1771234431756-python-interop.json`

| Runner | Loop avg (ms) | Calls/s | vs Python native |
|---|---:|---:|---:|
| Python native | 0.965 | 5.18M | baseline |
| Sengoo Runtime (PythonInterop) | 0.665 | 7.52M | -31.14% |
| C++ (CPython C API) | 0.718 | 6.97M | -25.65% |
| Rust (PyO3) | 1.069 | 4.68M | +10.74% |

## 2) Fast Feedback Through Incremental Pipeline Reuse

Compiler pipeline focus:

- Build/run cache and module fingerprint invalidation
- AST-aware edit classification (`noop` / `impl_only` / `interface_change`)
- Workset-aware backend orchestration
- Optional daemon mode for persistent process workflows

Cross-language scenario matrix snapshot (measured on **February 16, 2026**):
`bench/results/1771185238357-scenario-matrix.json`

| Metric (avg) | Sengoo | C++ | Rust | Python |
|---|---:|---:|---:|---:|
| Full compile (ms) | 835.92 | 1669.41 | 972.98 | 67.48 |
| Incremental after edit (ms) | 33.71 | 1702.23 | 1088.19 | 65.52 |
| Incremental reduction (%) | 95.99% | -2.28% | -4.95% | 2.61% |

Advanced pipeline snapshot (real edits + 100k/1000k scale, averaged on **February 18, 2026** from two runs):
`bench/results/1771390773767-advanced-pipeline.json` + `bench/results/1771392747911-advanced-pipeline.json`

Real incremental scenarios (`after_avg_ms`, Sengoo):

| Scenario | After avg (ms) |
|---|---:|
| `loop_body_change` | 39.77 |
| `function_signature_change` | 43.81 |
| `add_new_function` | 36.50 |

100k LOC full pipeline (Sengoo):

| Stage | Avg (ms) |
|---|---:|
| Frontend (`compile_frontend_llvm_avg_ms`) | 153.87 |
| Codegen object (`codegen_obj_avg_ms`) | 90.61 |
| Link (`link_avg_ms`) | 173.05 |
| End-to-end (`e2e_avg_ms`) | 417.53 |

10k-1000k four-language e2e compile comparison (`Sengoo / C++ / Rust / Python`):

| LOC | Sengoo (ms) | C++ (ms) | Rust (ms) | Python (ms) |
|---|---:|---:|---:|---:|
| 10k | 372.28 | 693.01 | 2246.86 | 157.18 |
| 100k | 417.53 | 1074.84 | 6625.35 | 832.91 |
| 1000k | 1827.84 | 4883.70 | 54642.47 | 8283.46 |


10k-1000k compile peak-memory comparison (RSS MB, compile-stage only, lower is better):

| LOC | Sengoo | C++ | Rust | Python |
|---|---:|---:|---:|---:|
| 10k | 18.88 | 75.68 | 70.84 | 41.40 |
| 100k | 140.18 | 118.50 | 337.86 | 288.46 |
| 1000k | 1367.99 | 435.22 | 2681.55 | 2610.90 |

Sengoo vs C++ RSS ratio: 10k `x0.25`, 100k `x1.18`, 1000k `x3.14`.

Sengoo 1000k stage split:
- Frontend: `1589.02ms` (`86.93%`)
- Codegen object: `76.77ms` (`4.20%`)
- Link: `162.04ms` (`8.86%`)

## 3) Runtime-Class Performance Track

Scenario runtime p50 average (same matrix file `1771185238357`):

| Language | Runtime p50 avg (ms) |
|---|---:|
| Sengoo | 8.92 |
| C++ | 8.55 |
| Rust | 8.59 |
| Python | 45.14 |

Interpretation:

- Sengoo runtime behavior is currently in the same class as C++/Rust in this loop-heavy matrix profile.
- In these samples, Sengoo is significantly faster than Python runtime execution.

## 4) Non-Invasive Reflection (Auto by Default)

Reflection in Sengoo is designed for low baseline overhead with an auto mode:

- Default mode is `--reflect=auto`
- Auto mode enables reflection only when reflect imports are detected (`import reflect;` / `import std::reflect;`)
- Force enable with `--reflect` or `--reflect=on`
- Force disable with `--reflect=off`
- Metadata emitted to sidecar JSON (`*.sgreflect.json`)
- Typed runtime invocation (`call_i64`/`call_f64`/`call_bool`) with signature checks
- Native reflection binding path is used when available (fallback handler path is retained)

Reflection build example:

```bash
sgc build examples/09_method_call.sg -O 2
```

Fine-grained reflection selection:

```bash
sgc build examples/09_method_call.sg -O 2 --reflect=on \
  --reflect-module examples/09_method_call.sg \
  --reflect-symbol examples/09_method_call.sg::main
```

Runtime reflection usage example (Rust):

```rust
use sengoo_runtime::{ReflectValue, ReflectionRuntime};

let rt = ReflectionRuntime::new("target/release/app.sgreflect.json");
let symbols = rt.list_symbols("examples/09_method_call.sg")?;
println!("symbols = {}", symbols.len());

let value = rt.call_i64("examples/09_method_call.sg", "main", &[])?;
println!("result = {}", value);
```

Reflection overhead benchmark:

```bash
cargo run -p sgc -- bench reflection runtime --warmup 1 --iterations 5
python ./scripts/reflection-perf-gate.py --mode soft --sample bench/results/<latest-reflection-report>.json
```

Reflection benchmark cases:

- `disabled`: compile with reflection fully off (baseline path)
- `enabled-unused`: compile with `--reflect=on`, runtime reflection API not called
- `enabled-used`: compile with `--reflect=on`, perform runtime symbol listing and typed reflection invoke

Current gate defaults:

- `soft`: enabled-unused overhead <= `25%`, enabled-used overhead <= `45%`
- `hard`: enabled-unused overhead <= `15%`, enabled-used overhead <= `30%`
- disabled regression check compares against `bench/baseline.json` key `reflection/<suite>/disabled` when available

## Quick Start

```bash
cargo build --release
```

```bash
target/release/sgc run examples/01_hello.sg
```

```bash
target/release/sgc build examples/05_loop.sg -O 2
```

Useful commands:

```bash
# type check
sgc check <file.sg>

# compile and run
sgc run <file.sg> -O 1

# build native binary
sgc build <file.sg> -O 2

# force full rebuild
sgc build <file.sg> -O 2 --force-rebuild

# optional daemon mode
sgc daemon --addr 127.0.0.1:48765
```

## VS Code Extension

- Extension package location: `vscode-sengoo/`
- Current package version: `1.0.0`

## Benchmark Reproducibility

Benchmark suites are maintained in a separate repository:

- `https://github.com/Hyper66666/bench`

Common commands:

```bash
python ./bench/scenario_matrix_bench.py
python ./bench/advanced_pipeline_bench.py
python ./bench/python_interop_bench.py
python ./bench/bootstrap_generality_bench.py
```

Fairness profile used in advanced pipeline comparison:

- C++: precompiled header enabled
- Rust: cargo incremental enabled (`CARGO_INCREMENTAL=1`)

## Documentation

- Tutorial: `docs/sengoo-tutorial.html`
- Language features: `docs/language-features.md`
- Development guide: `docs/DEVELOPMENT_GUIDE.md`

## Repository Layout

```text
Sengoo/
|-- compiler/        # Frontend, type checker, HIR/MIR pipeline
|-- runtime/         # Runtime support, Python interop, reflection runtime API
|-- tools/
|   |-- sgc/         # Compiler CLI
|   |-- sgfmt/       # Formatter
|   `-- sglsp/       # Language server
|-- examples/        # Language examples
|-- docs/            # Tutorial and developer docs
`-- vscode-sengoo/   # VS Code extension
```

## Project Status

Current stage: early but fast-iterating.

Current focus:

- Frontend architecture optimization
- Stronger incremental consistency under real edits
- Better interop/reflection ergonomics
- Tooling and developer experience polish

Notes:

- All benchmark numbers above are local-machine measurements and should be treated as trend indicators.
- Use the benchmark repository and CI gates to verify performance on your own hardware.

---

# Sengoo（中文版）

Sengoo 是一门自研编译型语言，聚焦实际工程落地：

- 强化 Python 互操作，支持渐进迁移
- 提升全量/增量编译反馈速度，缩短开发迭代周期
- 基于 LLVM 生成原生可执行产物
- 提供默认自动的非侵入式反射（sidecar 元数据）

项目仍在快速迭代，但本地 CLI 开发流程已经可用。

## 实用 Demo（面向开发者）

如果你希望看到业务风格的可落地证明，而不是仅有合成微基准，可直接运行：

```bash
# Sengoo vs Python 热点性能 Demo
python bench/demos/hotpath-risk-scoring/run_demo.py

# Sengoo 自动反射 vs C++ 手工注册 Demo
python bench/demos/reflection-auto-vs-cpp/run_demo.py
```

最新快照（测量日期：**2026-02-16**）：

- 热点 Demo 报告：
  `bench/demos/hotpath-risk-scoring/results/1771254169774-risk-scoring-demo.json`
- 反射工程性 Demo 报告：
  `bench/demos/reflection-auto-vs-cpp/results/1771255074700-reflection-auto-vs-cpp.json`

| Demo | Sengoo | Python / C++ |
|---|---:|---:|
| 热点路径运行时均值 (ms) | 25.23 | Python: 1285.13 |
| 热点路径速度比 | 比 Python 快 50.93x | 基线 |
| 反射规则文件 LOC | 28 | C++: 55 |
| 手工注册条目数 | 0 | C++: 2 |
| 动态规则缺失数 | 0 | C++: 1 |

## 为什么选择 Sengoo

## 1) 混合式 Python 迁移，而非一次性重写

Sengoo 在运行时提供 Python 互操作层（见 `runtime/src/python.rs`），支持“Python 编排 + Sengoo 热点模块”的混合架构。

互操作基准快照（测量日期：**2026-02-16**）：
`bench/results/1771234431756-python-interop.json`

| 路径 | Loop 平均耗时 (ms) | 吞吐 (Calls/s) | 相对 Python 原生 |
|---|---:|---:|---:|
| Python 原生 | 0.965 | 5.18M | 基线 |
| Sengoo Runtime (PythonInterop) | 0.665 | 7.52M | -31.14% |
| C++ (CPython C API) | 0.718 | 6.97M | -25.65% |
| Rust (PyO3) | 1.069 | 4.68M | +10.74% |

## 2) 快速反馈的增量编译链路

编译链路重点：

- build/run 缓存与模块指纹失效机制
- AST 感知编辑分类（`noop` / `impl_only` / `interface_change`）
- workset 感知后端调度
- 可选 daemon 常驻模式

跨语言场景矩阵（测量日期：**2026-02-16**）：
`bench/results/1771185238357-scenario-matrix.json`

| 指标（平均） | Sengoo | C++ | Rust | Python |
|---|---:|---:|---:|---:|
| 全量编译 (ms) | 835.92 | 1669.41 | 972.98 | 67.48 |
| 增量编辑后编译 (ms) | 33.71 | 1702.23 | 1088.19 | 65.52 |
| 增量收益 (%) | 95.99% | -2.28% | -4.95% | 2.61% |

高级流水线快照（真实编辑 + 100k/1000k 规模，测量日期：**2026-02-16**）：
`bench/results/1771390773767-advanced-pipeline.json` + `bench/results/1771392747911-advanced-pipeline.json`

真实增量场景（`after_avg_ms`，Sengoo）：

| 场景 | 平均耗时 (ms) |
|---|---:|
| `loop_body_change` | 39.77 |
| `function_signature_change` | 43.81 |
| `add_new_function` | 36.50 |

100k LOC 全流程（Sengoo）：

| 阶段 | 平均耗时 (ms) |
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


10k-1000k 编译峰值内存对比（RSS MB，仅编译阶段，越低越好）：

| LOC | Sengoo | C++ | Rust | Python |
|---|---:|---:|---:|---:|
| 10k | 18.88 | 75.68 | 70.84 | 41.40 |
| 100k | 140.18 | 118.50 | 337.86 | 288.46 |
| 1000k | 1367.99 | 435.22 | 2681.55 | 2610.90 |

Sengoo 相对 C++ 的 RSS 比例：10k `x0.25`，100k `x1.18`，1000k `x3.14`。

Sengoo 1000k 阶段占比：
- Frontend: `1589.02ms`（`86.93%`）
- Codegen object: `76.77ms`（`4.20%`）
- Link: `162.04ms`（`8.86%`）

## 3) 运行时性能等级

场景 runtime p50 平均（同一矩阵文件 `1771185238357`）：

| 语言 | Runtime p50 平均 (ms) |
|---|---:|
| Sengoo | 8.92 |
| C++ | 8.55 |
| Rust | 8.59 |
| Python | 45.14 |

解读：

- 在该循环密集型矩阵中，Sengoo 与 C++/Rust 处于同一量级。
- 在这些样本里，Sengoo 运行时显著快于 Python 解释执行。

## 4) 非侵入式反射（默认自动）

Sengoo 反射能力采用“默认自动 + 可强制开关”模型：

- 默认 `--reflect=auto`
- 检测到反射导入时自动启用（`import reflect;` / `import std::reflect;`）
- 显式强制开启：`--reflect` 或 `--reflect=on`
- 显式强制关闭：`--reflect=off`
- 输出 sidecar 元数据（`*.sgreflect.json`）
- 提供类型化调用（`call_i64` / `call_f64` / `call_bool`）
- 原生反射绑定路径可用时优先使用，不可用时回退

反射构建示例：

```bash
sgc build examples/09_method_call.sg -O 2
```

细粒度筛选示例：

```bash
sgc build examples/09_method_call.sg -O 2 --reflect=on \
  --reflect-module examples/09_method_call.sg \
  --reflect-symbol examples/09_method_call.sg::main
```

运行时调用示例（Rust）：

```rust
use sengoo_runtime::{ReflectValue, ReflectionRuntime};

let rt = ReflectionRuntime::new("target/release/app.sgreflect.json");
let symbols = rt.list_symbols("examples/09_method_call.sg")?;
println!("symbols = {}", symbols.len());

let value = rt.call_i64("examples/09_method_call.sg", "main", &[])?;
println!("result = {}", value);
```

反射性能门禁：

```bash
cargo run -p sgc -- bench reflection runtime --warmup 1 --iterations 5
python ./scripts/reflection-perf-gate.py --mode soft --sample bench/results/<latest-reflection-report>.json
```

## 快速开始

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

# 编译并运行
sgc run <file.sg> -O 1

# 编译为原生二进制
sgc build <file.sg> -O 2

# 强制全量重建
sgc build <file.sg> -O 2 --force-rebuild

# 可选 daemon 模式
sgc daemon --addr 127.0.0.1:48765
```

## VS Code 插件

- 插件目录：`vscode-sengoo/`
- 当前打包版本：`1.0.0`

## 基准复现

基准套件维护在独立仓库：

- `https://github.com/Hyper66666/bench`

常用命令：

```bash
python ./bench/scenario_matrix_bench.py
python ./bench/advanced_pipeline_bench.py
python ./bench/python_interop_bench.py
python ./bench/bootstrap_generality_bench.py
```

高级流水线公平性配置：

- C++：启用预编译头（PCH）
- Rust：启用 cargo incremental（`CARGO_INCREMENTAL=1`）

## 文档入口

- 教程：`docs/sengoo-tutorial.html`
- 语言特性：`docs/language-features.md`
- 开发手册：`docs/DEVELOPMENT_GUIDE.md`

## 仓库结构

```text
Sengoo/
|-- compiler/        # 前端、类型检查、HIR/MIR 流水线
|-- runtime/         # 运行时支持、Python 互操作、反射运行时 API
|-- tools/
|   |-- sgc/         # 编译器 CLI
|   |-- sgfmt/       # 格式化工具
|   `-- sglsp/       # 语言服务器
|-- examples/        # 语言示例
|-- docs/            # 教程与开发文档
`-- vscode-sengoo/   # VS Code 扩展
```

## 项目状态

当前阶段：早期，但在高速迭代。

当前重点：

- 前端架构优化
- 真实编辑下更强的一致性增量编译
- 互操作与反射体验持续打磨
- 工具链与开发者体验完善

说明：

- 上述基准均为本机测量值，应作为趋势信号而非绝对结论。
- 请结合 bench 仓库与 CI gate 在你的硬件环境上复验。
