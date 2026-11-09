## Contract and identity

- [x] Define the SG lexer/parser/formatter ownership and separate raw syntax from expansion parity.
- [x] Define structured AST edges, checked IDs, deterministic byte spans and bounded parser failure.
- [x] Validate this change using `openspec validate add-selfhost-syntax --strict` (Windows x64, 2026-11-09).

## Lexer and literals

- [x] Implement reference-vocabulary tokens with preserved whitespace/comments and byte spans (`target/selfhost/m2-grammar/evidence.json`).
- [x] Verify ordinary/raw/byte/char/formatted/triple literals and numeric token behavior against the reference (`target/selfhost/m2-embedded-reviewed/evidence.json`).
- [x] Verify invalid tokens, unterminated literals/comments, caps and resource cleanup (`target/selfhost/m2-lexer/` native evidence).

## Parser

- [x] Implement declaration, statement, type, pattern and Pratt-expression trees with actual child relationships (native `target/selfhost/m2-parser/evidence.json`).
- [x] Verify precedences, associativity, generics, FFI, attributes, loops, conditionals, patterns and async syntax (`target/selfhost/m2-parser/final-evidence.json`, stage0 Windows native scope).
- [x] Verify malformed input/EOF, node limits, invalid maximum values and recursion-depth rejection (same native evidence).
- [x] Verify formatted-string embedded expression trees and source maps (same native evidence).
- [x] Verify 100 repeated parser scopes restore tracked resources (same native evidence).
- [x] Verify declaration/binding frames, where-parameter checks and reviewed reference-negative cases with the current parser artifact (`target/selfhost/m2-parser/strict-evidence.json`; `target/selfhost/m2-embedded-strict/evidence.json`: 816 reference-negative values analyzed, 14 raw accepts owned by preprocessing, zero unexpected supported-positive failures).
- [x] Verify Formatter/Hasher protocol bridges, overrides, idempotence, composed source maps, metadata and exact resources, then compare them with the post-parse reference AST (`target/selfhost/m2-protocols/strict-evidence.json`, `target/selfhost/m2-transforms-audit-final/evidence.json`).

## Complete M2 parity

- [x] Implement and verify declarative macro expansion and bounded recursion/source mappings (`target/selfhost/m2-expand-real/cli-evidence.json`).
- [x] Implement and verify cfg/deprecation processing, derive and Formatter/Hasher synthesis (`target/selfhost/m2-derive/evidence.json`, `target/selfhost/m2-protocols/strict-evidence.json`).
- [x] Implement and verify comment-preserving/idempotent formatting with nontrivia token equivalence (`target/selfhost/m2-format-audit-final/evidence.json`, 19 steps).
- [x] Differentially verify current selfhost sources, stdlib and the frozen supported Windows corpus (`target/selfhost/m2-audit-final/evidence.json`, 310 source cases; `target/selfhost/m2-transforms-audit-final/evidence.json`, 1,748 transform cases).
- [x] Verify extracted Rust literal fixtures and the 17 exact reviewed incomplete prefixes, including completed positive counterparts; preserve initial discrepancy reports (`target/selfhost/m2-embedded-reviewed/evidence.json`: 2,304 extracted, 1,488 reference-accepted, 17 explicit rejections and completed positives, zero unexpected failures).
- [x] Record remaining reference differences explicitly; reviewed EOF prefixes, parser quirks and preprocessing-only negatives remain in the compatibility ledger rather than being hidden.
- [x] Record exact source revision/tool hashes/host/commands/results and run strict change/all validation (`openspec validate --all --strict`: 98 passed, 0 failed).

M2 audit summary, Windows x64 2026-11-10: 81 grammar cases matched the fresh
Rust oracle, 2,304 extracted literals yielded 1,488 accepted complete programs
plus 17 exact reviewed incomplete-prefix rejections, 14 preprocessing-owned
reference-negative cases were rejected by the full SG frontend, 1,748 expanded
and formatted programs matched reference AST rendering, and all component
native/resource gates passed. Semantic checking, HIR/MIR, ownership and the
native backend remain M3+ work.

Final audited parser SHA-256:
`1d0f4247df6f2aefd51a659d651a1b52253f9e004362305e0ee5af6f727b8c63`.
The component report (revision `ca3a163e361619b17e5829f26ac2575ac031e89d`)
has SHA-256 `5527ae359a1b9d5b08e78de9978a1892d7396c09caa547ad5f203de6c1b4d61e`;
its source and tool change lists are empty. The subsequent transformation
report at revision `4aa6e5d` has SHA-256
`1f2c16f0430d4ba47f535d8346ae04528e7fe9513aca5659d8b4e158c8982902`;
the formatter CLI report has SHA-256
`3d0eeb6fcfe38f502f4477db5f61e8d2875c9b460584085b32ffcb0340dbf8aa`.
Both report empty source change lists. These supersede the earlier broad
transform/component runs for the final grammar-audit changes; earlier failures
and discrepancy analyses remain preserved beside the development evidence.
