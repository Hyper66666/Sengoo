# Async Typed Frame Slots Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Allow single-slot non-`i64` values to survive async suspend/resume while rejecting aggregate values that cannot yet cross await points.

**Architecture:** Keep the runtime frame ABI as `i64` slots and add async-lowering-side encode/decode plus stricter validation. Extend LLVM/JIT cast generation so the helper MIR can legally convert between real MIR types and frame-slot `i64`.

**Tech Stack:** Rust, Sengoo compiler MIR lowering, LLVM IR backend, JIT backend, cargo tests.

---

### Task 1: Add failing async compiler tests

**Files:**
- Modify: `compiler/src/tests/async_tests.rs`

**Step 1: Write the failing tests**
- Add:
  - `async_bool_local_survives_await`
  - `async_i32_local_survives_await`
  - `async_ref_local_survives_await`
  - `async_tuple_local_across_await_rejected`
  - `async_struct_local_across_await_rejected`

**Step 2: Run tests to verify they fail**

Run:
```bash
cargo test -p sengoo-compiler async_bool_local_survives_await
cargo test -p sengoo-compiler async_i32_local_survives_await
cargo test -p sengoo-compiler async_ref_local_survives_await
cargo test -p sengoo-compiler async_tuple_local_across_await_rejected
cargo test -p sengoo-compiler async_struct_local_across_await_rejected
```

### Task 2: Add a dedicated async unsupported-type error

**Files:**
- Modify: `compiler/src/error.rs`
- Modify: `compiler/src/lib.rs`
- Modify: `compiler/src/mir/async_lowering.rs`

**Step 1: Add `CompileError::AsyncUnsupportedType`**
- Store the MIR type as a stringified field plus a reason string.

**Step 2: Change async helper expansion to return `Result<Vec<MirFunction>, CompileError>`**
- Update both async expansion call sites in `compiler/src/lib.rs`.

### Task 3: Implement frame slot classification and validation

**Files:**
- Modify: `compiler/src/mir/async_lowering.rs`

**Step 1: Add a frame-slot classification helper**
- Distinguish:
  - supported scalar/int kinds
  - bool
  - pointer/ref/future-handle
  - unsupported aggregate kinds

**Step 2: Validate all frame-bound values**
- Validate:
  - async params in `__start`
  - live locals spilled in `__poll`
  - result type in `__result`

### Task 4: Encode/decode frame values in MIR helper synthesis

**Files:**
- Modify: `compiler/src/mir/async_lowering.rs`

**Step 1: Encode before store**
- Add helper(s) that convert a local to an `i64` slot value.

**Step 2: Decode after load**
- Add helper(s) that convert loaded `i64` slot values back to the requested MIR type.

**Step 3: Apply helpers**
- `synthesize_start`
- linear multi-await spill/resume path
- `synthesize_result`

### Task 5: Make LLVM/JIT cast lowering match the new helper MIR

**Files:**
- Modify: `compiler/src/codegen/mod.rs`
- Modify: `compiler/src/codegen/jit.rs`

**Step 1: Support int widening/narrowing to and from `i64`**
- Cover `bool`, `i8`, `i16`, `i32`, `i64`.

**Step 2: Support pointer/ref/future-handle casts**
- Use `ptrtoint` / `inttoptr` semantics in both backends.

### Task 6: Add native runtime regression tests

**Files:**
- Modify: `tools/sgc/src/tests.rs`

**Step 1: Add failing native tests**
- `async_native_runtime_preserves_bool_across_resume`
- `async_native_runtime_preserves_i32_across_resume`
- `async_native_runtime_preserves_ref_across_resume`

**Step 2: Run them red, then green**

### Task 7: Verification

**Files:**
- Modify only if needed during fixes

**Step 1: Run targeted suites**

```bash
cargo test -p sengoo-compiler async_tests
cargo test -p sgc async_native_runtime_
```

**Step 2: Run package suites**

```bash
cargo test -p sengoo-compiler
cargo test -p sgc
```

**Step 3: Run full workspace**

```bash
cargo test
```

### Task 8: Commit and push

**Step 1: Commit**

```bash
git add compiler/src/error.rs compiler/src/lib.rs compiler/src/mir/async_lowering.rs compiler/src/codegen/mod.rs compiler/src/codegen/jit.rs compiler/src/tests/async_tests.rs tools/sgc/src/tests.rs docs/plans/2026-03-10-async-typed-frame-slots-design.md docs/plans/2026-03-10-async-typed-frame-slots.md
git commit -m "feat: support typed async frame slots for scalar values"
```

**Step 2: Push**

```bash
git push -u origin main
```
