# Implementation and verification

- [x] 1. Resolve lexical bindings and references in versioned AST workspace snapshots.
- [x] 2. Use the shared resolver for definitions, references, prepareRename and conflict-safe rename.
- [x] 3. Reject ambiguous, invalid, readonly and stale edit requests with no name-search fallback (190 main LSP tests + 1 evidence verifier; queued incremental changes and close/reopen included).
- [x] 4. Generate formatted bin/lib scaffolds with runnable tests and verify the fresh project loop (installed Windows toolchain, bin/lib new and init complete locked loops passed outside the checkout).
- [x] 5. Implement text/JSON doctor diagnostics and native toolchain probes (Windows source-development probe passed).
- [x] 6. Document actual editor running and native debugging workflows.
- [x] Validate this change and repository OpenSpec strictly (51/51).

The complete installed new/init acceptance passed in
`scripts/verify_daily_development.py`; evidence is
`target/daily-installed-evidence.json`. Other candidate hosts remain an M5 gate.
