# Grammar vs parser (checked 2026-05-31)

| Construct | `design.md` | Parser today | Notes |
|-----------|-------------|--------------|-------|
| `match expr { arms }` | yes | yes | `parse_match_expr` |
| arm `pat \| pat` | yes | yes | `BitOr` between patterns |
| arm guard `if expr` | yes | yes | before `=>` |
| wildcard / literal / ident binding | yes | yes | `parse_pattern` |
| enum / struct patterns | yes | partial | verify per-pattern tests in §3.1 |
| postfix `?` | yes | **yes** (AST `ExprKind::Try`) | lowering still passthrough |
| `try { block }` | yes | **yes** (AST `ExprKind::TryBlock`) | lowering emits plain block for now |
| `?` in invalid positions | reject | not yet | needs diagnostics (§4.1) |

Lexer: `TokenKind::Question` and `TokenKind::TryKw` already exist.
