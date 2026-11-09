# Canonical handle disarm and HIR schema 5

## Bounded producer authority
This extends existing reserved legacy owning-handle extraction semantics. It
does not redefine the global legacy-owner projection rule. Authorization is
limited to actual struct declarations for:
- Buffer: handle:i64 only.
- Vec<T>: handle:i64, marker:i64.
- HashMap<K,V>: handle:i64, key_marker:i64, value_marker:i64.

Fields must be direct declarations of that exact ADT owner, have these precise
ordinals/names and resolved signed-i64 types. Enum/class/inherited, extra fields,
refs, owned fields and unresolved generic payload fields are rejected. Every
matching Drop impl must be an SG unit(&mut matchingADT) callable with its
matching parameter type, containing exactly one call to the corresponding real
external fixed-arity i64->i64 reserved buffer_free/raw_vec_free/raw_hashmap_free
declaration, directly on that receiver's handle. Other destructor effects do
not authorize this disarm. Marker values rely on the existing reserved legacy
ABI contract; arbitrary same-sized application resource encodings do not gain
general ownership admission.

## Exact transfer / action contract
Scalar handle extraction retains its existing root move kind 2. A later
same-block direct assignment of that same field to the exact zero literal may
publish kind 10:
  [assignmentAST, originalRootBinding, 10, lexicalScope, 0]
with a field action_path. Its source extraction must be a prior direct let
initializer among the same AST block's ordered children. The most recent root
ownership event must be that exact scalar-field move; conditional moved state,
other root moves, intervening init/drop/disarm, wrong field, nonzero write,
cross-block extraction and ordinary consuming calls cannot authorize it.

The zero store leaves the root moved. It emits no kind 1 and does not restore
root use or Drop. The owner assembled from the detached scalar identity is
responsible for release. An actual MIR check requires one source init-flag
set at declaration, one source flag clear at extraction, zero source flag
instructions at the zero-write site, exactly one zero store there, and no
source drop instructions. The C transfer test additionally requires zero
source destructor invocations and exactly one destination destructor/release
per allocation.

## HIR bridge version and pure validation
Action kind 10 is not silently added to schema 4. Production uses HIR schema 5
in constructors, encoder and decoder. Schema 4 images reject 5502. All current
builtin_calls six-word facts and action kinds 8/9 keep their table widths and
meanings. There are no new ShChecked fields, native opcodes or pointer casts.
The pure kind-10 validator checks exact structure, destructor/body/prototype,
zero assignment, direct AST block sibling relationship, typed field/root and
prior move trace. It does not run semantic analysis or allocate type IDs.
Poison tests change prior move, action node/path/conditional and Drop receiver
mutability. They also jointly replace the field path and producer/target field
resolutions with another ADT's handle binding, or change root path, field path
and producer/target node types. The field must belong to the actual root ADT;
its path/node types must equal the resolved field type, and its root path type
must equal the owning binding type. Each rejected control preserves the arena
count, and restoring it restores successful validation. Roundtrip/re-encoding is exact and decoded HIR lowers to valid MIR.

Lowering requires no new machine instruction for action 10: the existing
typed assignment writes zero. Existing root move kind 2 already clears the
source initialization flag. The new action never requests flag restoration or
a source Drop. The lowering source documents this intent and all current HIR schema/version
fixtures use schema five; earlier four images explicitly fail 5502.

## Actual execution evidence and differences
The production HIR/native gate runs twenty resource-exact HIR rounds and compiles a
complete SG native emitter from its pinned target/production import snapshot.
One hundred transfers then execute against actual runtime.c/runtime_string.c.
The real C buffer-free entry is compiled under an instrumentation alias; a
delegating wrapper preserves its behavior while counting any source destructor
call. Destination cleanup calls the actual release function. All counters and
String/Buffer/opaque live counts are checked after every scope.

A separate frozen-reference LLVM control compiles/links the identical source
but fails the stronger source-destructor-count assertion: first scope created1,
freed1, source_drops1, live0. Its resources are released exactly once; it still
invokes the original source free(0) destructor after zeroing. This is an
observable destruction-call-policy difference, NOT a resource leak and NOT
a claim that the two complete ownership semantics are equivalent.

The approved implementation upgrades production HIR to schema five. Historical
target evidence remains separate; production source-bound gates must bind the
final implementation and show the actual diagnostic boundary independently.
General handle integers, arbitrary field reinitialization, complete SDK
admission, source runtime self-hosting and full M3 remain open.

The parent semantics change continues to own the general typed HIR contract.
This child owns only canonical disarm and its versioned representation. The
builtin-call table remains six words, deferred equalities remain four words,
and observed/projected parameter certificates retain action kinds eight/nine.

The durable runner excludes only this child's post-run tasks evidence ledger
from producer guards, preventing a report-SHA self reference. Proposal, design
and normative delta remain bound inputs. Cargo configuration bytes are read
once for both their fingerprint and TOML parsing, including absent-file
records. Immutable SG/stdlib inputs and selected tool hashes are checked again
after execution.
