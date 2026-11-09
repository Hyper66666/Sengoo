# Drop temporary owned receivers after borrowed chains

## Why
The Agent evaluation SDK exposed a bounded JsonDoc leak in
`make_document()?.root().object_get(...)`. Naming the owner avoids the leak,
but the accepted temporary expression must also release the resource.

## Capabilities
Track nonconsuming owning temporary method receivers in their lexical cleanup
scope. Keep the receiver alive for the complete borrowed chain, then destroy
it once on normal exit or propagation. The compiler owner coordinates lowering
and regression edits; the evaluation agent owns this minimal native fixture.

## Non-Goals
No Result ABI change, lifetime system redesign, eager mid-chain destruction,
runtime leak-hiding cleanup, or claim that all temporary ownership is sound.
