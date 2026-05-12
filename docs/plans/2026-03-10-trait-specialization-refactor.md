# Trait Specialization Refactor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Move remaining trait method specialization naming and registration planning out of MIR `lowering.rs` and into `generic_methods.rs`.

**Architecture:** Keep runtime behavior unchanged. Extend the existing trait template collector so it also returns eager, already-renamed non-generic trait impl/default methods that `lowering.rs` can register and lower without rebuilding trait-specific naming logic inline.

**Tech Stack:** Rust, Sengoo HIR/MIR lowering, existing unit tests in `compiler/src/mir/generic_methods.rs` and integration tests in `compiler/src/tests/trait_tests.rs`.

---

### Task 1: Lock the boundary with a failing unit test

**Files:**
- Modify: `compiler/src/mir/generic_methods.rs`
- Test: `compiler/src/mir/generic_methods.rs`

**Step 1: Write the failing test**

Add a unit test that asserts trait collection returns:
- generic templates for generic trait methods
- eager renamed functions for non-generic impl/default trait methods
- no unspecialized generic eager function

**Step 2: Run test to verify it fails**

Run: `cargo test -p sengoo-compiler collect_trait_method_templates_for_impl_collects_eager_trait_functions`

Expected: FAIL because the collection type does not yet expose eager trait functions.

### Task 2: Implement the lowering plan in `generic_methods.rs`

**Files:**
- Modify: `compiler/src/mir/generic_methods.rs`

**Step 1: Add plan data**

Extend `TraitMethodTemplateCollection` with an eager function list that contains non-generic trait impl/default methods already normalized for lowering.

**Step 2: Build eager impl/default entries**

During collection:
- keep generic trait methods in `templates`
- rename non-generic impl methods to three-part names and add them to eager functions
- synthesize non-generic default trait methods with injected `self` when needed and add them to eager functions

**Step 3: Keep implemented-name tracking**

Preserve `implemented_method_names` so default methods are still filtered correctly.

### Task 3: Make `lowering.rs` orchestration-only

**Files:**
- Modify: `compiler/src/mir/lowering.rs`

**Step 1: Replace inline trait registration logic**

Use the returned collection from `generic_methods.rs` to register known trait function names/signatures instead of rebuilding names in `lowering.rs`.

**Step 2: Replace inline eager lowering logic**

Lower eager trait functions from the collection rather than reconstructing impl/default functions inline.

**Step 3: Leave generic specialization behavior unchanged**

Keep generic trait method specialization driven by `TraitMethodTemplate`.

### Task 4: Verify focused regressions

**Files:**
- Test: `compiler/src/mir/generic_methods.rs`
- Test: `compiler/src/tests/mir_generic_methods_tests.rs`
- Test: `compiler/src/tests/trait_tests.rs`

**Step 1: Run focused tests**

Run:
- `cargo test -p sengoo-compiler collect_trait_method_templates_for_impl_`
- `cargo test -p sengoo-compiler mir_generic_methods`
- `cargo test -p sengoo-compiler trait_tests`

**Step 2: Run broader compiler verification**

Run: `cargo test -p sengoo-compiler`

Expected: pass with existing ignored count unchanged.
