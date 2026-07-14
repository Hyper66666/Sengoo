## 1. Fuzzing and malformed input

- [x] 1.1 Add bounded fuzz targets for lexer/parser, typecheck, and MIR lowering
  with stable no-panic/no-OOM contracts.
  - Compiler retained inputs, bounded generated input, arbitrary-source
    proptest, MIR stress, and direct MIR lowering all pass in Actions run
    `29308521559`; input bytes and case counts have enforced ceilings.
- [x] 1.2 Add fuzz targets for manifest/lockfile, registry metadata, package
  archive extraction, and selected runtime parsers/decoders.
  - The same run exercises manifest/workspace, lockfile, registry JSON,
    compressed archive, native JSON, and config decoder boundaries.
- [x] 1.3 Check in minimized regression inputs or deterministic tests for every
  fixed crash; retain seed corpora in CI artifacts/cache.
  - `fuzz/corpus` retains compiler and package/archive seeds, tests replay it
    before generated cases, and Actions uploads it even when the job fails.
- [x] 1.4 Run bounded per-commit fuzz smoke and longer scheduled fuzz jobs.
  - `.github/workflows/hardening-fuzz.yml` is fail-closed on pull requests and
    `main`; twice-weekly jobs expand the bounded case count from 512 to 4096.
    The first per-commit evidence is Actions run `29308521559`.

## 2. Native safety and longevity

- [x] 2.1 Run ASan/UBSan (or platform-equivalent supported instrumentation) over
  runtime C/Rust native integration tests.
  - Actions run `29321126548` passes the split C runtime under Clang ASan/UBSan
    and the Rust runtime/FFI suites under Rust ASan with leak detection enabled.
- [x] 2.2 Add leak-count gates for owned String, generic collections, async
  frames/tasks, registry archives, TLS/network handles, and FFI generation
  tables.
  - The sanitizer probe retains Buffer/String/opaque-handle baselines and exact
    collection Drop checks; the Rust ASan suites cover async task/file, TLS,
    registry archive, and FFI handle-table ownership. Run `29321126548` is the
    first all-green blocking evidence after the JSON document leak fix.
- [x] 2.3 Add long-running cancellation/channel/lock/reactor stress with bounded
  timeouts and deadlock diagnostics.
  - The bounded-longevity job in run `29321126548` completes ten repeated
    task-scope, channel, RwLock, and reactor/AsyncFile cycles under a 30-minute
    timeout and preserves its transcript artifact.
- [x] 2.4 Audit `unsafe`/C ABI boundaries and add negative pointer/length/handle/
  unwind tests.
  - `docs/native-safety-audit.md` maps each unsafe/native boundary to its
    validation and negative evidence; the same run rejects null pointer,
    invalid length, stale/double-close handle, and panic/unwind paths without a
    sanitizer report.

## 3. Compatibility and ABI

- [x] 3.1 Specify language edition/source compatibility and deprecation windows.
  - `docs/compatibility-policy.md` freezes the `2026` source edition,
    pre-1.0 patch/minor compatibility boundary, at-least-one-minor deprecation
    window, migration requirements, and the narrow security/soundness
    exception. Manifest and distribution tests lock the edition rejection and
    published policy.
- [x] 3.2 Version runtime ABI, manifest, lockfile, diagnostic JSON, and test-report
  schemas; reject incompatible combinations with stable diagnostics.
  - The whole native bundle now declares runtime ABI v1 in
    `runtime_shared.h`; `sgc` rejects missing/mismatched headers before
    object compilation/link with required and available versions, fingerprints
    the selected header, and a native probe calls
    `sengoo_runtime_abi_version()`. Manifest schema 1, lockfile 1/2, compiler
    diagnostic JSON 1, test/assertion JSON 1, package metadata 2, publish
    metadata 1, and reflection metadata 1 are documented and retain their
    existing unknown-version rejection tests where they are consumed.
- [x] 3.3 Add retained compatibility projects spanning the previous supported
  prerelease and current toolchain.
  - `examples/compat/v0.1.0-rc.1` and the fail-closed
    `compatibility-prerelease` workflow run the same copied, locked package
    outside the checkout with the installed previous release and current
    binaries. Actions run `29307733159` preserves the transcript and proves
    both toolchains pass locked check/test/fmt/doc/build.
- [x] 3.4 Publish the supported host/architecture/toolchain matrix and release
  support policy.
  - The compatibility policy lists Windows x64, Linux x64, macOS x64, and
    macOS arm64 target triples, LLVM 15+ / pinned LLVM 19 reference behavior,
    latest-prerelease support, retained previous-candidate compatibility input,
    and fail-closed release jobs. `docs/internal-release.md` links the policy.

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

- [x] 5.1 Install release archives into clean prefixes on every supported host
  and run hello plus stdlib import smoke outside the checkout.
  - Tag run `29259068988` installs `v0.1.0-rc.1` archives into clean
    prefixes on Windows x64, Linux x64, macOS x64, and macOS arm64, verifies
    coherent tool versions/checksums/provenance, and runs installed hello plus
    stdlib build/run smoke without checkout environment overrides.
- [ ] 5.2 Run all realworld fixtures through locked check/test/fmt/doc/build/run
  using installed binaries and stdlib.
- [ ] 5.3 Select a reviewed official package set for CLI, Python interop, and
  light-service workflows; verify publish/resolve/build/run against the release.
- [ ] 5.4 Refresh flagship docs and support matrix with release-host evidence.

## 6. Validation

- [ ] 6.1 Run `openspec validate production-hardening-v1 --strict`.
- [ ] 6.2 Run `openspec validate --all --strict`.
