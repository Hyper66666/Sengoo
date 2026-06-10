## Scope

Umbrella for mainstream production readiness after `six-pillar-gap-closure`.

## Phase order

```text
Phase 0  INVENTORY baseline (perf medians, matrix stale rows, host profiles)
Phase 1  Async defaults + language polish decisions (compiler/runtime risks)
Phase 2  TLS evidence + compression stdlib proof (stdlib production risks)
Phase 3  Package/release defaults (ecosystem/release risk)
Phase 4  Integration matrix refresh + umbrella archive gate
```

Compile scale is retained as historical evidence, not the current first blocker.
Async/language work should start first because it can affect compiler/runtime
contracts. TLS, compression, and package-release work can proceed in parallel
once their public API/status boundaries are stable.

## Cross-block dependencies

```text
Block 0 -> closed compile-scale evidence; reopen only on measured regression
Block 1 -> async-default-followups; must not break existing single-thread default
Block 2 -> stdlib-https-tls; uses existing `std::http` surface and status taxonomy
Block 3 -> stdlib-default-followups; compression depends on stable Buffer/status APIs
Block 4 -> language-default-polish; coordinates payload enum across await with async owners
Block 5 -> package-release-defaults; depends on archived package-graph ownership
```

## Non-goals (this program)

- Public crates.io-scale registry population
- Full proc-macro / attribute open extension
- WASM target
- Dynamic FFI callbacks beyond the current hardened subset
- Owned-string FFI widening beyond already accepted stdlib boundaries
