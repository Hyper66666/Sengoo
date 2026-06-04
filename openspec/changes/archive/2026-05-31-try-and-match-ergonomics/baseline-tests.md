# Match baseline (must stay green while extending match / adding `?`)

## Compiler — match IR / phi

- `cargo test -p sengoo-compiler test_match_expression_generates_phi`
- `cargo test -p sengoo-compiler test_match_statement_does_not_generate_phi_void`

## Compiler — pattern helpers

- `cargo test -p sengoo-compiler pattern_match_plan`
- `cargo test -p sengoo-compiler build_match_switch_plan`

## Compiler — parse regression

- `cargo test -p sengoo-compiler regression_suite_tests` (includes `match_with_or_pattern`)

## Compiler — property / stability

- `cargo test -p sengoo-compiler prop_match_pattern_parses`

## Compiler — async + match

- `cargo test -p sengoo-compiler match_with_await_arms`

## Aggregate

- `cargo test -p sengoo-compiler match -- --nocapture`

## Not yet covered (this change)

- Postfix `?` typeck/lowering/codegen
- `try {}` propagation semantics
- Match exhaustiveness / unreachable-arm diagnostics
