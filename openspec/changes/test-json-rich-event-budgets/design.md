# Preserve compatibility while exercising explicit budgets

Build synthetic usage metadata with 600 objects and seven counters each.
The legacy parser must retain its current error-10 node limit. The explicit
parser with 65536 nodes succeeds; 4096-node explicit admission fails with 13;
malformed JSON still fails with 10. Repeat twenty round trips and assert
bounded document and reservation counters return to their initial values.

All fixtures are synthetic and contain no user or provider data. No runtime
or compiler change is needed; native ABI and edition 2026 remain unchanged.
