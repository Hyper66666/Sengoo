## Scope

This change covers P0 control-flow ergonomics only: `?`, `try {}`, and match
stabilization. The implementation should first lock current match behavior with
regression tests, then extend it.

## Grammar

The accepted grammar shape is:

```text
postfix_expr ::= primary_expr ("." ident call_args? | call_args | "?")*

try_expr ::= "try" block

match_expr ::= "match" expr "{" match_arm+ "}"
match_arm  ::= pattern guard? "=>" expr ","?
guard      ::= "if" expr

pattern    ::= "_"
             | ident
             | literal
             | enum_path
             | enum_path "(" pattern_list? ")"
             | struct_path "{" field_pattern_list? "}"
             | pattern "|" pattern

pattern_list       ::= pattern ("," pattern)* ","?
field_pattern_list ::= ident (":" pattern)? ("," ident (":" pattern)?)* ","?
```

Implementation agents must update this design before accepting a different
surface.

## Propagation Rules

`expr?` is accepted only inside a function, closure, async function, or
`try {}` block whose immediate result type is compatible:

- `Result<T, E>` in a `Result<U, E>` context unwraps `Ok(value)` and returns
  `Err(error)` from the surrounding function/block on failure.
- `Option<T>` in an `Option<U>` context unwraps `Some(value)` and returns
  `None` from the surrounding function/block on failure.
- Cross-propagation between `Option` and `Result` is rejected.
- Different `Result` error types are rejected unless a future OpenSpec adds an
  explicit conversion trait or function.

`main` may use `?` only if its return type is an accepted `Result` or `Option`
shape. A plain `main() -> i64` cannot use `?` without wrapping the propagation
inside `try {}` and converting the result explicitly.

## Match Semantics

Guards are supported with `if <bool expr>`. A guarded arm does not prove
exhaustiveness for the guarded pattern; an unguarded covering arm or wildcard is
required when guards could fail.

Pattern bindings are scoped to the arm expression only. Or-pattern alternatives
must bind the same names with compatible types.

## Done Definition

This lane is done when `?`, `try {}`, and match can be explained by the grammar
above, and parser/typeck/lowering diagnostics reject unsupported forms with
stable source ranges.
