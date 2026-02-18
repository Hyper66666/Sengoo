# Sengoo Development Guide / Sengoo 开发手册

> Version: 2026-02-16  
> Audience: Contributors who want to join Sengoo compiler/runtime/tooling development quickly.

---

## English

### 1. Project Scope

Sengoo is a compiled language project with these practical goals:

- Native execution path via LLVM
- Fast edit-build-run iteration via incremental compilation
- Python interoperability for gradual migration scenarios
- Developer tooling around CLI and VS Code

This repository contains the core language stack. Benchmark assets are maintained in a separate repository: `https://github.com/Hyper66666/bench`.

### 2. Repository Map

```text
Sengoo/
|-- compiler/          # frontend, type checker, HIR/MIR, codegen pipeline
|-- runtime/           # runtime support + Python interop layer
|-- tools/
|   |-- sgc/           # compiler CLI
|   |-- sgfmt/         # formatter (if enabled in your workflow)
|   `-- sglsp/         # language server
|-- examples/          # runnable Sengoo source examples
|-- docs/              # tutorial/dev docs/technical plans
`-- vscode-sengoo/     # VS Code extension
```

### 3. 15-Minute Onboarding

1. Clone and build.

```bash
git clone https://github.com/Hyper66666/Sengoo.git
cd Sengoo
cargo build --release
```

2. Validate basic pipeline.

```bash
target/release/sgc check examples/01_hello.sg
target/release/sgc run examples/01_hello.sg
target/release/sgc build examples/05_loop.sg -O 2
```

3. If you use VS Code, install packaged extension from `vscode-sengoo/` (`1.0.0`) and verify `sengoo.run` command.

### 4. Compiler Architecture (Current Practical View)

Primary pipeline in `sgc`:

1. Lex/parse source into AST.
2. Type checking over AST.
3. Lower AST to HIR.
4. Lower HIR to MIR.
5. Run MIR optimization passes.
6. Emit LLVM IR.
7. Use clang/LLVM toolchain for native object/link/run path.

Key implementation files:

- `compiler/src/lexer/token.rs`
- `compiler/src/parser/`
- `compiler/src/ast/`
- `compiler/src/typeck/check.rs`
- `compiler/src/hir/`
- `compiler/src/mir/lowering.rs`
- `compiler/src/codegen/mod.rs`
- `tools/sgc/src/main.rs`

### 5. Runtime and Interop

Runtime code lives in `runtime/`.

Important context:

- Runtime helpers are used by generated LLVM/native path.
- Python interop currently exists at runtime integration level (PyO3 path), and is validated by benchmark suites in the separate `bench` repo.

### 6. CLI Commands You Need Daily

`sgc` supports:

- `build <file.sg> [-O 0..3] [--emit-llvm] [--force-rebuild] [--low-memory] [--daemon]`
- `run <file.sg> [-O 0..3] [--engine auto|native|lli] [--force-rebuild] [--low-memory] [--daemon]`
- `check <file.sg>`
- `dump-ast <file.sg>`
- `daemon --addr <host:port>`
- `bench run|compile|incremental ...` (internal benchmark suite entry)

Examples:

```bash
target/release/sgc run examples/09_method_call.sg -O 1
target/release/sgc build examples/08_struct.sg --emit-llvm
target/release/sgc build examples/08_struct.sg --emit-llvm --low-memory
target/release/sgc daemon --addr 127.0.0.1:48765
```

### 7. Incremental Pipeline and Performance Work

Current optimization direction uses:

- build/run cache metadata
- module and function fingerprints
- edit class detection (`noop`, `impl_only`, `interface_change`)
- workset-aware rebuild planning
- optional daemon dispatch for lower startup overhead

When changing this area:

1. Add/adjust tests around incremental behavior.
2. Run representative benchmark suites.
3. Compare against previous reports rather than single-run numbers.

### 8. Tests and Verification Workflow

Recommended local sequence before claiming completion:

1. Targeted unit/integration tests for your changed modules.
2. At least one end-to-end CLI run (`check` + `run`/`build`).
3. If performance-sensitive change: run benchmark suite in `bench` repo.

Core checks in this repo:

```bash
cargo check -p sengoo-compiler
cargo check -p sengoo-runtime
cargo check -p sgc
```

### 9. Working with the Bench Repository

Clone side-by-side with Sengoo:

```bash
cd /path/to/workspace
git clone https://github.com/Hyper66666/Sengoo.git
git clone https://github.com/Hyper66666/bench.git
```

Set `SENGOO_ROOT` if not sibling layout.

Run common suites:

```bash
cd bench
python ./scenario_matrix_bench.py
python ./advanced_pipeline_bench.py
python ./python_interop_bench.py
python ./bootstrap_generality_bench.py
```

### 10. Developer Workflow and Change Discipline

For reliable collaboration:

- Keep changes scoped.
- Write/update docs when behavior changes.
- Preserve measurable evidence for performance claims.
- Avoid benchmarking shortcuts that only optimize synthetic cases.
- Prefer reproducible scripts over manual ad-hoc runs.

### 11. OpenSpec Usage for Major Changes

Use OpenSpec for architecture-level or cross-cutting changes:

```bash
openspec list
openspec list --specs
```

Typical flow:

1. Create proposal/design/tasks under `openspec/changes/<change-id>/`.
2. Validate with strict mode.
3. Implement after proposal review/approval.

### 12. Common Issues and Fast Fixes

- `clang/lli not found`:
  - install LLVM and ensure in PATH.
- `command 'sengoo.run' not found` in VS Code:
  - confirm extension installation + `Developer: Reload Window`.
- Incremental path unexpectedly full-rebuild:
  - inspect cache metadata and changed interface signatures.
- Bench report variance:
  - rerun with fixed samples and reduced system noise.

### 13. Where to Continue Next

- Language tutorial: `docs/sengoo-tutorial.html`
- Root project overview: `README.md`
- Example programs: `examples/`
- Active technical plans: `docs/plans/`

---

## 中文版

### 1. 项目定位

Sengoo 是一个编译型语言项目，当前工程目标是：

- 通过 LLVM 走原生执行路径
- 通过增量编译缩短编辑-编译-运行反馈
- 支持 Python 互操作以实现渐进迁移
- 构建 CLI + VS Code 的开发工具链

本仓库是核心语言栈；基准资产独立在 `bench` 仓库：`https://github.com/Hyper66666/bench`。

### 2. 仓库结构

```text
Sengoo/
|-- compiler/          # 前端、类型检查、HIR/MIR、代码生成
|-- runtime/           # 运行时支持 + Python 互操作层
|-- tools/
|   |-- sgc/           # 编译器 CLI
|   |-- sgfmt/         # 格式化工具
|   `-- sglsp/         # 语言服务器
|-- examples/          # 可运行示例
|-- docs/              # 文档、教程、计划
`-- vscode-sengoo/     # VS Code 插件
```

### 3. 15 分钟上手

1. 克隆并构建：

```bash
git clone https://github.com/Hyper66666/Sengoo.git
cd Sengoo
cargo build --release
```

2. 跑通基本链路：

```bash
target/release/sgc check examples/01_hello.sg
target/release/sgc run examples/01_hello.sg
target/release/sgc build examples/05_loop.sg -O 2
```

3. 使用 VS Code 时，安装 `vscode-sengoo/` 中打包插件（当前 `1.0.0`），并验证 `sengoo.run` 命令可用。

### 4. 编译器架构（当前可落地视角）

`sgc` 主链路：

1. 词法/语法解析到 AST
2. AST 类型检查
3. AST 降级到 HIR
4. HIR 降级到 MIR
5. 执行 MIR 优化
6. 生成 LLVM IR
7. 通过 clang/LLVM 走原生目标链接与运行

关键实现入口：

- `compiler/src/lexer/token.rs`
- `compiler/src/parser/`
- `compiler/src/ast/`
- `compiler/src/typeck/check.rs`
- `compiler/src/hir/`
- `compiler/src/mir/lowering.rs`
- `compiler/src/codegen/mod.rs`
- `tools/sgc/src/main.rs`

### 5. 运行时与互操作

运行时代码在 `runtime/`。

当前关键点：

- 生成代码会调用 runtime 辅助能力。
- Python 互操作当前在运行时集成层（PyO3 路径），可通过独立 `bench` 仓库验证。

### 6. 日常必须掌握的 CLI

`sgc` 常用子命令：

- `build <file.sg> [-O 0..3] [--emit-llvm] [--force-rebuild] [--daemon]`
- `run <file.sg> [-O 0..3] [--engine auto|native|lli] [--force-rebuild] [--daemon]`
- `check <file.sg>`
- `dump-ast <file.sg>`
- `daemon --addr <host:port>`
- `bench run|compile|incremental ...`

示例：

```bash
target/release/sgc run examples/09_method_call.sg -O 1
target/release/sgc build examples/08_struct.sg --emit-llvm
target/release/sgc daemon --addr 127.0.0.1:48765
```

### 7. 增量编译与性能方向

当前优化路径包含：

- build/run cache metadata
- 模块与函数指纹
- 编辑分类（`noop` / `impl_only` / `interface_change`）
- workset 感知重编译计划
- 可选 daemon 分发降低冷启动开销

改这一块时建议：

1. 先补/改增量行为测试。
2. 再跑代表性基准。
3. 用报告对比趋势，不看单次数字。

### 8. 测试与验收流程

建议本地最小验收序列：

1. 变更模块的定向单测/集成测试。
2. 至少一次端到端 CLI 验证（`check` + `run`/`build`）。
3. 性能敏感改动必须跑 `bench` 仓库基准。

本仓库常用检查：

```bash
cargo check -p sengoo-compiler
cargo check -p sengoo-runtime
cargo check -p sgc
```

### 9. 如何联动 Bench 仓库

和 Sengoo 同级克隆：

```bash
cd /path/to/workspace
git clone https://github.com/Hyper66666/Sengoo.git
git clone https://github.com/Hyper66666/bench.git
```

如果不是同级，设置 `SENGOO_ROOT`。

常用基准：

```bash
cd bench
python ./scenario_matrix_bench.py
python ./advanced_pipeline_bench.py
python ./python_interop_bench.py
python ./bootstrap_generality_bench.py
```

### 10. 开发协作纪律

为了让后续开发者可持续接手：

- 变更范围要收敛。
- 行为变化要同步更新文档。
- 性能结论必须附可复现实验依据。
- 避免只对单一 synthetic bench 定向优化。
- 尽量用脚本化流程代替手工临时操作。

### 11. OpenSpec 用法（重大改动）

涉及架构级或跨模块改动时，按 OpenSpec 流程：

```bash
openspec list
openspec list --specs
```

通常步骤：

1. 在 `openspec/changes/<change-id>/` 写 proposal/design/tasks。
2. 严格校验后再实施。
3. 通过评审后执行实现。

### 12. 常见问题与快速定位

- `clang/lli not found`
  - 安装 LLVM 并加入 PATH。
- VS Code 报 `command 'sengoo.run' not found`
  - 检查插件安装，执行 `Developer: Reload Window`。
- 增量路径频繁退化为全量
  - 检查 cache metadata 与接口签名变化。
- 基准波动明显
  - 固定采样次数并降低系统噪声后重测。

### 13. 下一步阅读路径

- 语言教程：`docs/sengoo-tutorial.html`
- 项目总览：`README.md`
- 语法示例：`examples/`
- 技术计划：`docs/plans/`
