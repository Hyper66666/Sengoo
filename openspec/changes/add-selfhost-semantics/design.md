# Semantic core design

Use integer identities into owning arenas rather than recursive Rust ownership
objects. Structural type identity includes kind, qualifiers, nominal symbol
identity and ordered type arguments. Nominal declarations refer to separately
resolved member tables so recursive names do not require recursive value
allocation. Layout later rejects infinitely sized by-value cycles while
allowing supported indirection.

Resolve names using explicit module/scope/symbol identities, not source-text
matching. Keep source/expansion mappings attached to diagnostics and checked
nodes. Generic substitutions and specialization keys use canonical type IDs
and stable symbol order. Scope inference variables and constraints to their
owning analysis request; no state or mutable borrow escapes a request snapshot.

Ownership analysis must reproduce documented last-use/temporary/resource
boundaries. Supported implicit native borrows keep their inferred/declared
contracts, fresh temporary owners survive their consumers, and all exit paths
emit correct cleanup. The known seed temporary owning-result field-projection
leak is a regression to repair, not behavior to emulate. CFG merge, early
return, try propagation, moves and cancellation receive independent tests.

Typed HIR preserves resolved callable/type identities. MIR represents locals,
places, instructions, basic blocks and terminators explicitly, validates every
type/reference before backend entry, and records target pointer width and the
runtime/FFI contract. The future bootstrap exchange adapter is versioned and
test-only; it must not introduce a Rust dependency into SG self-builds.

Stage implementation from type identity through name/type resolution, generic
constraints, ownership and typed lowering. Run real positive/negative semantic
fixtures and observable generated-program behavior as each behavior lands.
Keep the entire M3 checklist open until all supported semantic and lowering
gates pass; no scalar-only path qualifies the complete milestone.

## Reference difference ledger

Semantic comparison uses the current Rust `Parser` plus a fresh `TypeChecker`
for every request. The frozen stage0 binary is a separate comparison input;
its embedded revision and binary digest must not stand in for the current
source oracle. A reported mismatch is never counted as parity merely because
the SG implementation's own assertions pass.

- Transparent aliases: `type Value = i64; def main() -> Value { 42 }` and the
  corresponding generic alias currently fail in the Rust checker because the
  alias remains nominal at the use site. Resolve aliases to their declared
  targets, substitute their generic arguments/defaults and reject alias
  cycles. This is a named correction to the declared alias capability, with
  executable fixtures in `selfhost/tests/semantic.sg`.
- Soundness and documented rejection boundaries: a non-exhaustive boolean
  match, `break` outside a loop, and a trait method implementation with the
  wrong return type are accepted by the current standalone Rust semantic
  oracle. The SG checker rejects these with located codes 5111, 5109 and 5104.
  Retain these named negative fixtures and classify the differences explicitly
  rather than weakening the checker or claiming zero differential failures.
- Duplicate function names currently overwrite the value lookup in the Rust
  environment. Preserve that lookup behavior during this port; do not add a
  blanket duplicate-function rejection. A future overload/code-generation
  policy needs its own evidence and compatibility decision.
- The current parser consumes `::` before `<` in `id::<i64>(42)` and constructs
  comparison expressions rather than an explicit generic call. Keep the
  malformed expression rejected in this change; do not bypass that AST in the
  semantic checker or claim that the spelling already works. An explicit
  generic-call syntax correction requires a separate parser change and AST
  regression evidence.
- The stage0 implementation's temporary owning aggregate field-projection
  cleanup and borrowed concrete `Vec<i64>::get` specialization mismatch are
  known implementation defects. SG implementation code uses explicit owners
  and the existing typed collection ABI while bootstrapping; the new semantic
  model must keep the source-level owner and method return type correct.

The first named-fixture comparisons are development evidence, not the complete
M3 compatibility gate. Record the oracle/candidate hashes, every mismatch and
its disposition; generic bounds, associated projections, ownership, HIR/MIR
and whole selfhost source validation remain required before milestone closure.

Fresh reference comparisons are authoritative only when the oracle completes;
stack overflow or process crashes are recorded as oracle faults and never
converted into SG passes. Existing reference quirks and soundness corrections
remain in this explicit compatibility ledger. The SG checker may reject an
unsound accepted form only with a stable diagnostic and a named corpus case.

## Checked callable and class facts

Bindings retain eight words: symbol, lexical scope, binding kind, type,
declaration AST ID, flags, owner binding ID and auxiliary value. Kind 15 is a
closure callable; its declaration is the lambda AST, its owner is the outer
callable and its auxiliary value is the closure scope. Closure parameters and
locals refer to that binding ID. Owner fields never contain AST IDs.

After argument inference, a call's callee node carries its normalized,
instantiated function type. The resolved binding retains its declaration
template. Projection normalization and generic obligations run after inference;
type applications also check their declared bounds. Built-in signatures have
declaration ID -1 and flag 4096 and do not introduce executable bodies.
`call_types` has one entry per AST node and exposes the same callable signature
through `sh_node_callable_type`; non-call nodes use -1. Ordinary and built-in
call facts carry the actual ordered argument types and return type. HIR schema 2
owns and serializes this table rather than recovering it from source spelling.

Class inheritance keeps the declared parent type, substitutes actual generic
arguments through it, and exposes fields in parent-first order through
`sh_class_parent`, `sh_layout_field_count` and `sh_layout_field_binding`.
Inherited fields preserve the original field binding; `sh_resolved_field_type`
resolves that binding in the actual derived type. MIR may normalize the shared
nominal type to its explicit class/enum representation without changing these
source binding identities.

Located code 5122 covers the existing FFI signature contract, including ABI,
supported parameter/return shapes, generic extern rejection and the raw-pointer
declaration marker. Code 5123 covers core trait declaration/implementation
contracts, conflicting implementations and class hierarchy errors. It does not
invent a new unsafe-call syntax: the current supported grammar has no ordinary
unsafe function or unsafe expression block.

Direct `Drop::drop` calls remain rejected outside the existing explicit-field
destruction pattern. The native regression
`explicit_field_drop_free_and_close_suppress_automatic_field_cleanup` requires
`self.child.drop()` inside a parent's destructor to run once and suppress the
child's automatic cleanup. Preserve that scoped case and pass its ownership
fact to lowering; do not skip implementation-body checking globally.

## Future and dynamic type contracts

A Future may be stored directly in a local variable and awaited later. Future
values inside aggregate storage, ordinary function arguments or escaping return
values are rejected. Direct `let scope = task_scope()` is the supported lexical
TaskScope introduction; constructing the capability or storing it in another
aggregate is rejected. AsyncContext remains opaque and poll-scoped. An earlier
development checker rejected all local Future storage; that was an implementation
error, not a supported-subset restriction or an approved reference exception.

User Future implementations retain their declared output type. The checker
validates a mutable receiver, the AsyncContext parameter and Poll layout before
accepting await. A separate path scan checks evident Pending constructions for a
prior `wake` or `wake_after` call on that function's context binding; wake calls
on unrelated values and wake registration in only one branch do not satisfy an
unregistered Pending path.

Type kind 14 has flags 0 for trait bounds and flags 1 for dynamic trait objects.
The latter's ordered arguments are kind-15 entries with flags 1: data names the
trait symbol, nominal names the associated item, and the sole argument is its
fixed type. Kind 15 with flags 0 remains an associated projection whose sole
argument is the base type. Type construction rejects other flags, mismatched
trait identities, duplicate associated items and nested binding payloads before
publishing an ID. Object safety and fixed-associated-type coercions are checked
against declarations; associated entries are type metadata, not runtime payload
fields.
