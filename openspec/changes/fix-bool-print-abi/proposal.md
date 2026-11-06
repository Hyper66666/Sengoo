# Preserve ABI 1 for boolean printing

## Problem
The CLI transport probe printed `eof=true` while the actual branch was false.
Generated calls passed an LLVM i1 to the existing i64 runtime print function.
Upper argument bits were unspecified, producing misleading diagnostics.

## Capabilities
Zero-extend booleans to the ABI-1 i64 parameter before stdout and stderr printing,
including boolean fields printed through aggregate Display. Root owns the fix.

## Non-Goals
No runtime ABI change, new print format, or network error policy change.
