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


## Operator, contract and unit impl method compatibility

Arithmetic operator nodes retain a resolved method binding and an instantiated
`call_types` signature, including the declared Rhs and Output types. Generic
operator bounds retain those arguments; primitive arithmetic satisfies only the
matching primitive Rhs/Output combination. Trait declarations and implementations
are checked against their receiver, argument and output contracts. A complete
lowering implementation must consume these facts rather than emit scalar
instructions for a user-defined operator.

Supertrait names must resolve to traits, their graph must be acyclic, and an
implementation must satisfy transitive supertrait obligations. A generic bound
also introduces its supertraits into method lookup. Or-pattern alternatives bind
the same names and compatible types to shared binding IDs. Function requires and
ensures expressions are boolean; ensures exposes a scoped result binding. Code
5127 identifies directly contradictory literal postconditions, while arbitrary
contract verification remains a separate obligation.

For compatibility with the actual reference, an impl method with no
written return type has a declared unit return and may discard its body tail
value. The complete body is still checked, and explicit return statements obey
that declared unit contract. This rule does not apply to ordinary functions or
methods with an explicit return type. Class methods retain strict unit tail checking. Named cases use Counter.bump with a boolean
tail and reject an ordinary boolean-tail unit function, explicit `->()` boolean
tail, and explicit `return true` in an implicit-unit method. MIR must discard the
tail according to the declaration and return unit; the source-to-native gate
records execution separately from typed acceptance.

Format templates follow the current shared mini-language: automatic and explicit
positions, debug style, right width, precision and escaped braces. Required
arguments use max(automatic count, greatest explicit index + 1), so a repeated
explicit position consumes one argument. Invalid specifiers remain located
errors rather than being counted as arbitrary placeholders.

The standalone Rust type checker accepts extracted fixture 625, whose Hash body
uses Hasher methods without importing their definitions. Qualified sgc check
rejects the same source at MIR lowering for missing write_i64, write_bool,
write_str and finish. This remains a typed differential gap and a reference
phase inconsistency; it is not reclassified as a typed negative or counted as
full-source native support. No invented implementation is registered to hide the
missing module context.

Actual selfhost and standard-library single-file discovery records the missing
module context explicitly. Imports do not yet load dependencies at this entry,
and five accepted files out of the initial 96-file scan do not establish module
resolution or self-compilation. Shared module graphs, canonical module identity,
source ownership, standard-library signatures and intrinsic validation remain
required before M3/M7 closure.


The subsequent class source-native comparison caught an overbroad implementation
of the impl-only effect-tail rule. A class Counter.bump with implicit unit and a
boolean tail is rejected by both the fresh standalone Rust checker and qualified
sgc check. The SG implementation is corrected to owner kind 14 (impl) only;
class owner kind 3 remains strict. This is an SG bug correction, not a reference
phase inconsistency. The original 178-case report remains historical evidence;
the named class negative is included in the next source-bound replay.


## Physical module graph and transparent import scopes

`ShModules` retains a FileSnapshot for every physical input. Stable file identity
deduplicates aliases before publishing ModuleIds; the matching SourceId owns the
strict UTF-8 bytes. DFS rejects a visiting dependency, records located import
edges, respects source-relative candidates before module-map entries, and emits
local dependencies before their importers. Standard modules follow the current
reference dependency policy and fixed source order. Limits bound module count,
combined input bytes and joined output. Input snapshots are revalidated before
publishing a result. Missing imports, cycles, malformed maps, limits, I/O,
changed inputs and invalid UTF-8 have distinct 5201 through 5207 diagnostics.

Joined-source mappings retain each original SourceId and byte interval and
compose with macro/attribute/derive/protocol mappings before CLI diagnostics.
Import discovery performs macro/attribute processing without running external
derive commands; the final joined frontend executes derive once. The module
checker records node, binding and lexical-scope provenance, explicit module
bindings/scopes and exported symbol-to-binding rows. ShChecked and HIR schema 2
are unchanged; these module facts belong to the owning ShModuleChecked wrapper.
They must be included by a future module-aware HIR serialization version before
claiming cross-process preservation.

Current reference source imports are transparent: imported declarations enter a
shared environment. Real qualified sgc probes accept `import worker; child()`
but reject `worker::child()` and an alias-qualified spelling when worker declares
only `child`. Module scopes therefore explicitly record provenance alongside the
shared import scope; they do not invent qualified source namespaces. Existing
qualified symbols and associated calls retain their checked binding identity.

The first module-aware actual-source discovery accepts std::assert, std::option
and std::result entry wrappers through ownership. std::string passes typing but
has ownership gaps. core, lexer and semantic entry modules reach the same
collections generic/type obligations after their real dependencies are loaded.
Those located failures remain required work, not a successful self-compilation
or complete module/stdlib milestone.


## Deferred generic association equalities

`deferred_types` stores four-word rows: callable binding ID, expected type ID,
actual type ID and the original comparison AST ID. Definition-time deferral is
limited to impl bodies whose compatible outer type shape contains an unresolved
associated projection based on generic parameters. Known scalar/shape errors are
still rejected. There is no general kind-15 wildcard and no skipped impl body.
The table is bounded by the syntax-node budget and retains located errors on
exhaustion.

Every instantiated callable rechecks these equalities after replacing its
parameters and normalizing concrete associated types. Calls are revisited to a
bounded fixed point after all bodies are checked, so forward declarations and
generic wrappers cannot bypass an obligation discovered later. Unresolved
conditions propagate to their calling generic body; concrete mismatches are
rejected. The original comparison node need not have the actual type as its node
type: it may be an explicit return or block whose control-flow type differs.
HIR schema 3 preserves the table with checked owner/type/origin relationships.
MIR must reject an unresolved or unequal concrete job rather than emit code.

The native semantic regressions use Source, Iter<T> and Take<I:Source,T>:
Take<Iter<i64>,i64> closes its result equality, while a boolean marker is rejected,
including a forward declaration and a generic wrapper. The remaining unspecialized
stdlib equations are explicit conditions, not evidence that all generic programs
already lower or self-compile.

Impl methods also retain a distinct discarded-expression context for non-tail
statements and while/for bodies. Every subexpression and explicit return is
checked, while a discarded conditional executes its branches and yields unit
without forcing the branch values to match. Ordinary/class methods and value
contexts retain their strict rules. Two actual qualified-reference O0 programs
with boolean effect calls in impl while conditionals execute with result 42;
the corresponding ordinary-function no-else form is rejected.

The existing reference reserves spawn_blocking_i64 and
spawn_blocking_future_i64 source call contracts, including their async-qualified
forms, for Send capture checking. The SG checker applies the same constraint to
resolved lambda capture bindings even when the source declares a wrapper with
that name. This typing rule does not authorize backend dispatch by source
spelling or fabricate a native runtime implementation.


## Borrowed string views and per-file BOM provenance

The synthetic String.as_str callable signature borrows shared storage from its
receiver. Owned, shared and mutable String inputs all produce a shared &String
receiver parameter and &str result; they do not consume the String or upgrade
mutability. Ownership analysis must retain the anonymous returned loan and its
storage owner even when the type has no destructor. A borrowed view leaving a
temporary storage lifetime is rejected, while full-expression observation and
borrowed-parameter forwarding retain their valid lifetimes.

A leading UTF-8 BOM belongs to an individual physical source file. Module merge
replaces exactly those first three bytes with three spaces, preserving joined
length and the original SourceId byte map. Original source snapshots retain the
BOM bytes. Interior BOM characters remain lexical errors; this does not inherit
the reference lexer's general behavior of skipping arbitrary error tokens.


## Explicit intrinsic ABI facts and ordinary Future annotations

`builtin_calls` stores six-word records: source call AST ID, intrinsic kind,
receiver nominal type, runtime declaration binding, language function type and
runtime function type. The first kind is the shared String.as_str view. Its
language signature is shared &String to shared &str; the runtime ABI contract is
a real external-C declaration of sengoo_string_as_str_ptr(i64) -> i64 and the
standard non-generic String layout with its own signed-i64 first field. A missing declaration publishes
no ABI authorization; an incompatible declaration or layout is a located 5122
error. Type-only lifetime analysis may still retain the language signature, but
native lowering cannot infer an available implementation from that signature.

`sh_builtin_call_valid` performs byte-preserving read-only table, original AST lexeme, receiver,
layout, callable/node-type and external-declaration relationship checks. HIR must
preserve the records and reject invalid or duplicate identities. Concrete MIR
jobs recheck the contract after substitution. A dedicated native bridge may
produce the borrowed view only from that checked intrinsic and its source owner;
an arbitrary integer cast never gains safe-reference authority. Language and C
function types remain distinct, and native support requires its own execution
and corruption tests.

Ordinary Future<T> annotations use structural type kind 16 with exactly one
argument. Actual trait-bound and impl-header roles continue to refer to the
Future trait declaration; they are not interchangeable namespace uses. The
reference inferencer currently lacks a Future/Future structural-unification arm,
so an explicitly annotated local future can fail with identical printed expected
and actual types. The new type arena does not replicate that inconsistency.
Unused Future parameters and annotated local await have named type regressions;
missing and extra arguments are rejected. Explicit unawaited Future escape rules
remain in force. Runtime async wrappers retain their separate language-Future,
C-start-handle and lifecycle-origin obligations; ordinary annotation support
alone does not establish native async or std::file support.


Intrinsic validation uses only checked IDs, direct declaration/field reads and
resolved primitive IDs. It does not intern or substitute new types while checking
an untrusted HIR record. The test poisons each of the six record words and
confirms both rejection and an unchanged type-arena count after restoration.
The ABI-one String bridge does not authorize generic or inherited-handle shapes.

## Future declaration and polling contracts

A `Future` trait declaration's `poll` method must use `&mut self`, including an
abstract method that has no implementation yet. Applying this check only when
an impl is present misses invalid shared, owning and static receiver contracts.
The checker reports the same located 5120 family before a caller can rely on
the malformed signature. Other Future trait methods retain their own declared
receiver contract.

Polling wake evidence remains specific to the resolved context parameter and
the current execution path. Both branches may establish a wake; a wake in an
optional loop, an uncalled lambda or after constructing an evident pending Poll
cannot establish an earlier pending result's wake. This is a static protocol
check, not proof that the native scheduler or Future lifecycle is implemented.

The canonical user-Future await layout remains `is_ready: bool` followed by
`value: T`. A declaration-only reordered Poll implementation passes the frozen
Rust type checker, but the same program with an actual await fails the frozen
compiler's MIR lowering with its explicit Poll-layout diagnostic. The discovery
reports retain those two phases; the SG checker continues to reject the invalid
canonical await layout rather than treating declaration-only acceptance as full
program support. Similarly, the existing rejection of a newly constructed,
shadowing AsyncContext is retained; a same-spelled local is not the caller's
resolved context parameter.

### Native getter inputs versus projected outputs

The native Buffer getter has two separate ownership properties. Its scalar
handle input is neither retained nor consumed by the synchronous C call; this
permits repeated getter calls inside a callable that already owns the Buffer.
Its returned pointer bits are still a projected output. Passing a Buffer by
value through a source projector retains the conservative caller transfer
policy unless the entire output chain reaches a proved noescape observer.
The walk-phase input exception therefore does not add the getter to ordinary
source-summary inference. Existing kind-nine HIR certificate validation and
callee cleanup suppression are unchanged.

Three additional destination sinks are admitted only by real external C
binding identity, fixed arity two, signed-i64 arguments/result and position zero:
Unicode version copy, Unicode provenance copy, and last-error copy. Their
actual C implementations perform bounded synchronous copies and do not retain
the destination or free a Buffer. Destination mutation is not a promise of
read-only memory. Variadic callable flags do not satisfy these contracts.

Actual registered runtime execution currently improves cleanup over four
frozen-reference LLVM programs: the SG programs release once per scope while
the frozen programs leak the first Buffer. Independent C counters retain this
difference explicitly, rather than weakening candidate cleanup assertions.
The current error-copy wrapper also evaluates getters that clear last-error;
the regression fixture restores a known nonempty error after those arguments
and makes no claim to fix the wrapper's error-state ordering.

A separate actual invalid-handle control exposes a pending pointer-status
boundary: runtime_string.c as_str_ptr can return negative statuses, and the
current native opcode 26 copies them into a safe view. Valid registered-handle
integration evidence does not cover that path. A target-only guard proposal
must reject non-positive pointer bits before the consumer; it is not production
implementation until separately reviewed and applied by the native owner.
