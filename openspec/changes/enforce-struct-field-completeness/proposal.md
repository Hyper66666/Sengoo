## Why

Struct literals currently do not enforce required field completeness at type-check time, which allows invalid programs to pass too far in the pipeline and weakens language correctness guarantees. This should be fixed now because the gap is already tracked in ignored tests and affects developer trust in diagnostics.

## What Changes

- Add mandatory-field completeness checks for struct literal expressions during type checking.
- Add duplicate-field and unknown-field validation in the same struct literal validation pass.
- Emit actionable diagnostics listing missing, duplicate, and unknown field names with source location context.
- Unignore and update existing regression tests for missing struct fields.
- Add focused regression coverage for missing/duplicate/unknown struct literal field cases.

## Capabilities

### New Capabilities
- `struct-literal-validation`: Validate struct literal field sets for completeness and correctness (required fields present, no duplicates, no unknowns), with deterministic diagnostics.

### Modified Capabilities
- _None._

## Impact

- Affected code: `compiler/src/typeck/check.rs` (struct literal checks), diagnostics plumbing, and related tests under `compiler/src/tests/`.
- Affected behavior: invalid struct literals now fail during type checking instead of proceeding.
- Tooling impact: improved `sgc check/build/run` error quality for struct initialization mistakes.
