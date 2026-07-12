## 1. dyn Drop metadata and semantics

- [x] 1.1 Extend per-`(trait, concrete)` vtable globals with a `drop` slot and
  size/align entries alongside the existing method slots.
- [x] 1.2 Lower dropping a `dyn Trait` value (scope exit and explicit early
  drop) to an indirect call through the vtable `drop` slot; concrete types
  without `impl Drop` get a no-op slot.
  - Owned `let s: dyn Trait = value` bindings coerce through a stack slot into
    the fat pointer and register the per-trait `__dyn_Trait_Drop_drop` helper,
    which loads vtable slot 0 and calls the erased drop thunk indirectly
    (guarded against null/no-op slots).
  - Explicit `value.drop()` on an owned dyn value dispatches through the same
    helper and suppresses the scope-exit drop, so `Drop` runs exactly once.
- [x] 1.3 Tests: dropping a `dyn` value runs the concrete `Drop` exactly once
  (IR shape + native runtime handle-count proof), and no-op slots skip the
  call.
  - IR shape tests cover the owned drop helper synthesis, the null-slot guard,
    single-call explicit early drop, and JIT lowering.
  - Native handle-count tests prove scope-exit and explicit early drop each
    release the runtime handle exactly once.

## 2. dyn dispatch surface

- [x] 2.1 Support `&mut self` receivers through dyn dispatch in the LLVM-text
  and JIT text paths.
- [x] 2.2 Emit stable diagnostics for still-unsupported `dyn A + B`
  (`dyn-multi-trait-unsupported`) and `Box<dyn Trait>`
  (`dyn-box-unsupported`) instead of internal errors.
- [x] 2.3 Tests: `&mut self` dispatch mutates through the fat pointer; the two
  unsupported forms produce their stable codes in compiler, `sgc` JSON, and
  `sglsp` lanes.

## 3. Hasher object protocol

- [x] 3.1 Define `Hasher` in the stdlib (`write_i64`, `write_bool`,
  `write_str`, `write_string`, `finish() -> i64`) backed by a deterministic
  runtime hash state.
- [x] 3.2 Allow `impl Hash for T` to define `hash_into(&self, h: &mut Hasher)`;
  synthesize the `hash() -> i64` bridge that drives `hash_into` through a
  fresh `Hasher`, mirroring the `Formatter`/`fmt` bridge.
- [x] 3.3 Route `#[derive(Hash)]` through `hash_into` for structs with
  hashable fields while keeping the existing generated `hash()` helper
  source-compatible.
  - Derives generate a `hash_into(&self, h: &mut Hasher)` body plus the
    `hash()` bridge whenever a `Hasher` surface is reachable; programs
    without a hasher keep the standalone FNV-1a `hash()` body.
  - Method calls resolved through a pointer receiver now load the receiver
    before the call so the `&mut Hasher` parameter matches the by-value
    receiver ABI of method definitions.
- [x] 3.4 Tests: custom `hash_into` impls satisfy `Hash` bounds; derived and
  custom hashes agree with the runtime hash state; format/derive lanes stay
  green.
  - Custom `hash_into` satisfies `Hash` bounds and drives the synthesized
    bridge; stdlib `Hasher` has a native runtime byte-state test.
  - Native `stdlib_surface_runtime_derived_hash_matches_manual_hash_into`
    proves derived hashes match manual `Hasher` writes at runtime.

## 4. Flow-sensitive borrowed views

- [x] 4.1 Track borrowed-view data flow through local reassignment chains
  (`let a = owner.as_str(); let b = a; return b;`) in the borrow checker.
- [x] 4.2 Keep `cannot-move-borrowed` accurate when the original view binding
  is dead but a rebound alias is still live.
- [x] 4.3 Tests: reassignment-chain escapes report `borrow-escapes-scope`;
  rebound-alias owner moves report `cannot-move-borrowed`; negative tests
  confirm dead views release the owner.

## 5. P0 gate closure

- [x] 5.1 Update `automatic-memory-management` tasks with the dyn-drop
  completion notes, run `openspec validate automatic-memory-management
  --strict`, and archive it; tick roadmap 1.1.
  - Archived as `2026-07-04-automatic-memory-management`; roadmap 1.1 ticked.
- [x] 5.2 Update `generics-and-trait-system` tasks 3.2/3.3/3.4/5.2 with the
  completion notes, run `openspec validate generics-and-trait-system
  --strict`, and archive it; tick roadmap 1.2.
  - Derive-Hash now routes through `hash_into`; archived as
    `2026-07-04-generics-and-trait-system`; roadmap 1.2 ticked.
- [x] 5.3 Update `examples/realworld/SUPPORT_MATRIX.md` rows for dyn dispatch,
  hashing, and borrowed views.
- [x] 5.4 Run `openspec validate p0-gate-closure --strict` and archive this
  change.
  - Validated at 43/43 with `openspec validate --all --strict` and archived
    after the derive-Hash-through-`hash_into` follow-up landed.

## Verification

- `cargo fmt --check`
- `cargo test -p sengoo-compiler --lib`
- `cargo test -p sgc core_conformance_examples_compile_link_and_run`
- `cargo test -p sglsp`
- `openspec validate --all --strict`
