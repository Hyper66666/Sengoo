# Correct conditional-return branch types

## Problem
The Sencode checkpoint implementation exposed a minimal `if` whose then branch
returns conditionally. An omitted else was inferred as Never, allowing an
enclosing bool/Unit mismatch through type checking and generating an undefined
LLVM Phi operand.

## Capabilities
Correct branch-type joining and choose the value type of paths reaching a MIR
join. Retain existing syntax, edition and runtime ABI.

## Non-Goals
No change to block semicolon syntax, full control-flow inference or runtime ABI.

Owner: root. Authorized by the CLI-driven language improvement implementation.
