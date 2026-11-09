# Close discarded-tail and legacy parameter ownership gaps

## Why

Real Agent SDK consumers exposed two independent resource leaks after the
earlier discarded-expression cleanup: a loop's ignored body tail bypassed the
statement cleanup path, while observed legacy String/Buffer arguments were
marked moved at call sites although their established ABI leaves cleanup with
the caller. A minimal native reproduction on compiler `6b40efa1…` measured
five leaked Strings from a five-field loop tail and one String per `eq` call.
SQLite ledger hashing independently exposed Buffer leakage across stdlib
wrappers using the same legacy argument mismatch.

## What changes

- Share ignored-expression cleanup between statements and non-returning body
  tails, including each loop iteration.
- Infer conservative per-parameter observation summaries from available concrete
  HIR definitions, propagating established borrows through wrappers.
- Use the summary consistently in HIR call and resolved-call lowering. Preserve
  caller ownership and temporary owners for proven observed arguments.
- Keep returns, aggregate stores, release operations, unknown/recursive calls
  and unsupported extern owner signatures consuming or rejected. Do not infer
  borrowing from a scalar return type alone.

## Compatibility and ownership

The compiler owner implements this change and its tests. It does not change
edition 2026, runtime ABI, the legacy C handle representation or existing
strict/lossy decoding contracts. Unknown metadata retains existing behavior;
this is not a complete lifetime or escape-analysis system. Borrow-returning
functions are not newly inferred as preserving an owning argument.
