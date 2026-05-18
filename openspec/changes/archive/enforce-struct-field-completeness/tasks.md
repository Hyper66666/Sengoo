## 1. TypeChecker Field-Set Validation

- [x] 1.1 Refactor `ExprKind::Struct` checking in `compiler/src/typeck/check.rs` to classify provided fields into known, duplicate, and unknown sets in one pass.
- [x] 1.2 Compute missing required fields from declared struct fields minus provided known fields, then sort missing/duplicate/unknown names deterministically.
- [x] 1.3 Emit one actionable `TypeckError::Other` diagnostic for any field-set violations while preserving type unification checks for valid known fields.

## 2. Regression Tests

- [x] 2.1 Unignore `test_struct_construction_missing_field_produces_error` in `compiler/src/tests/struct_codegen_tests.rs` and assert the error mentions the missing field name(s).
- [x] 2.2 Add regression tests for duplicate struct literal fields and unknown struct literal fields.
- [x] 2.3 Add a mixed-issues regression test (missing + duplicate + unknown) and assert deterministic, category-inclusive diagnostics.

## 3. Verification

- [x] 3.1 Run targeted tests for updated/new struct literal validation cases.
- [x] 3.2 Run `cargo test -p sgc` to verify no regressions in compiler behavior.
- [x] 3.3 Review and update any brittle exact-string assertions impacted by deterministic diagnostic formatting.
