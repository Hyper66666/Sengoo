## 1. Drop trait and semantics

- [ ] 1.1 Add the compiler-known `Drop` trait (`def drop(&mut self)`) to the
  trait/typeck layer and reserve it from manual direct calls.
- [ ] 1.2 Define `Copy` set (integer/float/bool scalars, `&T`) so `Copy` values
  are never moved or dropped.
- [ ] 1.3 Document drop order (reverse declaration order within a scope) in
  `docs/language-features.md`.

## 2. Move / use-after-move checking

- [ ] 2.1 Extend the type checker to mark a local dead after a by-value move
  (argument, return, assignment, field move-out).
- [x] 2.2 Emit a stable `use-after-move` diagnostic and add it to the shared
  `sgc` JSON / `sglsp` code list.
  - Implemented for the current owned `String` move checker; verified by
    compiler, `sgc` JSON, and `sglsp` diagnostic tests. General non-`Copy`
    move analysis remains open under 2.1/2.4.
- [ ] 2.3 Support partial moves: moved-out fields are not dropped; remaining
  fields are.
- [ ] 2.4 Tests under `compiler/src/tests/` for move, partial move, and the
  negative use-after-move diagnostic.
  - Partial: owned `String` negative use-after-move tests exist; partial moves
    and general owning values remain open.

## 3. MIR drop-glue insertion

- [ ] 3.1 Add a MIR pass that inserts drop calls for owning locals at scope exit.
- [ ] 3.2 Cover early `return`, `?`, `break`, `continue`, and conditional init
  with per-local drop flags.
- [ ] 3.3 Cover the abort path (best-effort release, no re-entrant unwinding).
- [ ] 3.4 Codegen the drop calls in the LLVM-text backend and the Cranelift path.
- [ ] 3.5 IR/codegen tests asserting drop count and order (extend
  `codegen_*`/`struct_codegen` test lanes).

## 4. Runtime resource migration

- [ ] 4.1 Make C free functions idempotent in `tools/stdlib/runtime*.c`
  (`Buffer`, collections, `runtime_json`, process, net).
- [ ] 4.2 Add compiler-known `Drop` impls for `Buffer`, `Vec<T>`, `String`,
  `JsonDoc`, `ProcessHandle`, and net handles.
- [ ] 4.3 Re-implement `free()/drop()/close()` wrappers as "explicit early drop"
  that marks the value moved so no double release occurs.

## 5. Opt-in shared ownership

- [ ] 5.1 Add `Rc<T>` library type (non-atomic refcount, `clone`, `Drop`).
- [ ] 5.2 Document `Rc` cycle-leak behavior in `docs/language-features.md`.

## 6. Conformance and docs

- [ ] 6.1 Rewrite `examples/stdlib/20_owned_string.sg` and
  `examples/realworld/cli-json-audit/src/main.sg` to use auto-drop (no manual
  release) as new committed examples; keep the originals as compatibility smoke.
- [ ] 6.2 Update `examples/realworld/SUPPORT_MATRIX.md` memory-safety row.
- [x] 6.3 Run `openspec validate automatic-memory-management --strict`.

## Verification

- `cargo test -p sengoo-compiler --lib`
- `cargo test -p sgc core_conformance_examples_compile_link_and_run`
- New move/drop unit tests (tasks 2.4, 3.5)
- Auto-drop example runs with zero manual release and no leak under a
  leak-check build (task 6.1)
