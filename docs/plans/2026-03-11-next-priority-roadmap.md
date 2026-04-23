# Sengoo Next Priority Roadmap

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 用最小风险顺序推进 Sengoo 的下一阶段能力建设，先清除明确技术阻塞，再扩展 async 能力，最后处理结构和重构类工作。

**Architecture:** 当前编译器主线已经具备 phase-1 async、泛型和 trait 基础，但 async 类型支持和 runtime 能力仍有明确边界。路线图先补 `MIR Bitcast`，解锁 float 跨 `await`；随后用低成本高收益的结构体字段完整性检查补齐语言正确性；最后分层推进 async phase-2，而不是一次性把 `async block`、`spawn/join/select` 混成一刀。

**Tech Stack:** Rust, MIR/LLVM IR codegen, JIT codegen, native async runtime bridge, OpenSpec, cargo test.

---

### Task 1: MIR Bitcast 指令

**Priority:** P0

**Why now:**
- 当前 `float` 跨 `await` 被明确拒绝。
- `Cast` 做的是值转换，不是位模式重解释。
- 这是 async 类型支持最清晰、最集中的技术阻塞。

**Files:**
- Modify: `compiler/src/mir/inst.rs`
- Modify: `compiler/src/mir/async_lowering.rs`
- Modify: `compiler/src/codegen/mod.rs`
- Modify: `compiler/src/codegen/jit.rs`
- Test: `compiler/src/tests/async_tests.rs`
- Test: `compiler/src/tests/cast_semantics_tests.rs`

**Deliverables:**
- 新增 `Instruction::Bitcast`
- LLVM/JIT 后端都支持 `Bitcast`
- `f32/f64` async frame 编解码改为 bit-preserving 路径
- 明确拒绝非法尺寸 bitcast

**Exit criteria:**
- `async_f32_local_survives_await`
- `async_f64_local_survives_await`
- 生成 IR 中能看到对应 bitcast 路径
- `cargo test -p sengoo-compiler`
- `cargo test -p sgc`

**Commit suggestion:**
- `feat: add mir bitcast for float async frame support`

---

### Task 2: 结构体字段完整性检查

**Priority:** P0.5

**Why before more async phase-2 work:**
- 这是语言正确性问题，不只是“代码质量”。
- 改动范围小，收益直接，风险低。
- 可以顺手把长期 ignored 的测试转绿。

**Files:**
- Modify: `compiler/src/typeck/check.rs`
- Test: `compiler/src/tests/struct_codegen_tests.rs`

**Deliverables:**
- struct literal 一次性检查：
  - missing fields
  - duplicate fields
  - unknown fields
- 错误顺序稳定，诊断可读
- 取消忽略缺字段测试

**Exit criteria:**
- `test_struct_construction_missing_field_produces_error` 转绿
- 新增 duplicate/unknown/mixed case 回归
- `cargo test -p sengoo-compiler struct_`
- `cargo test -p sgc`

**Commit suggestion:**
- `fix: validate struct literal field completeness`

---

### Task 3: Async Phase-2 shipped surface closeout

**Priority:** P1 documentation/status cleanup

**Current status:** Completed on `main`; keep this roadmap from re-planning
already shipped async Phase-2 work as future feature work.

**Evidence:**
- OpenSpec change: `openspec/changes/async-phase-2-features`
- Compiler tests: `compiler/src/tests/async_tests.rs`
- Native/runtime tests: `tools/sgc/src/tests.rs`
- Runtime support: `runtime/src/async_runtime.rs`

**Shipped surface:**
- `async { ... }` blocks
- `sleep(...)` and `timeout(...)`
- `spawn(...)`, `spawn_task(...)`, `cancel_task(...)`, and `task_status(...)`
- `join(...)`
- current `select(...)` surface

**Remaining follow-up boundaries:**
- cyclic async CFG for loop-heavy `await` bodies
- richer async frame types, including payload-carrying enum values across `await`
- final generalized `select(...)` result-type surface
- full async IO/reactor model

**Exit criteria:**
- `openspec/changes/async-phase-2-features` stays `status: completed`
- this roadmap no longer lists async block, spawn, join, or select as greenfield work
- future async work is tracked as boundary-specific follow-up, not as Phase-2 reimplementation

---

## Recommended Order

1. `MIR Bitcast`
2. Struct field completeness validation
3. Async Phase-2 closeout documentation and boundary tracking
4. Cyclic async CFG follow-up
5. Payload-carrying enum across `await`
6. Final generalized `select(...)` surface
7. Trait specialization refactor

## Scope Notes

- Do not re-plan `async block`, `spawn`, `join`, or the current `select` surface as unimplemented Phase-2 features; they are shipped and documented by `openspec/changes/async-phase-2-features`.
- Keep remaining async work boundary-specific: cyclic CFG, richer frame payload types, and final select generalization.
- Treat `MIR Bitcast` and struct field completeness as independent quality/blocking work rather than prerequisites for redoing the shipped async Phase-2 surface.
## Verification Baseline

每个阶段完成后至少执行：

```powershell
cargo test -p sengoo-compiler
cargo test -p sgc
```

如果阶段涉及 native async 路径，再补：

```powershell
cargo test -p sgc async_native_runtime_ -- --nocapture
```
