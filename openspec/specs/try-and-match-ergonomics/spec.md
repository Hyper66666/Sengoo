## Purpose

Pin control-flow ergonomics for error handling and pattern matching: postfix `?`,
`try {}` blocks, stabilized `match` patterns, exhaustiveness checking, and stable
diagnostics across the compiler, `sgc`, and `sglsp`.

## Requirements

### Requirement: Question-mark propagation SHALL have pinned Result and Option lowering

The language SHALL support postfix `?` propagation for `Result<T, E>` and
`Option<T>` in compatible return contexts with no implicit error conversion in
this phase.

#### Scenario: A Result failure is propagated

- **WHEN** a function returning `Result<U, E>` evaluates an expression of type `Result<T, E>` followed by `?`
- **THEN** `Ok(value)` yields `value`
- **AND** `Err(error)` returns `Err(error)` from the nearest compatible function, closure, async function, or `try` block

#### Scenario: An Option none is propagated

- **WHEN** a function returning `Option<U>` evaluates an expression of type `Option<T>` followed by `?`
- **THEN** `Some(value)` yields `value`
- **AND** `None` returns `None` from the nearest compatible function, closure, async function, or `try` block

#### Scenario: Unsupported propagation is rejected

- **WHEN** a program uses `?` across `Option` and `Result`, across mismatched `Result` error types, or inside a plain `main() -> i64`
- **THEN** type checking rejects the expression with a source-range diagnostic
- **AND** the diagnostic names the expected compatible return shape

### Requirement: Try blocks SHALL provide expression-scoped propagation

The language SHALL support `try { ... }` expression blocks that capture `?`
propagation inside the block and evaluate to the block's `Result` or `Option`
value.

#### Scenario: A try block converts failure explicitly

- **WHEN** a plain function evaluates `let result = try { fallible()? + 1 };`
- **THEN** failure returns from the `try` block rather than from the outer function
- **AND** the outer function can inspect or convert the result explicitly

#### Scenario: A try block mixes incompatible propagation

- **WHEN** one `try` block attempts to propagate both `Result` and `Option` without an explicit conversion
- **THEN** type checking rejects the block with a stable diagnostic

### Requirement: Existing match expressions SHALL gain pinned typed pattern semantics

The existing match expression baseline SHALL be stabilized with a documented
grammar, typed patterns, arm result unification, guard semantics, and binding
scope rules.

#### Scenario: A program matches an enum with destructuring

- **WHEN** a program matches an enum value with tuple-like or struct-like variant patterns
- **THEN** each arm can bind fields with type-checked names scoped to that arm expression
- **AND** all arm expressions unify to the match expression type

#### Scenario: A guarded arm can fail its guard

- **WHEN** a match arm uses `if <guard>`
- **THEN** the guard expression must be `bool`
- **AND** that guarded arm does not by itself prove exhaustive coverage for the pattern

#### Scenario: Or-pattern bindings are inconsistent

- **WHEN** an or-pattern alternative binds different names or incompatible binding types
- **THEN** type checking rejects the pattern before lowering

### Requirement: Match exhaustiveness and unreachable-arm diagnostics SHALL be stable

Match checking SHALL detect non-exhaustive matches and unreachable arms for the
accepted pattern forms with stable diagnostic codes and source ranges.

#### Scenario: An enum match is missing a variant

- **WHEN** a match over an enum omits a variant and has no unguarded wildcard or covering arm
- **THEN** type checking rejects the match as non-exhaustive
- **AND** the diagnostic identifies at least one missing variant when practical

#### Scenario: A wildcard hides later arms

- **WHEN** an unguarded wildcard arm appears before a later arm
- **THEN** type checking reports the later arm as unreachable

#### Scenario: A quick fix is safe

- **WHEN** JSON diagnostics or LSP diagnostics report a simple non-exhaustive match
- **THEN** the diagnostic may include a quick-fix action to insert a placeholder wildcard arm
- **AND** the quick fix is omitted when insertion would be ambiguous or unsafe
