# SG collections runtime core

## Why

The self-hosted compiler still consumes the reference C opaque registry and RawVec
algorithms. Its specialized scalar SG wrappers already lower to RawVec; the C
legacy VecI64 family is separate. A port must preserve real element ownership,
not classify any eight-byte allocation as Copy.

## What changes

Add an experimental, caller-owned context registry and one RawVec algorithm in SG,
with primitive memory/OS and exact C callback ABI adapters. Qualify the algorithms
against the actual reference C implementation, including failure consumption,
iteration, descriptor callbacks, and bounded context lifecycle. Execute an actual
SG-linked product in two fresh directories.

## Scope

Owner: `/root/selfhost_core`. Capability: `selfhost-collections-runtime-core`.
This child owns runtime algorithms and their independent profile only. Existing
namespace2 provider/source-lowering requirements remain with
`add-selfhost-collection-intrinsics`. No global String/Map/Rc/async registry,
stdlib declaration, existing SDK manifest, or installed runtime is replaced.
Full runtime deployment, compiler self-build, SDK admission, and M8 remain open.
