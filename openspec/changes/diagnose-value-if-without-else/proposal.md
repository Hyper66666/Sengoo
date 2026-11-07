## Why

The Sencode CLI recall work exposed an unpositioned bool/unit mismatch in a large
function. An `if` without `else` already requires unit, but the generic mismatch
does not identify the offending branch or explain the existing block semantics.

## What Changes

Add a positioned `if-without-else-value` diagnostic with the enclosing function
name and an explicit unit/else repair hint. Preserve type checking, runtime ABI
and all other diagnostic variants. Compiler diagnostics are the sole owner.

## Non-Goals

No change to semicolon, branch inference or return behavior.

## Capabilities

- Added: `unit-branch-diagnostic`.
