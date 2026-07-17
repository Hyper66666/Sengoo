# WASM Review Closure Implementation Plan

> **For Codex:** Execute this plan task-by-task with test-driven development.

**Goal:** Close the remaining scalar WASM correctness and OpenSpec-truth gaps found by the follow-up review.

**Architecture:** Keep the backend explicitly experimental. Preserve integer width and signedness in portable cast instructions, reject unsupported references, and make artifact validation fully parse the required `main` export. Align the child change contract with the narrowed canonical specification.

**Tech Stack:** Rust, Sengoo MIR, direct WebAssembly encoding, Cargo integration tests, OpenSpec.

---

### Task 1: Align the active OpenSpec change

**Files:**
- Modify: `openspec/changes/wasm-backend-v1/proposal.md`
- Modify: `openspec/changes/wasm-backend-v1/design.md`
- Modify: `openspec/changes/wasm-backend-v1/tasks.md`
- Modify: `docs/architecture/wasm-emitter-decision.md`

Rewrite the owner change around the experimental scalar scope. Reopen or remove claims for WASI, ownership/Drop, memory/output limits, and multi-OS CI that are not implemented.

### Task 2: Lock the remaining failures with tests

**Files:**
- Modify: `tools/sgc/src/portable_backends.rs`
- Modify: `tools/sgc/tests/portable_targets.rs`

Add tests that prove narrowing casts match native behavior, non-function or truncated `main` exports fail validation, and `GlobalRef` never lowers to zero. Run the focused tests and confirm they fail for the expected reasons.

### Task 3: Implement the minimal backend fixes

**Files:**
- Modify: `tools/sgc/src/portable_backends.rs`

Add a typed portable cast representation with width-aware truncate/sign-extend/zero-extend behavior in the VM and WASM emitter. Reject `GlobalRef`. Parse export kind and index completely, and reject imports for the pure-core experimental artifact profile.

### Task 4: Verify and close

Run `cargo fmt`, focused compiler/sgc tests, strict OpenSpec validation, and the prior manual native/WASM reproductions. Keep `language-maturity-roadmap` open unless every narrowed child gate is genuinely satisfied.
