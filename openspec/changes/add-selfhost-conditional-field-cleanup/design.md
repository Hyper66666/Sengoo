# Conditional field cleanup: representation and execution design

This is a proposed implementation contract, not implemented behavior. All new
claims below require the tasks and independent gates in this directory.

## 1. Current mechanism and the precise gap

`borrow.sg` stores ownership paths as `[root binding, parent path, field binding,
resolved type]`; action rows are `[AST node, root binding, kind, scope,
conditional]`, with a parallel `action_paths` table. Negative roots identify
owned temporaries. Action 1 initializes, 2 moves, 3 cleans up, 6 cleans up before
replacement, and 7 cleans up on propagation. Actions 8/9 carry observer/projector
contracts; action 10 is the separately constrained zero disarm.

`sh_b_drop_path` recursively enumerates surviving fields for a partially moved
struct. A branch merge can correctly emit a field action with conditional 1.
`sh_b_legacy_projection` intentionally moves the nearest reserved owning
handle ancestor when its scalar projection is consumed. Preserve both facts.

`mir_lower.sg` currently skips field initialization/move flag updates, emits
opcode 25 only for local-root updates, and uses opcode 21 for typed cleanup.
Its projected cleanup carries local ID -1. It rejects a conditional projected
cleanup because native code cannot select a flag for that place.

`mir.sg` currently has schema 1, ten-word locals, twelve-word instructions,
three-word Drop-glue records and no serialized ownership-path table. The HIR
bridge is independently versioned (the current joint candidate uses HIR7).
There is no separate MIR wire codec in the files examined; the existing
roundtrip gates serialize HIR, then compare regenerated MIR.

`mir_validate.sg` validates opcode 21's mutable pointer/type/local association
and opcode 25's local and Boolean value. Its function named
`sh_mir_validate_ownership` checks arena/CFG ownership, not a proof of linear
resource behavior over all paths. It must not be described as such.

`native.sg` allocates an initialized stack slot for each owning local, zeros it
in the prologue, and checks it for conditional opcode 21. Recursive typed Drop
visits remaining fields in reverse order. `sh_native_prepare` recognizes only
opcode 21 when reserving the 1024-byte recursive-drop scratch area. New path
cleanup must update that discovery as well as frame-size and unwind records.

## 2. Initial admitted domain

The first source gate covers concrete, statically laid-out named structs and
nested named structs with no custom destructor on an ancestor through which a
partial move occurs. Generic instantiations use their exact resolved field
types and ordinal mapping. Roots may be owned parameters, locals or registered
owned temporaries. A custom-`Drop` value can be an indivisible owning leaf.

All supported exit forms for an admitted function must work: block exit,
return, existing error/Option propagation, and branch-specific early exits.
Loop and replacement admission is a separate checkpoint below. Until that
checkpoint passes, a function combining those operations with the new field
state must fail explicitly; compilation may not silently omit its cleanup.

Existing whole-value handling of tuples, fixed arrays, enums and classes is
retained. Their use as indivisible leaves requires already valid resolved Drop
glue. New *partial* moves through tuple/index/tag/inheritance projections need
their own evidence before admission. References and raw pointers do not create
ownership paths in the pointee. Genuine tagged payload partial moves and
partial moves through a custom destructor retain 6107/6111 or their existing
located diagnostic family. No standard-library name grants admission.

## 3. Preferred MIR representation

Use MIR schema 2 for the extension; keep `target=1`, `runtime_abi=1` and
`pointer_width=64`. Explicit schema-1 handling can preserve existing producers,
but must not reinterpret their tables or auxiliary words. Unsupported versions
fail before reading extension fields. HIR7 can remain unchanged if all needed
information is derived from its current validated paths/actions. Any new HIR
fact or changed action meaning instead requires a separate HIR version change.

Add an owned `drop_paths` arena with candidate four-word records:

| Word | Meaning |
| --- | --- |
| 0 | Root MIR local ID, also determining the function and lexical scope |
| 1 | Parent path ID; -1 only for a root, otherwise an earlier same-root record |
| 2 | Concrete field ordinal; -1 only for a root |
| 3 | Exact resolved MIR type at this path |

Lowering maps source binding IDs and generic field bindings to real MIR locals
and layout ordinals. Do not serialize AST field-binding IDs as physical offsets.
The table contains a deterministic, complete tree of owning subobjects for each
participating root, not only paths happened to be visited in the first branch.
Validate root uniqueness, unique parent/ordinal pairs, type equality, complete
owned-field coverage, acyclicity, concrete layouts and checked global budgets.

Use a *terminal cleanup partition*: a structural aggregate without its own
destructor is a tree node; each indivisible owning leaf has one runtime live
flag. A custom destructor, including its existing automatic contained-field
cleanup, is one leaf and cannot also have independently scheduled descendants.
This avoids both double destruction and contradictory root/child flags.

Two candidate operations are sufficient for state storage and cleanup:

- `PathInitialized(path, Boolean)`: updates every terminal flag in that
  subtree, including a single leaf. It does not write object bytes or confer
  source-level permission to read a moved value.
- `TypedDropPath(path)`: visits the subtree's terminal leaves in reverse field
  order, tests each live flag, disarms that leaf, and invokes existing typed
  Drop on its exact address/type. It is a unit operation with no supplied raw
  pointer; native code derives the address from the validated root and path.

Choose unused opcode numbers only after inventory; 29/30 are candidates after
the currently used 28, not assigned by this draft. Specify all reserved words,
operand counts and flags exactly. Root schema-1 operations retain their old
meaning. A path-tracked local must not simultaneously use the old root flag or
direct root Drop as a second cleanup mechanism. Unrelated existing typed Drop,
including approved callbacks, needs an explicit compatibility route.

Alternative A is lowering each flag to ordinary Boolean locals and CFG branches
around opcode 21. This can avoid a schema bump but loses the explicit relation
between a flag and its ownership path and creates another implicit convention.
Alternative B overloads opcode 21's `aux` with a field flag; it silently changes
a documented local ID and still lacks subtree updates. Prefer the explicit
versioned representation. Do not simply use the aggregate's root flag for all
fields: one field move would suppress independent cleanup or double-release it.

## 4. Lowering invariants and state transitions

Retain a deterministic source-path-to-MIR-path map for each concrete callable
instance. Before publishing MIR, every relevant checked action must have one
accounted-for transition or its existing explicit proof of elision. In
particular, the old field-action skip cannot remain in the new route.

| Source event | Required ordering/effect |
| --- | --- |
| Parameter receipt or successful local initialization | Store/copy the complete value, then set its subtree live |
| Owning field or whole-value move | Materialize the value/argument/return first, then clear only the moved subtree at the ownership transfer point |
| Observed/projected parameter certificate | Preserve its existing no-ownership contract; do not invent a cleanup flag for borrowed authority |
| Scope/early-exit/propagation cleanup | Run `TypedDropPath` for each required source action; never drop a moved sibling |
| Canonical action-10 disarm | Emit its existing zero store; do not set any root or child ownership flag |
| Typed overwrite | Evaluate RHS safely, drop live old target leaves, store replacement, then set replacement subtree live |
| Whole aggregate reinitialization | Clean surviving old leaves, perform complete store, then restore all terminal flags under the root |

RHS temporaries need independent flags until the store/transfer commits. A
failed RHS or a source-level early return must not clear the old target or mark
the new target initialized. Self/overlapping assignment must preserve the
current semantic rejection or an explicitly proven transfer sequence, rather
than copying moved bytes back and resetting flags. Reinitializing one field
does not restore moved siblings. Overwriting a parent must invalidate stale
descendant states as part of the same subtree update.

Whole-value cleanup of a root that can be partially moved must use the same
terminal flags even on paths where the source checker emitted a root action.
Calling recursive root Drop plus separate leaf Drop would double-release.
For an atomic custom destructor, keep the existing resolved-glue checks,
including rejection of explicit/transitive nested field destruction without a
verified field-inactivation mask. This project does not invent such a mask.

## 5. Branches, loops and validation limits

Runtime flags represent the path actually executed. Branch joins do not set a
flag merely because either predecessor had it set; each predecessor carries
its own stores into the shared stack slot. Paths created while analyzing only
one branch must still exist before entry and start inactive.

Loop-local state is reset at its real declaration each iteration. Outer-owned
state persists across the backedge. Cover zero iterations, both branch orders,
repeated move/reinitialize, nested loops, break and continue. Current borrow
analysis replays a loop body and trims duplicate actions; assess whether its
bounded join represents every new path and state. If it does not, add a bounded
worklist/fixed-point analysis as a separately reviewed semantic change. Do not
assume two passes prove arbitrary new loop behavior, nor let runtime flags
make a source use-after-move admissible.

Pure MIR validation must check path geometry, operation scope/type, forbidden
mixed flag models, pending glue, concrete target storage, bounded lengths and
all reserved words without mutating original arenas. The source lowering gate
must independently compare every emitted update/drop with the original
validated HIR action and inspect stores/transfers and control edges.

These are distinct claims: a coherent, well-typed edit that changes which of
two same-typed siblings a program drops may describe a *different* MIR program.
A structural validator alone cannot prove it equals the original source.
Mutation tests must distinguish malformed MIR (validator rejection) from
source-to-MIR mapping drift (lowering/evidence comparison rejection). If the
implementation promises that native admission itself checks original source
equivalence, it must carry and validate a corresponding owned certificate;
that is additional representation work, not an undocumented assumption.

## 6. Native storage, ABI and bounded execution

Flags live only in the generated function's frame. They do not change aggregate
field offsets/size/alignment, generic ABI layout, argument copies, sret memory,
public SDK headers or runtime handles. Independent C callers must continue to
observe the same physical representation. Address calculation uses checked
target-layout field offsets at each parent; reject wrong parent type, overflow
and unsupported shapes before emitting bytes.

Allocate/zero all terminal flag slots once in the actual prologue and include
them in overflow checks, stack probing, frame alignment and unwind allocation.
Update drop-scratch discovery for the new instruction. Preserve saved parent
addresses across nested calls, Windows shadow space, nonvolatile registers,
call alignment and hidden result pointers. A skipped drop must skip its field
load and destructor call; bytes belonging to a moved field need not be valid.
The existing short/probed prologue allocation-end offsets (11/22) must be
checked against emitted bytes when flags change the frame category; do not
retain an old unwind offset merely because the function's source is unchanged.

Disarming immediately before the leaf destructor avoids reentering the same
cleanup obligation. Supported destructors must return normally under the
existing contract. Existing unwind tables describe stack recovery, not language
exception cleanup; this draft does not establish cleanup on SEH, longjmp,
process abort, panic unwinding or arbitrary FFI reentry.

Initially prefer ordinary independently checked flag slots over bit packing.
Bound terminal count, traversal depth, total frame bytes and emitted instruction
count; budget exhaustion is a located failure preserving the prior output.
Do not introduce runtime-sized field-flag allocation or unbounded loop codegen.

## 7. Standard String resource oracle and separate semantic decision

`tools/stdlib/runtime_string.c:462` clones bytes into a new registered string and
does not free the input. Under the preserved legacy move rule, the selected
original in `Option<String>::unwrap_or` becomes unowned before that clone.
General field cleanup must not silently destroy a value marked moved merely
to make this one method leak-free. The anticipated mismatch must remain visible.

Use actual canonical source bytes, the current captured C implementation and
the real `String` Drop. Interpose delegating C wrappers only for counters/fault
injection; retain and hash the original source, headers, exports and commands.
Test Some/None with nonempty Unicode strings, distinct fallback allocations,
zero placeholders, selected zero, and nonselected live payloads. Record original
and clone handles, byte equality, independent storage, clone calls, successful
releases, destructor attempts and live-handle/allocation-byte deltas.

Some with two live inputs should ultimately release the unused fallback and
the original selected input exactly once, and later release the independent
result. None should likewise release any owned unused placeholder and original
fallback, then the result. Zero handles must not create a live allocation.
Repeat complete scopes and verify exact baseline recovery. Inject clone failure
through the actual registered API boundary and state explicitly how its negative
status is represented by the current String source; do not invent success or
change the API during this gate. Selected-invalid tests need an explicit unsafe
fixture classification, not an allegation against valid source callers.

If retained move semantics leave the selected original live, keep this gate
failed and report the leak. A separate source-bound observer rule or canonical
source rewrite must establish its own provenance, legal borrowing/transfer,
Drop and error contract. Compare the frozen reference independently and record
differences; reference agreement is not a substitute for exact resource counts.

## 8. Integration and source integrity

All phase reports must capture the current helper/import closure, C/Rust
development runtime and headers, canonical specification inputs, compiler,
formatter/linker tools and actual executable artifacts. Recheck originals and
retained inputs after execution; exclude mutable task/checkpoint summaries
explicitly rather than weakening guards. Bind named commands, exit codes,
stdout/stderr hashes and nested report pins. Reuse relevant existing gates;
new names in `tasks.md` are proposed, not existing commands.

Preserve deterministic lowering order and repeated object bytes, HIR clone/wire
equivalence, diagnostic physical paths/spans, prior-output preservation and
first-call/repeated compiler-resource recovery. Compare current ordinary MIR,
root/structural Drop, callbacks, handle disarm, projection, collection effects
and module/sourceflow regressions after the bounded path gate passes.
