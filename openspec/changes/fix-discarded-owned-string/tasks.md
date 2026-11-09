## Compiler owner

- [x] Reproduce separate loop-tail String and legacy argument leaks with native
  counters and emitted LLVM IR using the historical compiler.
- [x] Share ignored-body-tail and expression-statement cleanup and add IR coverage.
- [x] Implement conservative concrete-HIR observation summaries and use them in
  expression and resolved-call argument ownership lowering.
- [x] Verify named/fresh/alias/forwarded borrows, usable post-call String/Buffer
  values, consuming return/store and failed propagation with native counters.
- [x] Run existing ownership, try, independent MCP fixtures and cross-module hashing regressions with
  one pinned rebuilt compiler and record full hashes and command results.
- [x] Format source and pass strict OpenSpec validation with exact evidence.
- [x] Cover both source Unary::Ref and generated HIRExpr::Ref when retaining a
  fresh owning referent; verify direct/method/named/early-failure IR paths and
  twenty-round String/Buffer/JsonDoc/Vec native resource baselines.

## Integration owner

- [x] Rebuild Agent kernel, schema and SQLite ledger consumers using the same
  compiler, and verify their repeated resource baselines without manual frees.
