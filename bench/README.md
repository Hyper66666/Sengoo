# Sengoo Bench

Independent benchmark repository for Sengoo.  
Goal: keep performance and correctness measurements reproducible, comparable, and CI-gated.

## Scope

- Cross-language matrix: Sengoo vs C++ vs Rust vs Python
- Real incremental compile scenarios
- Scale curve: 1k / 10k / 100k / 1000k LOC
- Stage split and link-share analysis
- Compile peak-memory comparison
- Python interop benchmark
- Non-invasive reflection benchmark
- Bootstrap generality proof
- KPI gate scripts for CI
- Runnable demos and smoke checks

## Layout

```text
bench/
|-- scripts/
|-- suites/
|-- tests/
|-- demos/
|-- results/
|-- scenario_matrix_bench.py
|-- advanced_pipeline_bench.py
|-- python_interop_bench.py
|-- llm_scheduler_bench.py
|-- noninvasive_reflection_bench.py
`-- bootstrap_generality_bench.py
```

## Prerequisites

- Python 3.10+
- Rust toolchain (`cargo`, `rustc`)
- Sengoo source tree (`SENGOO_ROOT` or sibling layout)
- Recommended: LLVM/Clang (`clang`, `clang++`)

## Quick Start

### Clone

```bash
cd /path/to/workspace
git clone https://github.com/Hyper66666/Sengoo.git
git clone https://github.com/Hyper66666/bench.git
cd bench
```

```powershell
Set-Location C:\path\to\workspace
git clone https://github.com/Hyper66666/Sengoo.git
git clone https://github.com/Hyper66666/bench.git
Set-Location .\bench
```

If not sibling layout:

```bash
export SENGOO_ROOT=/absolute/path/to/Sengoo
```

```powershell
$env:SENGOO_ROOT = "C:\absolute\path\to\Sengoo"
```

### Environment check

```bash
python --version
cargo --version
clang --version
```

## Standard Run Flow

### 1) Smoke

```bash
bash ./scripts/e2e-smoke.sh
```

```powershell
powershell -File .\scripts\e2e-smoke.ps1
```

### 2) Core suites

```bash
python ./scenario_matrix_bench.py
python ./advanced_pipeline_bench.py
python ./python_interop_bench.py
python ./llm_scheduler_bench.py
python ./noninvasive_reflection_bench.py
python ./bootstrap_generality_bench.py
```

### LLM scheduler benchmark (prefill/decode orchestration focus)

```bash
python ./llm_scheduler_bench.py
```

This benchmark compares:
- Python scheduler + shared decode-step kernel
- Sengoo Runtime scheduler + same decode-step kernel

It emits two scenarios:
- `prefill_decode_orchestration_light_kernel` (orchestration-dominant, expected Sengoo advantage)
- `prefill_decode_orchestration_heavy_kernel` (compute-kernel heavier, expected parity)

Output:
- writes `results/*-llm-scheduler-bench.json`
- includes checksum parity, loop latency, and tokens/s gain

CI smoke preset:

```bash
python ./llm_scheduler_bench.py --preset ci-smoke
# alias
python ./llm_scheduler_bench.py --smoke
```

Fixed smoke parameters:
- `requests=64`
- `max_batch=8`
- `max_new_per_step=4`
- `max_len=6`
- `samples=1`
- `warmup=0`
- `light_kernel_iters=0`
- `heavy_kernel_iters=8`

Notes:
- smoke keeps the normal JSON report schema; existing consumers do not need a separate parser
- smoke still fails the run on Python/Sengoo checksum mismatch
- if `.llm-scheduler-work` already has a built runner, use `--skip-build` for a faster local check

### 3) CI gates

```bash
python ./scripts/advanced-kpi-gate.py --mode soft --sample ./results/<advanced-report>.json --baseline-profile ./frontend-memory-baseline.json
python ./scripts/interop-bootstrap-gate.py --mode soft --interop-sample ./results/<interop-report>.json --bootstrap-sample ./results/<bootstrap-report>.json
python ./scripts/llm-scheduler-gate.py --mode soft --sample ./results/<llm-scheduler-report>.json
```

Use `--mode hard` in CI.

## Latest Snapshot (February 18, 2026)

Source reports:

- `results/1771185238357-scenario-matrix.json`
- `results/1771390773767-advanced-pipeline.json`
- `results/1771392747911-advanced-pipeline.json`
- `results/1771234431756-python-interop.json`
- `results/1771242399249-noninvasive-reflection-bench.json`
- `results/1771230417893-bootstrap-generality.json`
- `results/1771425334804-low-memory-e2e-1000k.json`

### Incremental reduction average

| Language | Incremental reduction avg |
|---|---:|
| Sengoo | 95.99% |
| C++ | -2.28% |
| Rust | -4.95% |
| Python | 2.61% |

### 10k-1000k e2e compile comparison

| LOC | Sengoo (ms) | C++ (ms) | Rust (ms) | Python (ms) |
|---|---:|---:|---:|---:|
| 10k | 372.28 | 693.01 | 2246.86 | 157.18 |
| 100k | 417.53 | 1074.84 | 6625.35 | 832.91 |
| 1000k | 1827.84 | 4883.70 | 54642.47 | 8283.46 |

### Compile peak RSS comparison (compile-stage)

| LOC | Sengoo (MB) | C++ (MB) | Rust (MB) | Python (MB) |
|---|---:|---:|---:|---:|
| 10k | 18.88 | 75.68 | 70.84 | 41.40 |
| 100k | 140.18 | 118.50 | 337.86 | 288.46 |
| 1000k | 1367.99 | 435.22 | 2681.55 | 2610.90 |

### Low-memory mode (1000k e2e, newly added)

| Mode | e2e avg (ms) | Peak RSS (MB) |
|---|---:|---:|
| Default (`sgc build`) | 2331.39 | 1418.61 |
| `--low-memory` | 1737.71 | 672.10 |

Benefits:

- e2e time reduction: 25.46%
- peak RSS reduction: 52.62%

Trade-offs:

- less incremental cache/session reuse
- single-thread frontend in low-memory mode
- lower MIR optimization cap

Enable:

```bash
sgc build your_file.sg --low-memory
sgc run your_file.sg --low-memory
```

## Result Files

Main outputs under `results/`:

- `*-scenario-matrix.json`
- `*-advanced-pipeline.json`
- `*-python-interop.json`
- `*-noninvasive-reflection-bench.json`
- `*-bootstrap-generality.json`
- `*-low-memory-e2e-1000k.json`

## Troubleshooting

- `cannot resolve Sengoo source root`: set `SENGOO_ROOT`.
- `clang++ not found`: install LLVM/Clang, or C++ runners may be skipped.
- timing variance: close heavy background processes and increase samples.

---

# 中文�?
这是 Sengoo 的独立基准仓库，用于把性能与正确性测试做成可复现、可对比、可接入 CI 的流程�?
## 覆盖内容

- 四语言对比：Sengoo / C++ / Rust / Python
- 真实增量场景（改循环体、改函数签名、加新函数）
- 规模曲线�?k / 10k / 100k / 1000k LOC�?- 阶段拆分与链接占�?- 编译峰值内存对�?- Python 互操作基�?- 非侵入式反射基准
- 自举通用性证�?- CI 门禁脚本

## 快速开�?
```bash
cd /path/to/workspace
git clone https://github.com/Hyper66666/Sengoo.git
git clone https://github.com/Hyper66666/bench.git
cd bench
```

非同级目录时手动设置�?
```bash
export SENGOO_ROOT=/absolute/path/to/Sengoo
```

## 推荐执行流程

### 1）冒烟检�?
```bash
bash ./scripts/e2e-smoke.sh
```

### 2）核心基�?
```bash
python ./scenario_matrix_bench.py
python ./advanced_pipeline_bench.py
python ./python_interop_bench.py
python ./llm_scheduler_bench.py
python ./noninvasive_reflection_bench.py
python ./bootstrap_generality_bench.py
```

### 3）门�?
```bash
python ./scripts/advanced-kpi-gate.py --mode soft --sample ./results/<advanced-report>.json --baseline-profile ./frontend-memory-baseline.json
python ./scripts/interop-bootstrap-gate.py --mode soft --interop-sample ./results/<interop-report>.json --bootstrap-sample ./results/<bootstrap-report>.json
python ./scripts/llm-scheduler-gate.py --mode soft --sample ./results/<llm-scheduler-report>.json
```

## 最新快照（2026�?�?8日）

报告文件�?
- `results/1771185238357-scenario-matrix.json`
- `results/1771390773767-advanced-pipeline.json`
- `results/1771392747911-advanced-pipeline.json`
- `results/1771234431756-python-interop.json`
- `results/1771242399249-noninvasive-reflection-bench.json`
- `results/1771230417893-bootstrap-generality.json`
- `results/1771425334804-low-memory-e2e-1000k.json`

### 增量收益平均�?
| 语言 | 增量收益平均�?|
|---|---:|
| Sengoo | 95.99% |
| C++ | -2.28% |
| Rust | -4.95% |
| Python | 2.61% |

### 10k-1000k e2e 编译对比

| LOC | Sengoo (ms) | C++ (ms) | Rust (ms) | Python (ms) |
|---|---:|---:|---:|---:|
| 10k | 372.28 | 693.01 | 2246.86 | 157.18 |
| 100k | 417.53 | 1074.84 | 6625.35 | 832.91 |
| 1000k | 1827.84 | 4883.70 | 54642.47 | 8283.46 |

### 编译峰�?RSS（仅编译阶段�?
| LOC | Sengoo (MB) | C++ (MB) | Rust (MB) | Python (MB) |
|---|---:|---:|---:|---:|
| 10k | 18.88 | 75.68 | 70.84 | 41.40 |
| 100k | 140.18 | 118.50 | 337.86 | 288.46 |
| 1000k | 1367.99 | 435.22 | 2681.55 | 2610.90 |

### 低内存模式（新增�?000k e2e�?
| 模式 | e2e 平均 (ms) | 峰�?RSS (MB) |
|---|---:|---:|
| 默认（`sgc build`�?| 2331.39 | 1418.61 |
| `--low-memory` | 1737.71 | 672.10 |

优势�?
- e2e 时间下降 25.46%
- 峰�?RSS 下降 52.62%

副作用：

- 增量缓存/会话复用能力会减�?- 前端固定单线�?- MIR 优化上限下降

开启方式：

```bash
sgc build your_file.sg --low-memory
sgc run your_file.sg --low-memory
```



