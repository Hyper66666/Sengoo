# Design

The type checker accepts one shared/mutable reference layer on the existing
String intrinsic, matching pointer-aware MIR lowering. Returning a view of a
local owned value continues to fail borrow checking.

Automatic Drop continues to pass the value ABI. MIR stores it locally, registers
remaining field cleanup against that storage, and binds body `self` to its
address. Explicit `&self`/`&mut self` in that body reborrow the same address,
preserving prior source patterns. Other function arguments are unchanged.

Fixtures must inspect both pointer helper calls and value destructor signature,
then run O0/O2 native programs to check mutations and exact field release. No
tests use Sencode's real database or invoke a provider.
