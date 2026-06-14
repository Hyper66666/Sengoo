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
  - Partial: the owned `String` checker now marks direct let moves, named-call
    arguments, method-call arguments, and assignment RHS moves. Return moves are
    handled by MIR drop suppression for function exits; general non-`Copy`
    values, field move-out, and path-sensitive move analysis remain open.
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
    and general owning values remain open. Assignment RHS move coverage is now
    included.

## 3. MIR drop-glue insertion

- [ ] 3.1 Add a MIR pass that inserts drop calls for owning locals at scope exit.
  - Partial: top-level stdlib `String` let bindings now get MIR-level
    `String_drop` calls at function exits; straight-line single-exit functions
    use the no-flag fast path only when every dropped binding initializes in
    the entry block. Conditionally initialized bindings use runtime flags even
    when the function has one return. General owning locals and nested scope
    exits remain open.
- [ ] 3.2 Cover early `return`, `?`, `break`, `continue`, and conditional init
  with per-local drop flags.
  - Partial: `?` propagation exits use per-binding runtime drop flags, set false
    at function entry and true after the owning let initializes. Every MIR
    `Return` exit is guarded so values declared before `?` are dropped, values
    declared after `?` are skipped on early propagation, multiple bindings drop
    in reverse declaration order, and moved-from bindings are excluded for the
    implemented move sites: direct `let b = a`, owned tail-expression returns,
    owned named-call arguments, owned method-call arguments, owned assignment
    RHS moves, and explicit `String.drop()` receivers. Explicit `return expr`
    now lowers to a real MIR `Return` exit and reuses the same drop-flag machinery. Loop
    `break`/`continue` currently rejoin before function return; nested scope
    exit timing and partial-move flag clearing remain open.
- [ ] 3.3 Cover the abort path (best-effort release, no re-entrant unwinding).
- [ ] 3.4 Codegen the drop calls in the LLVM-text backend and the Cranelift path.
- [ ] 3.5 IR/codegen tests asserting drop count and order (extend
  `codegen_*`/`struct_codegen` test lanes).
  - Partial: `compiler/src/tests/drop_flag_tests.rs` covers the MIR shape for
    straight-line drop insertion, `?` early-return flags, reverse drop order,
    conditional-init flags, tail-return moves, named-call/method-argument moves,
    assignment moves, explicit `return` exits, explicit drop receivers, and
    moved binding exclusion.

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
