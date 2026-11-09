# Represent canonical handle disarm without reviving its moved owner

## Why
Seven real collections source sites extract a reserved owning handle and then
write zero into the original field. Current ownership checking treats extraction
as a root move and rejects the required storage disarm. Simply reinitializing the
root would restore its Drop/use rights and can duplicate ownership.

## What Changes
Add bounded ownership action ten for an exact same-block zero store after a
proved scalar handle extraction. Preserve the original root move and disabled
Drop flag. Restrict it to audited Buffer/Vec/HashMap layouts and single-free
destructors. Upgrade the owned HIR bridge from schema four to five, retain the
current builtin six-word facts and observed/projected parameter certificates,
and reject old schema-four images explicitly.

## Impact
The implementation owner is the SG ownership/HIR/lowering integration in
selfhost/src/borrow.sg, hir.sg and mir_lower.sg. The native backend receives no
new opcode or ABI. This change retains an observable frozen-reference difference:
that compiler still invokes the zero-handle source destructor, while this action
keeps the moved source disabled. Both paths may release the resource once; the
difference is not called a resource leak. Complete M3 and general handle/lifetime
admission remain open.
