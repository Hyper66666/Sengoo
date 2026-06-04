## Why

Sengoo already has `Result`, `Option`, and an existing match implementation
baseline in parser/typeck/lowering. The remaining mainstream gap is not "add
match from zero"; it is to make error propagation and pattern matching precise,
exhaustiveness-aware, diagnosable, and consistent enough for stdlib and user
code to rely on.

## Proposal

- Add `?` propagation for `Result<T, E>` and `Option<T>` with pinned lowering
  rules and no implicit error-type conversion in this phase.
- Add `try { ... }` expression blocks so propagation can be scoped to a value
  expression.
- Stabilize the existing `match` baseline with pinned syntax, guards,
  destructuring, binding scope, exhaustiveness, and unreachable-arm diagnostics.
- Keep parser/typeck/lowering/codegen regression tests for current match
  behavior before expanding it.

## Impact

- Updates parser, AST/HIR, type checker, MIR lowering, codegen if required,
  diagnostics, and LSP diagnostic/code-action coverage.
- Existing match programs continue to compile unless they relied on behavior
  now diagnosed as unreachable, non-exhaustive, or type-inconsistent.
- The change is independent from owned `String`, broad stdlib modules, and
  runtime hardening.

## Non-Goals

- No implicit `From`/conversion between error types.
- No exception syntax or stack-unwinding model.
- No regex-like pattern guards beyond boolean `if` guards.
- No pattern macros.
