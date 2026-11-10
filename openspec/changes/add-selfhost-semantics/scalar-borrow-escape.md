# Primitive reads and borrow escape

Owner: this focused semantic checkpoint owns borrow.sg's sh_b_escape path
branch, focused scalar escape tests/runner/probe and this unique specification.
Collection effect/Some authority remains owned by its independent child; no
runtime, provider, HIR or MIR implementation is changed here.

The checker already guards escape checks for calls/unary/binary results with
the resulting value type. Its subsequent path>=0 branch instead inspects every
loan associated with the root binding, even when the final field/dereference is
an i64 read. Thus Option<&Owner> item; item.value.value is incorrectly rejected
6105 as though the Option's backing reference were returned. A local reference
alias followed by a bool/int/float/char field read has the same issue.

The minimal fix is confined to that path branch: after obtaining the checked
resolved node type, kinds1,2,3,4 do not perform this loan-holder escape check.
Earlier walking/use/move/Drop/legacy-handle rules still execute. Do not return
from the entire sh_b_escape function, remove diagnostic records, suppress Drop,
or use !carriesBorrow: rawptr7 and other old conservative aggregate/generic
boundaries must remain in the original path logic. Width is irrelevant.

A baseline target checker and one patched target checker run exactly the same
source fixture set. Primitive positive cases must change from6105 to0; Ref,
reference ADT, rawptr, aggregate, array, tuple, generic move, borrowed Owner move,
invalidated backing and legacy handle-transfer cases keep their rejection.
The selected runtime fixture is ordinary source Option<&Owner>, not an SDK or
provider intrinsic. Source/NLL/provider/Some qualification for real collection
pointers remains separate, including unguarded/false-Some rejection.

Production promotion requires the exact old borrow SHA990fb05324a641ecd2ebee7cd696d3224e55f8992df5bdd4118718969bdff45c, a proved target delta,
and focused first-call resources. Follow with the old ownership corpus, raw
MIR/owned HIR6, projection and disarm action/cleanup regression checks. Captured
source snapshots and pinned tools bind the actual runs. No M0-M8 completion or
full library/native self-compilation claim follows from this correction.
