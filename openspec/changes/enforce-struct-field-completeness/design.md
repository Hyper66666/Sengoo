## Context

Struct literal checking currently happens in `TypeChecker::check_expr` (`ExprKind::Struct`) and validates duplicate and unknown fields, then type-checks provided field values. It does **not** verify that all declared struct fields are present, so partial literals can pass type checking too far in the pipeline.

There is already an ignored regression test in `compiler/src/tests/struct_codegen_tests.rs` for missing required fields (Requirement 4.5), which confirms this gap is known.

Related constraint: parser supports `..base` syntax in struct literals, but current HIR lowering ignores `base`. This change should improve field-set validation without trying to implement full struct-update semantics.

## Goals / Non-Goals

**Goals:**
- Enforce required-field completeness for struct literals during type checking.
- Keep duplicate/unknown validation in the same pass and make diagnostics deterministic.
- Provide actionable diagnostics that list missing, duplicate, and unknown field names.
- Unignore and extend regression tests for struct literal validation.

**Non-Goals:**
- Implementing full semantics for struct update syntax (`..base`).
- Refactoring the entire diagnostics architecture or introducing span-rich type-check error variants.
- Changing MIR/codegen layout for structs.

## Decisions

1. Centralize struct literal field-set validation in a single pass inside `ExprKind::Struct` checking.
- Build expected field-name set from `struct_field_defs`.
- Scan provided fields once to classify duplicates and unknown fields.
- Compute missing fields as `expected - provided_known` after the scan.

Alternative considered: keep fail-fast checks inline (current style). Rejected because it cannot reliably compute missing fields and yields less consistent diagnostics.

2. Emit one deterministic field-set diagnostic when structural issues exist.
- Collect `missing`, `duplicate`, and `unknown` names.
- Sort names lexicographically before formatting.
- Return one actionable error message containing all non-empty categories.

Alternative considered: fail on first issue only. Rejected because users need multiple edit-compile cycles for simple literal mistakes.

3. Keep this iteration on `TypeckError::Other` for aggregated messages.
- Avoid broad enum/plumbing changes in this scoped fix.
- Preserve compatibility with existing error conversion paths.

Alternative considered: add dedicated `TypeckError` variants for each struct-literal issue. Deferred to a dedicated diagnostics improvement change.

4. Treat `..base` as out of scope for semantic completion in this change.
- Completeness is enforced against explicitly provided fields.
- `base` is not used to satisfy missing-field requirements in this iteration.

Alternative considered: count base-provided fields as present. Rejected because base expression is not semantically lowered today and would create inconsistent behavior.

## Risks / Trade-offs

- [Stricter compile-time behavior] Existing code with partial struct literals will start failing.
  -> Mitigation: explicit error text with field names and targeted regression tests.
- [Diagnostic ordering expectations] Existing tests/scripts that match exact error strings may need updates.
  -> Mitigation: keep stable, sorted field-name lists and assert by key substrings in tests.
- [`..base` user expectation mismatch] Users might expect update syntax to fill missing fields.
  -> Mitigation: document as non-goal and track as a follow-up capability.

## Migration Plan

- No data/runtime migration required.
- Land checker change and tests together in one patch.
- Verify with targeted `cargo test` for struct-related suites and full `cargo test -p sgc` when feasible.
- Rollback path is straightforward: revert the checker validation block and test updates.

## Open Questions

- Should a follow-up change introduce dedicated type-check error variants with source spans for struct literal validation categories?
- Should `..base` become an explicit capability with typed merge semantics and ownership/borrow constraints?
