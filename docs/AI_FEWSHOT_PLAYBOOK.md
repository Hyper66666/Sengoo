# Sengoo AI Few-Shot Playbook (EN + 中文)

This document is designed for handing Sengoo work to another LLM with minimal context loss.

It contains:
- Stable compiler diagnostic JSON format
- Contract-oriented function style (`requires` / `ensures`)
- Few-shot examples with both code and "why this shape" notes

---

## 1) Compiler Error JSON (for tool-friendly agents)

Use:

```bash
sgc --error-format json check path/to/file.sg
sgc --error-format json build path/to/file.sg
sgc --error-format json run path/to/file.sg
```

Note:
- Progress logs are printed to `stdout`.
- Structured compile error JSON is printed to `stderr`.
- For automation, parse `stderr` first.

Current payload shape:

```json
{
  "ok": false,
  "kind": "compile_error",
  "stage": "parse|typecheck|codegen|io|mir_lower|compile",
  "message": "short one-line error summary",
  "input": "optional source path",
  "hint": "use --error-format text for human-friendly diagnostics",
  "details": ["extra lines from compiler diagnostic"]
}
```

Why this format:
- `stage` lets agents branch quickly (parser fix vs type fix vs backend fix).
- `message` is concise and easy to feed back into prompts.
- `details` preserves multi-line context without breaking machine parsing.

---

## 2) Contracts as first-class syntax

Sengoo function contracts:

```sg
def divide(a: i64, b: i64) -> i64
requires b != 0
ensures result * b == a
{
    a / b
}
```

Current compiler checks:
- `requires` must type-check to `bool`.
- `ensures` must type-check to `bool`.
- `ensures` can reference `result` (the return value placeholder).
- Obvious contradictions are rejected (for constant-return functions and literal postconditions).
- Runtime contract guard insertion is controlled by `--contract-checks auto|on|off`.

Why this is useful for AI generation:
- Model can generate **contract first** (intent), then implementation.
- Compiler becomes an automatic guard against intent/code mismatch.

Runtime-mode examples:

```bash
sgc run path/to/file.sg -O 1 --contract-checks auto
sgc run path/to/file.sg -O 2 --contract-checks on
```

---

## 3) Few-shot examples (Code + Why)

## Example A: Defensive arithmetic

Goal:
- Generate safe arithmetic with explicit intent.

Code:

```sg
def checked_ratio(sum: i64, count: i64) -> i64
requires count != 0
ensures result * count == sum
{
    sum / count
}
```

Why this shape:
- We expose the key domain rule (`count != 0`) as `requires` instead of hidden comments.
- We bind expected result semantics (`result * count == sum`) as a postcondition.
- This separates "what must hold" from "how to compute".

---

## Example B: Contract-first refactor target

Goal:
- Ask an LLM to optimize implementation while preserving behavior.

Code:

```sg
def abs_i64(x: i64) -> i64
ensures result >= 0
{
    if x < 0 { -x } else { x }
}
```

Why this shape:
- The postcondition defines invariant semantics independent of control-flow style.
- A model can rewrite the body for performance/readability and keep the same contract.

---

## Example C: Tool-driven compile loop

Prompt template:

```text
Input:
1) Sengoo source file
2) Compiler JSON error payload

Task:
- Read payload.stage first.
- Propose the smallest patch that addresses payload.message.
- Do not change public signatures unless stage == interface/type-level issue.
- Preserve requires/ensures semantics if present.
```

Why this shape:
- It forces stage-aware repair and avoids random full rewrites.
- It keeps contract semantics stable while iterating.

---

## 4) Recommended LLM handoff workflow

1. Run `sgc --error-format json check <file.sg>`.
2. Feed JSON payload + relevant source snippet to the model.
3. Ask model to return:
   - minimal patch,
   - reason in 3-5 bullets,
   - risk list.
4. Re-run compile/test loop.

---

# 中文版

本文件用于把 Sengoo 开发工作快速移交给其他大模型，核心是“低上下文损失”。

包含三类内容：
- 编译器 JSON 报错协议
- 契约式函数写法（`requires` / `ensures`）
- 可直接当 Few-Shot 的“代码 + 为什么这样写”示例

## 1）编译器 JSON 报错（便于模型自动处理）

使用方式：

```bash
sgc --error-format json check path/to/file.sg
sgc --error-format json build path/to/file.sg
sgc --error-format json run path/to/file.sg
```

注意：
- 常规进度日志在 `stdout`。
- 结构化错误 JSON 在 `stderr`。
- 自动化脚本建议优先解析 `stderr`。

字段语义：
- `stage`: 快速定位阶段（parse/typecheck/codegen/...）
- `message`: 一行核心错误
- `details`: 多行附加信息，便于后续修复

这样设计的原因：
- 大模型可以先按阶段分流，再做最小修复，而不是整文件重写。

## 2）契约优先（先写“做什么”，再写“怎么做”）

示例：

```sg
def divide(a: i64, b: i64) -> i64
requires b != 0
ensures result * b == a
{
    a / b
}
```

当前编译器保障：
- `requires` / `ensures` 必须是 `bool`。
- `ensures` 可以引用 `result` 返回值占位符。
- 对“明显自相矛盾”的后置条件会直接报错（常量返回场景）。
- 运行时契约检查是否插入由 `--contract-checks auto|on|off` 控制。

价值：
- 让模型先生成契约，再补实现，减少“逻辑幻觉”。

运行时模式示例：

```bash
sgc run path/to/file.sg -O 1 --contract-checks auto
sgc run path/to/file.sg -O 2 --contract-checks on
```

## 3）Few-Shot 示例（代码 + 原因）

### 示例 A：防御式算术

```sg
def checked_ratio(sum: i64, count: i64) -> i64
requires count != 0
ensures result * count == sum
{
    sum / count
}
```

为什么这样写：
- 前置条件显式表达业务边界（避免除零）。
- 后置条件固定语义，后续优化实现时不易跑偏。

### 示例 B：可重构的语义锚点

```sg
def abs_i64(x: i64) -> i64
ensures result >= 0
{
    if x < 0 { -x } else { x }
}
```

为什么这样写：
- 把语义锚点放在契约，不绑死实现细节。
- 模型可在不破坏契约的前提下做局部优化。

### 示例 C：错误驱动修复提示词

```text
输入：
1）Sengoo 源码片段
2）编译器 JSON 报错

任务：
- 先看 payload.stage 判断是语法/类型/后端问题
- 只做最小修复
- 非必要不改公共签名
- 如果有 requires/ensures，必须保持语义一致
```

为什么这样写：
- 约束模型走“定向修复”路径，降低过度修改风险。

## 4）推荐移交流程

1. 运行 `sgc --error-format json check <file.sg>`
2. 把 JSON + 相关代码喂给模型
3. 要求模型输出最小补丁 + 简要理由 + 风险点
4. 编译和测试回归，循环迭代
