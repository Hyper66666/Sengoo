## 1. dyn Drop metadata and semantics

- [ ] 1.1 Extend per-`(trait, concrete)` vtable globals with a `drop` slot and
  size/align entries alongside the existing method slots.
- [ ] 1.2 Lower dropping a `dyn Trait` value (scope exit and explicit early
  drop) to an indirect call through the vtable `drop` slot; concrete types
  without `impl Drop` get a no-op slot.
- [ ] 1.3 Tests: dropping a `dyn` value runs the concrete `Drop` exactly once
  (IR shape + native runtime handle-count proof), and no-op slots skip the
  call.

## 2. dyn dispatch surface

- [ ] 2.1 Support `&mut self` receivers through dyn dispatch in the LLVM-text
  and JIT text paths.
- [ ] 2.2 Emit stable diagnostics for still-unsupported `dyn A + B`
  (`dyn-multi-trait-unsupported`) and `Box<dyn Trait>`
  (`dyn-box-unsupported`) instead of internal errors.
- [ ] 2.3 Tests: `&mut self` dispatch mutates through the fat pointer; the two
  unsupported forms produce their stable codes in compiler, `sgc` JSON, and
  `sglsp` lanes.

## 3. Hasher object protocol

- [ ] 3.1 Define `Hasher` in the stdlib (`write_i64`, `write_bool`,
  `write_str`, `write_string`, `finish() -> i64`) backed by a deterministic
  runtime hash state.
- [ ] 3.2 Allow `impl Hash for T` to define `hash_into(&self, h: &mut Hasher)`;
  synthesize the `hash() -> i64` bridge that drives `hash_into` through a
  fresh `Hasher`, mirroring the `Formatter`/`fmt` bridge.
- [ ] 3.3 Route `#[derive(Hash)]` through `hash_into` for structs with
  hashable fields while keeping the existing generated `hash()` helper
  source-compatible.
- [ ] 3.4 Tests: custom `hash_into` impls satisfy `Hash` bounds; derived and
  custom hashes agree with the runtime hash state; format/derive lanes stay
  green.

## 4. Flow-sensitive borrowed views

- [ ] 4.1 Track borrowed-view data flow through local reassignment chains
  (`let a = owner.as_str(); let b = a; return b;`) in the borrow checker.
- [ ] 4.2 Keep `cannot-move-borrowed` accurate when the original view binding
  is dead but a rebound alias is still live.
- [ ] 4.3 Tests: reassignment-chain escapes report `borrow-escapes-scope`;
  rebound-alias owner moves report `cannot-move-borrowed`; negative tests
  confirm dead views release the owner.

## 5. P0 gate closure

- [ ] 5.1 Update `automatic-memory-management` tasks with the dyn-drop
  completion notes, run `openspec validate automatic-memory-management
  --strict`, and archive it; tick roadmap 1.1.
- [ ] 5.2 Update `generics-and-trait-system` tasks 3.2/3.3/3.4/5.2 with the
  completion notes, run `openspec validate generics-and-trait-system
  --strict`, and archive it; tick roadmap 1.2.
- [ ] 5.3 Update `examples/realworld/SUPPORT_MATRIX.md` rows for dyn dispatch,
  hashing, and borrowed views.
- [ ] 5.4 Run `openspec validate p0-gate-closure --strict` and archive this
  change.

## Verification

- `cargo fmt --check`
- `cargo test -p sengoo-compiler --lib`
- `cargo test -p sgc core_conformance_examples_compile_link_and_run`
- `cargo test -p sglsp`
- `openspec validate --all --strict`
