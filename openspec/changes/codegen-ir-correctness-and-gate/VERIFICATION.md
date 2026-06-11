# Verification

Local host: Windows, `clang` 19.1.7.

## Passed

- `openspec validate codegen-ir-correctness-and-gate --strict`
- `openspec validate --all --strict`
- `cargo fmt --all --check`
- `cargo clippy -p sgc -p sengoo-compiler --all-targets -- -D warnings`
- `cargo test -p sengoo-compiler --lib -- --test-threads=1`
- `cargo test -p sengoo-compiler match_pattern_parser -- --nocapture`
- `cargo test -p sengoo-compiler linux_sysv_async_three_field_results_use_sret_for_declaration_and_call -- --nocapture`
- `cargo test -p sgc parse_clang_major_version -- --nocapture`
- `cargo test -p sgc validate_clang_major_version -- --nocapture`
- `cargo test -p sgc core_conformance_examples_compile_link_and_run -- --nocapture --test-threads=1`
- `cargo test -p sgc -- --test-threads=1`

## Pending External Evidence

- Linux CI on the pinned core-conformance toolchain after push.
