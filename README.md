# Sengoo

Sengoo is a self-developed compiled language focused on practical engineering outcomes:

- Python interoperability for gradual migration from existing ecosystems
- Fast full/incremental compile loops for day-to-day development
- Native execution path through an LLVM backend

Sengoo is still in active development, but the CLI workflow is already usable for real local projects.

## Who Should Evaluate Sengoo

- Teams with Python-heavy services that need selective native-speed acceleration
- Developers who want short edit-build-run loops without leaving native binaries
- Compiler/tooling engineers who care about measurable incremental architecture

## Core Strengths

### 1) Python Interoperability for Hybrid Architectures

Sengoo runtime provides a Python interop layer (implemented in `runtime/src/python.rs`) so teams can keep Python orchestration while moving hot paths to compiled modules.

Latest local interop benchmark (measured on **February 16, 2026**):
`bench/results/1771230408116-python-interop.json`

| Runner | Loop avg (ms) | Calls/s | vs Python native |
|---|---:|---:|---:|
| Python native | 2.184 | 9.16M | baseline |
| Sengoo Runtime (PythonInterop) | 2.665 | 7.50M | +22.02% |
| C++ (CPython C API) | 2.919 | 6.85M | +33.63% |
| Rust (PyO3) | 2.930 | 6.83M | +34.15% |

What this means in practice:

- The interop boundary cost is competitive with common C++/Rust embedding paths.
- You can migrate incrementally instead of rewriting a full Python system.
- Runtime interop is suitable for mixed workloads where Python remains the control plane.

### 2) Fast Compile + Incremental Feedback

Sengoo compiler pipeline is optimized for short feedback loops:

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

Advanced pipeline snapshot (real edits + 100k scale, measured on **February 16, 2026**):
`bench/results/1771228834821-advanced-pipeline.json`

Real incremental scenarios (`after_avg_ms`, Sengoo):

| Scenario | After avg (ms) |
|---|---:|
| `loop_body_change` | 219.75 |
| `function_signature_change` | 206.60 |
| `add_new_function` | 240.84 |

100k LOC full pipeline (Sengoo):

| Stage | Avg (ms) |
|---|---:|
| Frontend (`compile_frontend_llvm_avg_ms`) | 446.09 |
| Codegen object (`codegen_obj_avg_ms`) | 60.23 |
| Link (`link_avg_ms`) | 460.78 |
| End-to-end (`e2e_avg_ms`) | 967.10 |

### 3) Runtime-Class Performance Track

Scenario runtime p50 average (same matrix file `1771185238357`):

| Language | Runtime p50 avg (ms) |
|---|---:|
| Sengoo | 8.92 |
| C++ | 8.55 |
| Rust | 8.59 |
| Python | 45.14 |

Interpretation:

- Sengoo runtime behavior is in the same class as C++/Rust in this matrix profile.
- In these loop-heavy samples, Sengoo is much faster than Python process runtime.

### 4) Generality: Not Optimized for One Synthetic Case Only

Bootstrap generality proof (measured on **February 16, 2026**):
`bench/results/1771230417893-bootstrap-generality.json`

- Proof status: `pass`
- Scenarios passed: `6/6`
- Covered capability classes:
  - `control_flow_for`
  - `branching_while`
  - `array_ops`
  - `recursion`
  - `impl_method`
  - `module_import_graph`

This suite validates that optimization progress still preserves broad language behavior.

## Language and Tooling Highlights

- LLVM-based native compilation path
- Type-check and compile pipeline through `sgc`
- Cache/incremental-aware CLI flow
- Optional daemon mode for persistent compile sessions
- VS Code extension (current package version: `1.0.0`)

Useful CLI commands:

```bash
# type check
sgc check <file.sg>

# run via compile+cache pipeline
sgc run <file.sg> -O 1

# build native binary
sgc build <file.sg> -O 2

# force full rebuild
sgc build <file.sg> -O 2 --force-rebuild

# optional daemon
sgc daemon --addr 127.0.0.1:48765
```

## Best-Fit Application Scenarios

- Accelerating hot loops inside Python services
- CLI tools that need fast iterative compile cycles
- Algorithm-heavy modules distributed as native binaries
- Mixed-language systems where gradual migration matters
- Teams that want measurable perf gates in CI while evolving a compiler stack

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

## Repository Layout

```text
Sengoo/
|-- compiler/        # Frontend, type checker, HIR/MIR pipeline
|-- runtime/         # Runtime support and interop layer
|-- tools/
|   |-- sgc/         # Compiler CLI
|   |-- sgfmt/       # Formatter
|   `-- sglsp/       # Language server
|-- examples/        # Language examples
|-- docs/            # Documents and plans
`-- vscode-sengoo/   # VS Code extension
```

## Project Status

Current stage: early but fast-iterating.

Current focus:

- Frontend architecture optimization
- Stronger incremental consistency under real code edits
- Better interop/reflection ergonomics
- Tooling and developer experience polish

Notes:

- All benchmark numbers above are local-machine measurements and should be treated as trend indicators.
- Use the benchmark repository and CI gates to verify performance on your own hardware.

---

## 中文版

# Sengoo（中文介绍）

Sengoo 是一门自研编译型语言，当前目标非常明确：

- 强化 Python 互操作，支持渐进式迁移
- 提升全量/增量编译速度，缩短开发反馈周期
- 基于 LLVM 走原生性能路径

项目仍在快速迭代，但本地 CLI 开发流程已经可用。

## 适合什么团队

- 已有大量 Python 资产，但热点模块需要原生加速的团队
- 追求“改完就能快编译验证”的工程团队
- 关注编译器工程化、增量架构和可观测性能门禁的开发者

## 核心优势

### 1）Python 互操作：不必一次性重写系统

Sengoo 在运行时提供 Python 互操作层（`runtime/src/python.rs`），支持“Python 编排 + Sengoo 热点模块”的混合架构。

最新本地互操作基准（**2026-02-16**）：
`bench/results/1771230408116-python-interop.json`

| 路径 | Loop 平均耗时 (ms) | 吞吐 (Calls/s) | 相对 Python 原生 |
|---|---:|---:|---:|
| Python 原生 | 2.184 | 9.16M | 基线 |
| Sengoo Runtime (PythonInterop) | 2.665 | 7.50M | +22.02% |
| C++ (CPython C API) | 2.919 | 6.85M | +33.63% |
| Rust (PyO3) | 2.930 | 6.83M | +34.15% |

工程意义：

- Sengoo 的跨语言边界开销与常见 C++/Rust Python 嵌入路径同一量级。
- 可以按模块逐步迁移，而不是“全量重写再上线”。
- 适合保留 Python 主流程，同时把热点逻辑下沉到编译型模块。

### 2）编译与增量反馈：面向高迭代开发

Sengoo 编译链路当前重点放在“编辑-编译-运行”短反馈：

- build/run cache
- 模块指纹失效机制
- AST 级编辑分类（`noop` / `impl_only` / `interface_change`）
- workset 感知的后端调度
- 可选 daemon 常驻编译模式

跨语言场景矩阵快照（**2026-02-16**）：
`bench/results/1771185238357-scenario-matrix.json`

| 指标（平均） | Sengoo | C++ | Rust | Python |
|---|---:|---:|---:|---:|
| 全量编译 (ms) | 835.92 | 1669.41 | 972.98 | 67.48 |
| 增量编辑后编译 (ms) | 33.71 | 1702.23 | 1088.19 | 65.52 |
| 增量收益 (%) | 95.99% | -2.28% | -4.95% | 2.61% |

高级流水线基准（真实编辑 + 100k 规模，**2026-02-16**）：
`bench/results/1771228834821-advanced-pipeline.json`

Sengoo 真实增量场景（`after_avg_ms`）：

| 场景 | after 平均耗时 (ms) |
|---|---:|
| `loop_body_change` | 219.75 |
| `function_signature_change` | 206.60 |
| `add_new_function` | 240.84 |

100k LOC 全链路：

| 阶段 | 平均耗时 (ms) |
|---|---:|
| 前端 (`compile_frontend_llvm_avg_ms`) | 446.09 |
| 代码生成 (`codegen_obj_avg_ms`) | 60.23 |
| 链接 (`link_avg_ms`) | 460.78 |
| 端到端 (`e2e_avg_ms`) | 967.10 |

### 3）运行性能轨道：接近 C++/Rust 量级

同一场景矩阵下的 runtime p50 平均：

| 语言 | Runtime p50 平均 (ms) |
|---|---:|
| Sengoo | 8.92 |
| C++ | 8.55 |
| Rust | 8.59 |
| Python | 45.14 |

解释：

- 在当前样本中，Sengoo 运行时表现与 C++/Rust 接近。
- 在循环密集型样例里，相对 Python 进程运行时间有明显优势。

### 4）通用性验证：不是只对单一样例做优化

Bootstrap 通用性证明（**2026-02-16**）：
`bench/results/1771230417893-bootstrap-generality.json`

- 证明状态：`pass`
- 场景通过：`6/6`
- 覆盖能力：
  - `control_flow_for`
  - `branching_while`
  - `array_ops`
  - `recursion`
  - `impl_method`
  - `module_import_graph`

这套基准用于保证优化过程中不牺牲语言通用能力和正确性。

## 语言与工具链亮点

- LLVM 原生编译路径
- `sgc` 提供检查、运行、构建一体化流程
- 缓存/增量友好的命令行体验
- 可选 daemon 模式
- VS Code 插件（当前打包版本 `1.0.0`）

常用命令：

```bash
sgc check <file.sg>
sgc run <file.sg> -O 1
sgc build <file.sg> -O 2
sgc build <file.sg> -O 2 --force-rebuild
sgc daemon --addr 127.0.0.1:48765
```

## 典型应用场景

- Python 服务中的热点循环/算法模块加速
- 需要高频迭代验证的 CLI 工具开发
- 需要原生二进制交付的算法组件
- 需要平滑迁移的混合语言工程
- 希望在 CI 中建立可量化性能门禁的团队

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

## 基准与复现

基准仓库（独立维护）：

- `https://github.com/Hyper66666/bench`

常用脚本：

```bash
python ./bench/scenario_matrix_bench.py
python ./bench/advanced_pipeline_bench.py
python ./bench/python_interop_bench.py
python ./bench/bootstrap_generality_bench.py
```

高级流水线公平化设置：

- C++ 启用预编译头（PCH）
- Rust 启用 `cargo incremental`（`CARGO_INCREMENTAL=1`）

## 当前状态

当前阶段：早期但快速迭代。

当前重点：

- 前端架构进一步优化
- 真实代码编辑场景下的增量稳定性
- 互操作与反射能力的工程化增强
- 工具链和开发体验打磨

说明：

- 上述数据均为本地机器实测，主要用于趋势判断。
- 建议使用 bench 仓库和 CI gate 在目标硬件上复测。
