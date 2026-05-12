# Sengoo 3-Month Development Roadmap (Rebaselined 2026-04-23)

## Executive Summary
This roadmap is now a cleanup-and-verification plan, not a re-plan of already shipped async foundation work.

The main async phase-2 track is largely shipped. The next real priorities are:

1. doc governance and OpenSpec state cleanup
2. debt split in codegen and MIR lowering
3. remaining feature verification

## Current State

### Shipped or Mostly Shipped
- Async phase-2 main-path work is largely in place on `main`.
- The roadmap should not keep re-tracking completed foundation work.
- The remaining work is mostly alignment, cleanup, and verification.

### Still Open
- OpenSpec state for async work needs cleanup before any further phase-2 expansion.
- `stdlib-generic-support` still has task `4.1` blocked.
- `stdlib-generic-support` task `4.3` looks functionally complete, but the checklist/docs still need alignment.
- `compiler/src/codegen/mod.rs` has been reduced, but `compiler/src/codegen/jit.rs` and `compiler/src/mir/lowering.rs` are still the large debt surfaces.

### What This Means
The roadmap should be shorter and narrower than the old version:
- stop re-planning shipped async foundation items
- treat OpenSpec/doc state as the first cleanup lane
- keep the codegen split focused on the remaining large files
- use tests and native-runtime checks to close out the remaining feature claims

## Next 90 Days

### 1. Doc Governance and OpenSpec Cleanup
Priority: P0

Focus:
- reconcile async-phase-2 state with the current shipped implementation
- align `stdlib-generic-support` task status with actual progress
- remove stale checklist items and stale roadmap language

Outcome:
- the docs and OpenSpec state should match what is actually true on `main`

### 2. Debt Split
Priority: P1

Focus:
- keep shrinking `compiler/src/codegen/mod.rs`
- isolate the remaining work in `compiler/src/codegen/jit.rs`
- continue separating the large lowering debt in `compiler/src/mir/lowering.rs`

Outcome:
- the codegen/MIR cleanup lane becomes explicit instead of being mixed into feature work

### 3. Remaining Feature Verification
Priority: P1

Focus:
- finish verification for the blocked `stdlib-generic-support` repeated-build/cache path
- confirm the checklist item that appears complete is aligned in docs and state
- re-run the remaining async/runtime coverage where it still matters

Outcome:
- the roadmap closes only on verified, current behavior

## Recommended Order

1. Doc governance and OpenSpec cleanup
2. Codegen and MIR debt split
3. Remaining feature verification

## Verification Baseline

Every roadmap item should preserve:

```powershell
cargo test -p sengoo-compiler
cargo test -p sgc
```

Async/runtime changes should also preserve:

```powershell
cargo test -p sgc async_native_runtime_ -- --nocapture
```

## Success Criteria
- No already-completed async foundation work is tracked as future work
- OpenSpec and roadmap state match current implementation status
- The remaining debt split is centered on `codegen/jit.rs` and `mir/lowering.rs`
- Remaining feature claims are closed only after verification
