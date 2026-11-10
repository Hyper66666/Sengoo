# Connect checked collection intrinsics to the SG compiler

The SG source checker has independent iterator-origin and backing-loan candidate
layers. The compiler still lacks the complete constructor and borrowed-next
runtime bridges needed to compile its own collection use. A source placeholder
or serialized candidate does not implement those bridges.

This child owns independently selected collection providers, their mapping into
actual checked program declarations, concrete constructor/callback lowering and
verified optional-borrow lowering. The origin child owns source-origin/loan
analysis; the native-backend child owns physical callback ABI encoding. Preserve
the existing sixteen-entry native SDK and public collection layout and APIs.
The initial provider is the actual development C collection runtime. Porting
its algorithms to SG and final SDK qualification remain required separately.

The initial implementation checkpoint qualifies six namespace-two C exports
and their actual physical SG declarations. It authenticates caller-selected
source/object inputs and explicit development context without granting any
origin, loan, constructor or safe-reference permission.
