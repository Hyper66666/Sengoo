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

Advanced pipeline snapshot (real edits + 100k/1000k scale, measured on **February 16, 2026**):
`bench/results/1771252338862-advanced-pipeline.json`

Real incremental scenarios (`after_avg_ms`, Sengoo):

| Scenario | After avg (ms) |
|---|---:|
| `loop_body_change` | 242.07 |
| `function_signature_change` | 267.02 |
| `add_new_function` | 245.44 |

100k LOC full pipeline (Sengoo):

| Stage | Avg (ms) |
|---|---:|
| Frontend (`compile_frontend_llvm_avg_ms`) | 503.57 |
| Codegen object (`codegen_obj_avg_ms`) | 57.78 |
| Link (`link_avg_ms`) | 492.72 |
| End-to-end (`e2e_avg_ms`) | 1054.08 |

10k-1000k four-language e2e compile comparison (`Sengoo / C++ / Rust / Python`):

| LOC | Sengoo (ms) | C++ (ms) | Rust (ms) | Python (ms) |
|---|---:|---:|---:|---:|
| 10k | 666.99 | 830.18 | 1225.40 | 81.95 |
| 100k | 1054.08 | 1145.91 | 4135.55 | 498.95 |
| 1000k | 6482.95 | 3373.79 | 35292.84 | 5100.83 |

Sengoo 1000k stage split:
- Frontend: `5869.79ms` (`90.54%`)
- Codegen object: `56.19ms` (`0.87%`)
- Link: `556.97ms` (`8.59%`)

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

Sengoo 是一门自研编译型语言，目标聚焦工程落地：

- 强化 Python 互操作，支持渐进迁移
- 提升全量/增量编译反馈速度，缩短开发周期
- 基于 LLVM 生成原生可执行程序
- 提供按需开启的非侵入式反射能力

项目仍在快速迭代，但本地 CLI 开发流程已经可用。

## 为什么选择 Sengoo

## 1）Python 渐进迁移能力

Sengoo 在运行时提供 Python 互操作层（`runtime/src/python.rs`），支持“Python 编排 + Sengoo 热点模块”的混合架构。

互操作基准快照（**2026-02-16**）：
`bench/results/1771230408116-python-interop.json`

| 路径 | Loop 平均耗时 (ms) | 吞吐 (Calls/s) | 相对 Python 原生 |
|---|---:|---:|---:|
| Python 原生 | 2.184 | 9.16M | 基线 |
| Sengoo Runtime (PythonInterop) | 2.665 | 7.50M | +22.02% |
| C++ (CPython C API) | 2.919 | 6.85M | +33.63% |
| Rust (PyO3) | 2.930 | 6.83M | +34.15% |

## 2）增量编译反馈

编译链路重点：

- build/run cache 与模块指纹失效机制
- AST 级编辑分类（`noop` / `impl_only` / `interface_change`）
- workset 感知后端调度
- 可选 daemon 常驻模式

跨语言场景矩阵（**2026-02-16**）：
`bench/results/1771185238357-scenario-matrix.json`

| 指标（平均） | Sengoo | C++ | Rust | Python |
|---|---:|---:|---:|---:|
| 全量编译 (ms) | 835.92 | 1669.41 | 972.98 | 67.48 |
| 增量编辑后编译 (ms) | 33.71 | 1702.23 | 1088.19 | 65.52 |
| 增量收益 (%) | 95.99% | -2.28% | -4.95% | 2.61% |

高级流水线（100k/1000k）快照：
`bench/results/1771246902505-advanced-pipeline.json`（Sengoo 1000k 端到端：`5885.34ms`）

## 3）运行时性能路径

同一矩阵中的 runtime p50 均值：

| 语言 | Runtime p50 平均 (ms) |
|---|---:|
| Sengoo | 8.92 |
| C++ | 8.55 |
| Rust | 8.59 |
| Python | 45.14 |

含义：在该循环密集矩阵中，Sengoo 与 C++/Rust 处于同量级，并明显快于 Python 解释执行。

## 4）非侵入式反射（默认自动）

Sengoo 反射能力采用“默认自动 + 可强制开关”模型：

- 默认 `--reflect=auto`
- 在检测到反射导入时自动启用（`import reflect;` / `import std::reflect;`）
- 显式强制开启：`--reflect` 或 `--reflect=on`
- 显式强制关闭：`--reflect=off`
- 输出 sidecar 元数据（`*.sgreflect.json`）
- 运行时提供类型化调用（`call_i64`/`call_f64`/`call_bool`）
- 可用时走原生反射绑定路径，不可用时自动回退

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
target/release/sgc run examples/01_hello.sg
target/release/sgc build examples/05_loop.sg -O 2
```

## VS Code 插件

- 插件目录：`vscode-sengoo/`
- 当前打包版本：`1.0.0`

## 基准复现

基准套件位于独立仓库：

- `https://github.com/Hyper66666/bench`

常用命令：

```bash
python ./bench/scenario_matrix_bench.py
python ./bench/advanced_pipeline_bench.py
python ./bench/python_interop_bench.py
python ./bench/bootstrap_generality_bench.py
```

## 文档入口

- 教程：`docs/sengoo-tutorial.html`
- 语言特性：`docs/language-features.md`
- 开发手册：`docs/DEVELOPMENT_GUIDE.md`
