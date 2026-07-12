## 1. Fuzzing and malformed input

- [ ] 1.1 Add bounded fuzz targets for lexer/parser, typecheck, and MIR lowering
  with stable no-panic/no-OOM contracts.
- [ ] 1.2 Add fuzz targets for manifest/lockfile, registry metadata, package
  archive extraction, and selected runtime parsers/decoders.
- [ ] 1.3 Check in minimized regression inputs or deterministic tests for every
  fixed crash; retain seed corpora in CI artifacts/cache.
- [ ] 1.4 Run bounded per-commit fuzz smoke and longer scheduled fuzz jobs.

## 2. Native safety and longevity

- [ ] 2.1 Run ASan/UBSan (or platform-equivalent supported instrumentation) over
  runtime C/Rust native integration tests.
- [ ] 2.2 Add leak-count gates for owned String, generic collections, async
  frames/tasks, registry archives, TLS/network handles, and FFI generation
  tables.
- [ ] 2.3 Add long-running cancellation/channel/lock/reactor stress with bounded
  timeouts and deadlock diagnostics.
- [ ] 2.4 Audit `unsafe`/C ABI boundaries and add negative pointer/length/handle/
  unwind tests.

## 3. Compatibility and ABI

- [ ] 3.1 Specify language edition/source compatibility and deprecation windows.
- [ ] 3.2 Version runtime ABI, manifest, lockfile, diagnostic JSON, and test-report
  schemas; reject incompatible combinations with stable diagnostics.
- [ ] 3.3 Add retained compatibility projects spanning the previous supported
  prerelease and current toolchain.
- [ ] 3.4 Publish the supported host/architecture/toolchain matrix and release
  support policy.

## 4. Performance and resource budgets

- [ ] 4.1 Freeze committed small/medium/large/full-incremental compile scenarios
  with wall-time and peak-RSS budgets.
- [ ] 4.2 Add artifact-size, startup, and representative CLI/runtime throughput
  budgets without weakening correctness paths.
- [ ] 4.3 Fail CI on regressions beyond documented thresholds; budget changes
  require evidence and review.
- [ ] 4.4 Preserve raw benchmark metadata and distinguish trend evidence from
  cross-language marketing claims.

## 5. Released-toolchain ecosystem proof

- [ ] 5.1 Install release archives into clean prefixes on every supported host
  and run hello plus stdlib import smoke outside the checkout.
- [ ] 5.2 Run all realworld fixtures through locked check/test/fmt/doc/build/run
  using installed binaries and stdlib.
- [ ] 5.3 Select a reviewed official package set for CLI, Python interop, and
  light-service workflows; verify publish/resolve/build/run against the release.
- [ ] 5.4 Refresh flagship docs and support matrix with release-host evidence.

## 6. Validation

- [ ] 6.1 Run `openspec validate production-hardening-v1 --strict`.
- [ ] 6.2 Run `openspec validate --all --strict`.
