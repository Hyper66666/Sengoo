# Design investigation

`lower_try_expr` extracts the success `value` into a LocalKind::Temp. A following
nonconsuming method call uses this owned temporary without registering its
Drop binding. A named `let` registers a binding in lower_let_stmt, which explains
why the equivalent named code releases JsonDoc correctly.

The implementation registers owning temporary receivers in method-expression lowering
when receiver ownership is retained. Do not register borrowed/loaded copies as
owners. Consuming methods must transfer ownership and cannot leave a second live
Drop binding. The temporary owner must remain live until all chained borrowed
uses finish and participate in early-propagation cleanup. Lexical scope cleanup
is the initial conservative boundary; this does not claim complete NLL semantics.

The native regression uses the real bounded JSON document counters and checks
both normal return and a later failing `?`. The read must return 42 before
cleanup, preventing an eager destruction fix from masquerading as success.
A separate named-owner control verifies the runtime counter baseline.

Registration alone was insufficient: flagged cleanup discovers initialization
through Store instructions, while an SSA Call/Extract temporary has no Store.
`retain_temporary_receiver` materializes a hidden owning User local, inserts its
initializing Store, marks the source temporary moved and registers its Drop.
This preserves the existing flag machinery rather than claiming that an
uninitialized temporary is always live. Borrowed/loaded field aliases are not
promoted; genuinely consuming methods transfer ownership to the callee.

A second real consumer exposed unused owning producers such as
`client.next_notification()?;`. Expression-statement lowering now creates a
statement cleanup scope and retains a fresh owning result through the same
hidden-owner helper. The statement boundary destroys both the unused result
and any owning receiver needed only by that expression, including each loop
iteration. Bare variables and field aliases do not acquire a second owner.

The approved changes cover method-expression/call lowering, the necessary Drop
helper, expression-statement dispatch and regression tests. The SDK's named-local
pattern is independent from the correction and is not its acceptance evidence.

The original TUI scalar-partition regression exposed a second cleanup boundary:
`consume(if flag { make_string()?.as_str() } else { "literal" })` returned a
borrowed pointer from a scoped branch after destroying its anonymous String.
The UI activity row consequently contained freed heap bytes. A native counter
probe detects the early destruction before dereferencing the pointer, so the
regression is deterministic rather than depending on allocator reuse.

Anonymous retained producers now carry an explicit lowering identity. If a
scoped body yields a borrowed pointer/reference (including aggregate fields),
its live anonymous producer bindings move to the enclosing cleanup scope.
Lexical owners are not promoted. Existing Store-based flags still distinguish
the selected branch from uninitialized alternatives. Normal full-expression
cleanup and subsequent argument `?` failures release exactly the constructed
owners. Nested scopes propagate the same obligation until the outer consumer.
This conservative extension does not implement NLL, lift named locals or permit
a borrowed temporary to escape a function; the borrow checker still rejects
that escape.
