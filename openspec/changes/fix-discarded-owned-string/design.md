# Design

`lower_body_to_block_with_return(add_return=false)` lowered a body tail but did
not register its unused owning result. The preceding fix only covered
`HIRStmt::Expr`, so a loop ending in `control_string(...)?;` leaked its final
String on each iteration. A shared complete-expression cleanup helper now
handles both paths, retaining only fresh producer results rather than aliases.

Legacy native wrappers intentionally omit automatic parameter Drop for by-value
String/Buffer-like observation APIs. Four caller layers previously marked all
arguments moved unconditionally: HIR function calls, HIR method calls, resolved
named calls and resolved methods. A caller-side change alone would be overridden
by the later layers. All layers now consult a shared options metadata map.

The map is derived from actual concrete function/expanded method HIR before
lowering. Explicit reference parameters remain borrowed. For legacy value
parameters, field observation and calls to already proven borrowed positions
can establish a borrow. Starting with no inferred borrows and iterating to a
fixed point handles wrappers such as std::hash's str-to-bytes delegation;
unknown callees and recursive cycles cannot create an unproven borrow. Whole
value return, aggregate storage, binding ownership, explicit release and
closure capture prevent inference. Borrow/pointer-containing return types also
prevent new inference, avoiding a freed temporary behind a borrowed result.
JsonDoc's owning parameter contract is not downgraded.

Raw field projection is context-sensitive. Comparisons can observe a native
identity, but returning it, storing it in a local/aggregate or rebuilding an
owner around it prevents borrowing. Native raw-argument observation is allowed
only by the separately audited exact ABI table in
`native_borrow_contracts.rs`: exact declared C symbol, arity, all-i64 signature
and input positions, with no same-named source definition. Unknown extern calls
remain consuming. `docs/runtime-native-borrow-contracts.md` lists the inspected
String, Buffer copy/UTF-8 and hash implementations; no freeing operation or
pointer-returning accessor is included. Negative tests cover native-name
shadowing, signature mismatch, raw return/reconstruction/storage/alias and
retaining unknown extern calls.

Fresh arguments to proven observation calls are retained in an initialized
cleanup place, as with existing temporary borrowed receivers. Named arguments
retain their existing owner binding. Resource transfer through return/aggregate
construction retains consuming behavior. Established receiver compatibility and
explicit release rules remain intact; this change does not redefine all legacy
methods or external ABIs.

Semantic borrow checking runs the same conservative summary over declaration
HIR after registering signatures. It does not infer receiver identity from local
inferred types, and unknown local function values are not mistaken for globals.
The compiler and sgc entrypoints pin that checked map in MIR options before
discarding the type environment. If typed HIR cannot re-establish a checked
borrow, lowering rejects it with `ownership::parameter_contract`; it never
silently tightens ownership after allowing a caller to reuse the argument.
Direct HIR embedders without this checked metadata retain conservative transfer
behavior. A typed-only discovery does not widen the checked policy.

Method policies use the actual receiver-prefixed dispatch identity, not just a
method's short name. The current source-import implementation concatenates
dependency sources and does not independently namespace imported function
names. Duplicate canonical identities therefore disable inferred observation
for that identity; this change does not claim distinct same-named package
functions become language namespaces. Separate receiver methods, ambiguous
function identities and contract mismatches have independent IR/typeck tests.

Verification must include live counters and validity after the observed call:
String.eq leaves both input byte sequences readable, including same-parameter
aliasing; hashing leaves the output Buffer usable; normal/failure exits restore
String and Buffer baselines. Independent IR tests check discarded loop cleanup,
named/fresh/alias arguments, wrapper forwarding and consuming return behavior.
The old compiler and failed native/IR artifacts remain as reproduction evidence.

Follow-up real protocol consumers exposed remaining legacy copy edges:
argument/environment/file copying, NetByteStream reading and Buffer needle
search. The observer table now records only individually inspected noescape
native positions. A separate projected-output summary describes borrowed
Buffer pointers. It is enabled only while an expression's output flows directly
into an established noescape argument; direct pointer return/storage remains a
transfer. Both maps and the exact native observer identity set are pinned from
semantic checking and revalidated in typed MIR. Each argument restores the
previous context before evaluating its siblings. Fresh owners are registered
before later arguments can propagate failure, and remain alive through the sink.

Borrowed receiver identity follows the actual struct dereference dispatch plan.
Without that alignment, a front-end `ptr_index_of_bytes` lookup missed a borrow
proven by the resolved `Buffer_index_of_bytes` callee and suppressed the needle
cleanup. The successful HTTP/SSE regressions now cover this real path. The
native copy regressions exercise a fresh Buffer pointer into String construction
and a failing subsequent argument with exact post-scope counters.

Reference aliases are conservative: creating a local `&value.handle` does not
prove the owner cannot escape through a later unknown call. Such a local alias
prevents inferred observation. A canonical String test requires use-after-move
when a caller tries to reuse an owner after that escape. Reference-to-i64 casts
were separately checked and already rejected as invalid casts; this change does
not describe that rejected hypothesis as an executable source-language defect.

Three historical Drop assertions described observation as consumption: two
`consume(value) { value.len() }` fixtures and `String.eq`. The consuming fixtures
now bind `let owned = value` to make the callee actually acquire and release the
owner; the method observation fixture requires both caller Drops. A separate
method transfer fixture requires zero caller Drops and one callee Drop. The
borrowed-value diagnostic fixture likewise uses a genuinely consuming binding.
These changes preserve transfer/rejection gates while closing the leak that the
old expectations encoded.

The remaining six Strings in the full repository-audit workload were the six
`self.stream(&string_new(), true)` referents. The source parser lowers `&value`
to `HIRExpr::Unary(Ref, value)`, while generated HIR can use `HIRExpr::Ref`.
An initial correction covered only the latter and therefore did not fix source
programs. Both reference lowering paths now materialize fresh owning referents
in initialized cleanup storage before taking the address. Existing named
locals/field aliases do not acquire a second owner, and Drop-body `&self`
reborrows keep their established ABI rule. An IR regression covers direct and
method calls, named references and a failing later argument; the native fixture
exercises twenty rounds of fresh String/Buffer/JsonDoc/Vec plus early failure.
