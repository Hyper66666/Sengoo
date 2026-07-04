## 1. Drop trait and semantics

- [x] 1.1 Add the compiler-known `Drop` trait (`def drop(&mut self)`) to the
  trait/typeck layer and reserve it from manual direct calls.
  - Completed: `TypeChecker::new` seeds `Drop` as a compiler-known trait with
    `drop(&mut self)`, the type checker enforces that contract for trait
    declarations and impls, and direct trait-dispatched `Drop::drop` calls are
    rejected while inherent compatibility release methods keep their existing
    priority. Covered by `compiler/src/tests/drop_trait_tests.rs`.
- [x] 1.2 Define `Copy` set (integer/float/bool scalars, `&T`) so `Copy` values
  are never moved or dropped.
  - Completed: `Ty::is_copy_value` defines the compiler-known Copy baseline for
    integer/float/bool scalars and references; the current owned-value move
    checker consults it before marking owned values moved. Covered by
    `typeck::ty::copy_tests` plus existing owned-`String` move tests.
- [x] 1.3 Document drop order (reverse declaration order within a scope) in
  `docs/language-features.md`.
  - Added "Ownership, moves, and automatic drop" (EN §2.8 / ZH §2.6) covering
    reverse-declaration drop order, early-exit drop flags, move/use-after-move,
    and compiler-called `drop`; honestly scoped to the current owned-`String`
    surface with the broader auto-drop work marked as in progress.

## 2. Move / use-after-move checking

- [x] 2.1 Extend the type checker to mark a local dead after a by-value move
  (argument, return, assignment, field move-out).
  - Completed for the current move-path model: the checker marks direct let moves, named-call
    arguments, method-call arguments, assignment RHS moves, and owning field
    move-outs. A by-value `return` now also marks an owned local or field moved
    for later diagnostics in the same block, while MIR drop suppression keeps
    function exits free of double-drop. General non-`Copy` values implementing
    `Drop` share this tracking. Full NLL-style lifetime analysis remains outside
    this task.
- [x] 2.2 Emit a stable `use-after-move` diagnostic and add it to the shared
  `sgc` JSON / `sglsp` code list.
  - Implemented for the current owned `String` move checker; verified by
    compiler, `sgc` JSON, and `sglsp` diagnostic tests. General non-`Copy`
    move analysis remains open under 2.1/2.4.
- [x] 2.3 Support partial moves: moved-out fields are not dropped; remaining
  fields are.
- [x] 2.4 Tests under `compiler/src/tests/` for move, partial move, and the
  negative use-after-move diagnostic.
  - Covered by owned `String` negative use-after-move tests, user `Drop` type
    move/use-after-move coverage, field move diagnostics, sibling-field access,
    whole-parent partial-move rejection, assignment RHS moves, and field
    assignment reinitialization. Moving an owned root while it has an active
    lexical borrow is rejected with the stable `cannot-move-borrowed` code.
    Borrow tracking now uses the same field-aware move paths: borrowing a field
    blocks moving that field or its parent while disjoint sibling fields remain
    independently movable.

## 3. MIR drop-glue insertion

- [x] 3.1 Add a MIR pass that inserts drop calls for owning locals at scope exit.
  - Completed for the current owning-value surface: top-level stdlib `String`
    let bindings get MIR-level `String_Drop_drop` calls at function exits;
    straight-line single-exit functions
    use the no-flag fast path only when every dropped binding initializes in
    the entry block. Conditionally initialized bindings use runtime flags even
    when the function has one return. User types with `impl Drop` now resolve
    their concrete `<Type>_Drop_drop` function for live locals and by-value
    parameters, while `Drop::drop` receivers are not recursively auto-dropped.
    Lexical blocks, if branches, loop/while/for bodies, and try blocks now drop
    their own bindings at the scope boundary instead of delaying cleanup until
    function return. Tail-expression moves now propagate through `if`/block
    expressions so generic `Result<T, E>::unwrap_or` does not drop an owned
    selected value before returning it. General recoverable unwinding remains
    out of scope until Sengoo has recoverable panic semantics.
  - Cross-note: `p0-gate-closure` added dyn vtable drop metadata and erased
    drop thunks, but source-level owned `dyn Trait` values are not yet a
    complete owning-value surface. Scope-exit/explicit drop through a dyn
    vtable remains tracked there instead of being counted as closed by this
    AMM change.
- [x] 3.2 Cover early `return`, `?`, `break`, `continue`, and conditional init
  with per-local drop flags.
  - Completed for the current MIR drop-glue surface: `?` propagation exits use
    per-binding runtime drop flags, set false at function entry and true after
    the owning let initializes. Every MIR `Return` exit is guarded so values
    declared before `?` are dropped, values declared after `?` are skipped on
    early propagation, multiple bindings drop in reverse declaration order, and
    moved-from bindings are excluded for the implemented move sites: direct
    `let b = a`, owned tail-expression returns, owned named-call arguments,
    owned method-call arguments, owned assignment RHS moves, field moves, and
    explicit `String.drop()` receivers. Explicit `return expr` now lowers to a
    real MIR `Return` exit and reuses the same drop-flag machinery. Nested
    lexical scopes emit cleanup before normal exit, explicit `return`, `?`
    propagation, try-block propagation, `break`, and `continue`. Partial-move
    field-state clearing and field reinitialization are covered.
- [x] 3.3 Cover the abort path (best-effort release, no re-entrant unwinding).
  - Completed: MIR drop glue treats `Unreachable` contract-failure aborts as
    cleanup exits alongside `Return`, emits initialized-flag-guarded drop
    chains, and preserves the final `Unreachable` terminator. Covered by
    `drop_flags_guard_contract_abort_path_for_initialized_binding`.
- [x] 3.4 Codegen the drop calls in the supported LLVM-text backends.
  - Completed: the LLVM-text backend now receives unit-typed auto-drop calls
    for user `impl Drop` functions, avoiding bogus boolean return calls. The
    `JITCodegen` LLVM-text path has matching coverage for the same concrete
    user Drop call ABI. The separate `sgc` Cranelift fast-JIT is not a general
    MIR/drop-glue backend in this change; it evaluates a constant-expression
    subset before emitting a trivial Cranelift `main`, and
    `cranelift_fast_jit_rejects_non_constant_runtime_calls` pins that boundary.
    A future Cranelift MIR backend must add its own drop-glue conformance lane.
- [x] 3.5 IR/codegen tests asserting drop count and order (extend
  `codegen_*`/`struct_codegen` test lanes).
  - Completed for the supported LLVM-text/native path:
    `compiler/src/tests/drop_flag_tests.rs` covers the MIR shape for
    straight-line drop insertion, `?` early-return flags, reverse drop order,
    conditional-init flags, tail-return moves, named-call/method-argument
    moves, assignment moves, explicit `return` exits, explicit drop receivers,
    and moved binding exclusion. It now also covers user `impl Drop` live
    locals, by-value parameters, non-recursive `Drop::drop` receivers, and
    LLVM-text unit-return codegen for user auto-drop calls in both LLVM-text
    codegen and the `JITCodegen` text path. Nested block/branch/loop/try
    tests assert cleanup occurs in the exiting CFG block before control leaves
    the lexical scope, and `for_scope_drops_before_break` covers the distinct
    `for` lowering path. Composite owning-field tests now cover reverse field
    drop order, partial-move skip/drop behavior, field moves through calls and
    returns, and field reinitialization restoring scope-exit drop.
    Contract abort cleanup is covered by a runtime-contract-checked
    postcondition-failure path that drops initialized owned locals before the
    final `Unreachable` terminator.
    `stdlib_owned_result_unwrap_or_moves_value_without_dropping_it_first`
    covers the generic Result branch-move regression that previously freed
    returned `Buffer`/`JsonDoc` handles before realworld code could use them.
    Native `sgc` runtime smoke coverage proves stdlib owning handles and
    generic `Rc<T>` payload resources return to their live-handle baselines
    after scope exit.

## 4. Runtime resource migration

- [x] 4.1 Make C free functions idempotent in `tools/stdlib/runtime*.c`
  (`Buffer`, collections, `runtime_json`, process, net).
  - Completed: String and Buffer generation-slot releases return success on
    repeated release of the same live-generation handle; JsonDoc and process
    command/output/handle close paths now keep/recognize a closed shell or slot
    so repeated close returns success instead of double-freeing. Covered by
    `tools/sgc/src/tests.rs::stdlib_runtime_release_functions_are_idempotent_for_core_handles`.
    Vec/HashMap/text collection values and iterators now use a shared
    generation-slot opaque-handle table: release frees the payload/control
    block, stale handles no longer resolve, repeated release is a no-op, and
    the conformance test proves the opaque live-handle count returns to zero.
    Rust native TCP/UDP/HTTP response/HTTP server/request/WebSocket ABI close
    functions treat an already-absent handle as successfully closed while
    preserving strict per-`NetRuntime` instance isolation. The C-only net
    fallback follows the same close contract and is covered by
    `stdlib_c_fallback_net_release_functions_are_idempotent`.
- [x] 4.2 Add compiler-known `Drop` impls for `Buffer`, `Vec<T>`, `String`,
  `JsonDoc`, `ProcessHandle`, and net handles.
  - Completed for the current concrete stdlib handle surface: `Buffer`,
    `Vec<i64>`, `Vec<bool>`, `Vec<String>`, `JsonDoc`, `ProcessCommand`,
    `ProcessOutput`, `ProcessHandle`, `TcpStream`, `UdpSocket`, `HttpClient`,
    `HttpServer`, `HttpServerRequest`, and `WsClient` now implement `Drop`
    and auto-release at local scope exits. The collection surface also covers
    all concrete `HashMap<K,V>` combinations, `TextList`, the i64/bool/string
    text maps, and their heap-backed iterator handles. `String` now has a real
    `impl Drop for String` while its old `String.drop()` compatibility method
    remains available. Legacy by-value handle APIs are temporarily treated as
    idempotent borrow-like wrappers for move checking, and callee parameters of
    those legacy handle types are not auto-dropped, because the current public
    stdlib methods still pass handles by value rather than through `&self`.
    Covered by `stdlib_owned_handles_auto_drop_without_manual_release` plus the
    existing `stdlib_surface` suite.
- [x] 4.3 Re-implement `free()/drop()/close()` wrappers as "explicit early drop"
  that marks the value moved so no double release occurs.
  - Completed in MIR lowering for the compatibility method names `drop`,
    `free`, and `close`: calling one of these methods marks the receiver moved
    so scope-exit auto-drop is suppressed. Verified by
    `explicit_drop_method_consumes_receiver_for_drop_glue` and by
    `cargo run -p sgpm -- test --locked --manifest-path
    examples/realworld/cli-json-audit/Sengoo.toml`, whose smoke test keeps
    legacy `doc.close()`, `scores.free()`, and `buffer.free()` calls.

## 5. Opt-in shared ownership

- [x] 5.1 Add `Rc<T>` library type (non-atomic refcount, `clone`, `Drop`).
  - Completed for the current source surface: `tools/stdlib/collections.sg`
    now exposes `Rc<T>` with the
    verified scalar constructors `rc_new_i64` and `rc_new_bool`; `Rc<i64>` and
    `Rc<bool>` support `clone`, `get`, `strong_count`, `is_unique`, and
    compiler-inserted `Drop` backed by a non-atomic C runtime refcount control
    block. `Rc<String>` now has a first owning-value slice: construction clones
    the source `String` into the Rc control block, `get` returns a cloned
    `String`, and final refcount release drops the stored string handle. Generic
    `rc_new<T>(value)` now lowers through a compiler-only ABI that copies the
    moved payload bytes into the Rc control block with the concrete MIR
    size/alignment and records a compiler-generated typed drop thunk; final
    release invokes that thunk exactly once, so user aggregates containing owned
    fields release their nested resources after the last clone drops. The
    `RcValue` trait gives generic functions a bound-based construction path
    for the verified scalar/string payloads (`value.rc()` / `T: RcValue`) while
    the direct `rc_new<T>` path covers arbitrary monomorphized owning payloads.
    `rc_new<T>` also materializes temporary payload expressions into a hidden
    stack slot before copying them into the control block, avoiding invalid
    address-taking IR for expressions such as `rc_new(21)`.
    `Rc<T>` now has one runtime representation (`handle` only, with no
    duplicated `marker: T` payload), and generic `clone`/`Drop` impls work
    through nested `&Rc<T>` calls and monomorphized MIR. `borrow() -> &T` now
    has a compiler-known read slice for generic payloads: the runtime exposes
    the payload address, MIR casts it to the concrete shared reference type, and
    dereferencing that reference reads the shared value; borrowed aggregate
    scalar fields also project through `(*rc.borrow()).field`. Moving an
    `Rc<T>` owner while a borrow produced by `borrow()` is live is rejected with
    the stable `cannot-move-borrowed` diagnostic. Covered by compiler surface
    and native sgc runtime smoke tests, including
    `stdlib_surface_runtime_rc_generic_payload_drops_once_after_last_release`,
    `stdlib_surface_runtime_rc_generic_borrow_reads_shared_payload`,
    `stdlib_surface_runtime_rc_generic_payload_accepts_temporary_value`, and
    `stdlib_surface_runtime_rc_generic_borrow_reads_aggregate_field`, plus
    `rc_owner_cannot_move_while_borrow_is_live`. Projecting owned fields by
    value through a borrow remains a broader reference-field/lending limitation,
    not an `Rc<T>` lifecycle blocker.
- [x] 5.2 Document `Rc` cycle-leak behavior in `docs/language-features.md`.
  - Added the `Rc` shared-ownership section with the current verified API,
    move-only default reminder, and explicit cycle-leak behavior.

## 6. Conformance and docs

- [x] 6.1 Rewrite `examples/stdlib/20_owned_string.sg` and
  `examples/realworld/cli-json-audit/src/main.sg` to use auto-drop (no manual
  release) as new committed examples; keep the originals as compatibility smoke.
  - Completed: both committed examples now omit manual release calls on the
    owning values they create, while `cli-json-audit/tests/audit_smoke.sg`
    remains the compatibility smoke for explicit release methods.
  - Native leak harness:
    `cargo test -p sgc stdlib_auto_drop_releases_all_generation_handles
    -- --nocapture` creates owned String/Buffer values in a callee and proves
    their runtime live-handle counts return to zero after scope exit. The same
    fixture now creates Vec/HashMap/TextList/string-map values and proves the
    shared opaque collection-handle count also returns to zero. This is a
    deterministic ownership-resource check; allocator-level ASan/LSan coverage
    remains a separate CI hardening follow-up because slot-table capacity is
    process-global by design. `Rc<T>` generic payload storage is covered by a
    native runtime smoke that shares a user aggregate containing `String` and
    proves the string live-handle count returns to its baseline after the final
    Rc release.
- [x] 6.2 Update `examples/realworld/SUPPORT_MATRIX.md` memory-safety row.
  - Added the "Automatic drop / move ownership" support row with compiler,
    stdlib example, realworld, and scalar `Rc` proof points.
- [x] 6.3 Run `openspec validate automatic-memory-management --strict`.

## Verification

- `cargo test -p sengoo-compiler --lib`
- `cargo test -p sgc core_conformance_examples_compile_link_and_run`
- New move/drop unit tests (tasks 2.4, 3.5)
- Auto-drop native fixture runs with zero manual release and zero live
  generation handles after scope exit (task 6.1)
