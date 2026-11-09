## Contract and identity

- [x] Define the SG lexer/parser/formatter ownership and separate raw syntax from expansion parity.
- [x] Define structured AST edges, checked IDs, deterministic byte spans and bounded parser failure.
- [x] Validate this change using `openspec validate add-selfhost-syntax --strict` (Windows x64, 2026-11-09).

## Lexer and literals

- [ ] Implement reference-vocabulary tokens with preserved whitespace/comments and byte spans.
- [ ] Verify ordinary/raw/byte/char/formatted/triple literals and numeric token behavior against the reference.
- [ ] Verify invalid tokens, unterminated literals/comments, caps and resource cleanup.

## Parser

- [x] Implement declaration, statement, type, pattern and Pratt-expression trees with actual child relationships (native `target/selfhost/m2-parser/evidence.json`).
- [x] Verify precedences, associativity, generics, FFI, attributes, loops, conditionals, patterns and async syntax (`target/selfhost/m2-parser/final-evidence.json`, stage0 Windows native scope).
- [x] Verify malformed input/EOF, node limits, invalid maximum values and recursion-depth rejection (same native evidence).
- [x] Verify formatted-string embedded expression trees and source maps (same native evidence).
- [x] Verify 100 repeated parser scopes restore tracked resources (same native evidence).
- [x] Verify declaration/binding frames, where-parameter checks and reviewed reference-negative cases with the current parser artifact (`target/selfhost/m2-parser/strict-evidence.json`; `target/selfhost/m2-embedded-strict/evidence.json`: 816 reference-negative values analyzed, 14 raw accepts owned by preprocessing, zero unexpected supported-positive failures).
- [ ] Verify Formatter/Hasher protocol bridges, overrides, idempotence, composed source maps, metadata and exact resources, then compare them with the post-parse reference AST.

## Complete M2 parity

- [ ] Implement and verify declarative macro expansion and bounded recursion/source mappings.
- [ ] Implement and verify cfg/deprecation processing, derive and Formatter/Hasher synthesis.
- [ ] Implement and verify comment-preserving/idempotent formatting with nontrivia token equivalence.
- [ ] Differentially verify current selfhost sources, stdlib and the frozen supported Windows corpus.
- [x] Verify extracted Rust literal fixtures and the 17 exact reviewed incomplete prefixes, including completed positive counterparts; preserve initial discrepancy reports (`target/selfhost/m2-embedded-reviewed/evidence.json`: 2,304 extracted, 1,488 reference-accepted, 17 explicit rejections and completed positives, zero unexpected failures).
- [ ] Record remaining reference differences explicitly; do not close M2 while any supported syntax/transform gap remains.
- [ ] Record exact source revision/tool hashes/host/commands/results and run strict change/all validation.
