## 1. Baseline And Spec

- [x] 1.1 Validate this change with `openspec validate owned-string-text --strict`.
- [x] 1.2 Record existing `&str` and `Buffer` behavior tests that must remain unchanged.
- [x] 1.3 Confirm public names in `design.md` do not collide with existing syntax or stdlib symbols.

## 2. Compiler And Runtime

- [x] 2.1 Add parser/typeck tests for `String` type references, method calls, move, clone, mutable append, and canonical-type identity.
- [x] 2.2 Implement source-level `String` type resolution and ownership checks.
- [x] 2.3 Implement lowering/codegen/runtime representation for owned string allocation, move, clone, drop, equality, and length.
- [x] 2.4 Ensure allocation failure returns stable status categories rather than panicking.

## 3. Standard Library Surface

- [x] 3.1 Add `std::string` wrappers for construction, append, clear, copy-to-buffer, and buffer-to-string conversion.
- [x] 3.2 Add text examples covering literal borrow, owned construction, append, clone, Unicode byte length, and buffer copy.
- [x] 3.3 Update docs and LSP stdlib symbol/signature discovery for owned-string APIs.

## 4. Verification

- [x] 4.1 Run `cargo fmt --check`.
- [x] 4.2 Run `cargo test -p sengoo-compiler string_ -- --nocapture`.
- [x] 4.3 Run `cargo test -p sgc stdlib_string -- --nocapture`.
- [x] 4.4 Run `cargo test -p sglsp string -- --nocapture`.
- [x] 4.5 Run `sgc check`, `sgc build --force-rebuild`, and `sgc run --force-rebuild` for the new owned-string example.

## Done Definition

- [x] A `String` value can be created from `&str` and from a used `Buffer` range.
- [x] A `String` can be moved, cloned, appended to, compared, inspected via `len()`, and copied back into a `Buffer` via `copy_to_buffer`.
- [x] Existing `&str` literal and managed `Buffer` examples still pass.
- [x] Docs, LSP signatures, and examples describe byte-length semantics.

## Archive Gate

- [x] `openspec validate owned-string-text --strict` passes.
- [x] `openspec validate --all --strict` passes.
- [x] All verification commands above pass or have documented, accepted platform skips.
