# HIR6 complete standard graph body coverage

This focused checkpoint owns `hir.sg`, `tests/hir_body_coverage.sg`,
`verify_hir_body_coverage.py` and this capability's specification/design notes.
The collection source-ownership module, old borrow checker, semantic sources,
HIR origin codec, MIR and native backend are not modified.

The actual semantic checker deliberately returns before visiting every method
body whose binding owner is a trait (kind7). The complete standard collections
source has ten such default bodies and four unvisited calls to option_none_i64.
Their raw node type, resolution, call type and scope values are all -1; the old
HIR validator resolved the missing type to ErrorType18 and rejected5503.
The earlier mini-library HIR6 gate did not expose this complete-library boundary.

Keep schema6 and the entire raw input. Derive a transient coverage map from an
actual trait AST27, actual owned Fn20 declaration and its unique body40 subtree.
Require declaration/binding relationships, unique incoming syntax edges and
unchanged semantic unvisited sentinels throughout that subtree. All AST/type/
reference bounds still run first. Calls in that precisely derived region are
source syntax that remains deferred; an ordinary unknown call cannot use it.
No mutable saved trust flag or replacement clean callable fact is serialized.

`sh_hir_function_body_state(ast,checked,function)` returns -1 for malformed or
uncovered input,0 for an actual bodyless declaration,1 for a checked ordinary
body,2 for a genuine deferred trait default. Consumers must already validate
HIR/module/actual inputs and admit only state1 as executable body coverage.
State0 follows the separately checked external declaration path, never a
zero-body implementation. State2 is explicitly5008 until a later fresh concrete
body typing/rebinding stage. This query is not a monomorphic job certificate.

The complete graph also has two typed mapper(item.value) calls whose callee Fn
node was normalized in semantic completion but whose saved call Fn still holds
an inference variable return. Fix their identity test separately by bounded,
read-only recursive substitution resolution. Kind, data/ABI, flags, nominal,
arity and all nested arguments must match; this is not general compatibility or
an unresolved-type wildcard. Both depth and total comparison work are bounded.
Raw signatures and type arguments remain unchanged in encode/decode/clone.

Focused tests separate ordinary unresolved-call poisoning from deferred source
coverage. They retain missing/changed callable metadata, forged owner/body
association, shared/cross-attached subtree, bad raw bounds, signature ABI/type
poisons, old wire5 rejection and module provenance/ownership mismatch controls.
The actual full graph is lifted with independent scoped ownership and complete
module witnesses within the unchanged four-million-word budget. First-call
String/Buffer/opaque/FileSnapshot handle and byte counts must return exactly.
Passing a container/roundtrip does not close collection runtime, reference or
full standard-library ownership/NLL support.
