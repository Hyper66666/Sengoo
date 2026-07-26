## 0. OpenSpec and baseline

- [x] 0.1 Start from the latest `origin/main` and record the immutable baseline
  SHA in `INVENTORY.md`.
- [x] 0.2 Inventory release-relevant branches/worktrees and identify already-
  contained branches so they are not merged twice.
- [x] 0.3 Freeze owner boundaries, host roles, consecutive-RC semantics, reset
  rules, and archive ordering in `proposal.md` / `design.md`.
- [x] 0.4 Run `openspec validate v0-2-release-candidate-closure --strict`.
  (`Change 'v0-2-release-candidate-closure' is valid` on 2026-07-18.)
- [x] 0.5 Run `openspec validate --all --strict`.
  (51 passed, 0 failed on 2026-07-18.)

## 1. Mainline convergence

- [x] 1.1 Audit the residual state of `codex/v0-2-http-tls`; the only
  untracked file is generated test output (`tls_fix.txt`), while every source
  and test change is retained by its Lore commits and PR #51.
- [x] 1.2 Merge the HTTP owner through PR #51 from current `origin/main` and
  verify its compatibility, core, fuzz, safety, performance, realworld, and
  four-host distribution checks. Merge SHA:
  `af18e8fadaa20ce99ccea1b087ca57c4c3266859`.
- [x] 1.3 Prove every other release-relevant branch is merged, superseded, or
  intentionally deferred; preserve unique commits before optional cleanup.
  `INVENTORY.md` records ancestry, dirty-worktree preservation, and the
  file-bounded Senline slice disposition.
- [x] 1.4 Reconcile active OpenSpec task state with implementation and archive
  historical umbrellas that no longer own work. Do not rewrite archived
  history merely to change old evidence wording.
  Archived `six-pillar-gap-closure` and
  `mainstream-adoption-gap-closure` after reconciling their active task and
  inventory state against retained PR #51 evidence. Archived child task files
  were left as historical records.
- [x] 1.5 Record the converged remote main SHA; verify no required source, test,
  fixture, certificate, transcript, or evidence exists only untracked or in an
  obsolete worktree.
  PR #54 merged as `f5a09c4baa83f539c5d7e889c9fce3d23e2b4289`; the
  release-sequence worktree was clean, the branch inventory remained unchanged,
  and RC1 evidence was retained by GitHub rather than local-only files.

## 2. HTTP production owner closure

- [x] 2.1 Complete every `http-production-serving` task and archive that change
  into canonical `stdlib-http-server`.
  Archived as `2026-07-25-http-production-serving`; repository-wide strict
  validation passes with the canonical HTTP requirements in place.
- [x] 2.2 Run Windows Schannel and POSIX rustls real-handshake tests with CA and
  hostname verification; no `verify=false` or plaintext fallback counts.
  PR #54 candidate SHA `39a6b036aff5892eedf56e4f32ec249c07124c00`
  passed `toolchain-distribution` run `30180980360` on Windows x64, Linux x64,
  macOS x64, and macOS arm64. Every host ran the verified-CA/`localhost`
  runtime and real-`sgc` TLS composition tests without a verification bypass.
- [x] 2.3 Prove managed TLS Buffers, Vec Router, keep-alive, and chunked
  streaming compose through installed `sgc` and the locked
  `http-echo-service` fixture.
  `realworld-e2e` run `30180980373` installed the packaged toolchain outside
  the checkout on all four hosts, passed the locked fixture loop including
  `http-echo-service`, and reran the verified-CA Router/keep-alive/chunked
  streaming composition through the installed `sgc`.
- [x] 2.4 Preserve C-only fallback linkability and `STATUS_UNSUPPORTED` behavior
  without shadowing native runtime strong symbols.
  C-only link/idempotence regressions pass, and
  `real_sgc_tls_router_keep_alive_streaming_composes_with_verified_ca` proves
  the native runtime strong symbols still win when both inputs are linked.
- [x] 2.5 Update network docs and `SUPPORT_MATRIX.md` from retained host runs.
  The matrix keeps POSIX rustls Platform-specific pending the release-host
  proof rather than promoting the Windows-only local evidence.

## 3. One-SHA release matrix

- [x] 3.1 Push one candidate integration SHA and generate a retained evidence
  manifest keyed by full SHA and version.
  `v0.2.0-rc.1` run `30184545506` published `release-evidence.json` for
  `f5a09c4baa83f539c5d7e889c9fce3d23e2b4289` and version `0.2.0-rc.1`.
- [x] 3.2 On Windows x64, Linux x64, macOS x64, and macOS arm64, install the
  packaged archive outside the checkout and run version, stdlib, locked
  package, reviewed realworld, compatibility, upgrade, and rollback smokes.
  Main run `30183586721`, manual distribution run `30183605458`, and RC1 tag
  run `30184545506` passed the installed and transition loops on all four hosts.
- [x] 3.3 Run host-role evidence from design D4: Schannel/CDB on Windows,
  rustls/LLDB/safety/fuzz/perf on Linux, and rustls/reactor/native package loops
  on both macOS architectures.
  The same main SHA passed core/LLDB `30183586698`, native safety
  `30183586670`, fuzz `30183586677`, performance `30183586679`, and the RC1
  tag's Schannel/CDB plus four-host TLS/reactor/package roles.
- [x] 3.4 Run Linux sanitizer/leak and bounded fuzz gates over compiler,
  manifest/lock/archive, runtime handle/FFI, and portable artifact boundaries;
  retain minimized regressions for any fix.
  Runs `30183586670` and `30183586677` passed and retained their evidence
  artifacts on the candidate SHA.
- [x] 3.5 Run the committed 100k/1000k compile and representative runtime gates;
  budget changes require reviewed benchmark evidence.
  Performance run `30183586679` passed with retained
  `production-performance-evidence`.
- [x] 3.6 Reject mixed-SHA, skipped-required-job, expired-artifact, or local-only
  evidence as a release pass.
  The tag-only collector and generator validated six successful main-push runs,
  eight successful host package/transition jobs, unexpired artifacts, exact
  fixture hashes, and one full source revision before publication.

## 4. Candidate 1

- [x] 4.1 Set coherent workspace/tool/runtime/stdlib versions for
  `v0.2.0-rc.1`; fail tag/workspace mismatch before publication.
  Workspace crates now inherit `0.2.0-rc.1`, the four-tool coherence test
  passes, and the distribution workflow retains its tag/workspace fail-fast
  check.
- [x] 4.2 Pass the complete section 3 matrix on candidate 1 and retain all four
  archives, checksums, provenance, evidence manifest, and compatibility inputs.
  RC1 published four archives, four `.sha256` files, evidence manifest, and
  attestation `37127632`; every archive passed independent provenance verify.
- [x] 4.3 Prove `v0.1.0-rc.1` retained projects check/test/build/run under
  candidate 1 without silent source, manifest, or lockfile rewriting.
  Transition artifacts `8627024903`, `8627026644`, `8627022847`, and
  `8627031690` record previous/upgraded/rolled-back fixture loops on all hosts.
- [x] 4.4 Publish candidate 1 only after every required target passes; preserve
  the previous release for rollback.
  [`v0.2.0-rc.1`](https://github.com/Hyper66666/Sengoo/releases/tag/v0.2.0-rc.1)
  was published only after all prerequisite jobs passed; `v0.1.0-rc.1`
  remains published and checksum-installable.

## 5. Candidate 2 and consecutive compatibility

- [x] 5.1 Create candidate 2 from fixes that preserve the frozen Stable surface,
  or reset the candidate sequence when a P0/P1 fix changes that surface.
  Candidate 2 contains only retained RC1 evidence updates and the coherent
  `0.2.0-rc.2` version transition; no Stable source, stdlib, CLI, schema,
  diagnostic, protocol, or runtime ABI behavior changed, so the count remains
  consecutive.
- [x] 5.2 Pass the complete section 3 matrix on candidate 2 using its own SHA.
  Main SHA `6f9475dd956e63c886c8868278bc233a7044806b` passed core
  `30186907325`, safety `30186907312`, fuzz `30186907299`, compatibility
  `30186907296`, realworld `30186907319`, performance `30186907306`, and
  manual four-host distribution `30187594178`.
- [x] 5.3 Install/upgrade from retained candidate 1 to candidate 2 and run
  candidate-1 source/manifest/lockfile/diagnostic/ABI fixtures unchanged.
  RC2 tag run `30188454330` passed all four package and published-transition
  jobs against `examples/compat/v0.2.0-rc.1`; fixture hashes in
  `release-evidence.json` match the repository inputs.
- [x] 5.4 Roll back from candidate 2 to candidate 1 with checksum verification;
  retained compatible packages pass and newer incompatible artifacts fail with
  actionable version diagnostics.
  Transition artifacts `8628005628`, `8628007905`, `8628004938`, and
  `8628006273` retain previous/upgraded/rolled-back phases on Linux, Windows,
  macOS arm64, and macOS x64 respectively.
- [x] 5.5 Retain both candidate evidence manifests and record whether the
  consecutive count was reset, with the behavior-changing commit if it was.
  RC1 and RC2 public releases retain both manifests. Commit `0c3d2a1f5` is a
  test-only subprocess-readiness fix, so no Stable behavior changed and the
  candidate count was not reset.

## 6. Stable release and truth sources

- [x] 6.1 Reconcile README/README.zh-CN, language reference, compatibility
  policy, migration guide, internal release docs, release notes, and
  `SUPPORT_MATRIX.md` against the candidate-2 SHA and retained runs.
  Stable-preparation commit updates every named truth source and adds
  `docs/release-notes-v0.2.0.md`; final stable run identifiers are added after
  transactional publication.
- [x] 6.2 Keep experimental WASM/bytecode/Cranelift and platform-specific gaps
  explicitly outside the v0.2 Supported claim.
  Release notes and language/support references keep portable backends and
  unproved platform/framework breadth Experimental or outside v0.2.
- [x] 6.3 Publish `v0.2.0` as one complete four-target set with checksums and
  provenance only after sections 1-5 are complete.
  Stable tag run `30191226253` published four archives, four sidecars,
  `release-evidence.json`, and provenance attestation `37141681` only after all
  required main, package, and transition jobs passed.
- [x] 6.4 Install `v0.2.0` outside the checkout on every supported host and rerun
  the release smoke from the published assets, not workflow staging paths.
  The stable tag's Windows, Linux, macOS x64, and macOS arm64 package jobs each
  installed the checksum-verified archive into a clean prefix and passed the
  installed stdlib/tool-version smoke.
- [x] 6.5 Prove rollback to the previous published toolchain remains available
  and non-destructive after stable publication.
  Transition artifacts `8628932458`, `8628935571`, `8628931575`, and
  `8628932694` retain RC2 previous, stable upgraded, and RC2 rolled-back loops
  without protected fixture edits.

## 7. Final verification

- [x] 7.1 `cargo fmt --all -- --check`
- [x] 7.2 `cargo clippy -p sengoo-compiler -p sengoo-runtime -p sgc -p sgpm -p sgfmt -p sglsp --all-targets -- -D warnings`
- [x] 7.3 `cargo test -p sengoo-compiler --lib` (1123 passed)
- [x] 7.4 `cargo test -p sengoo-runtime --lib --features native-bridge` (143 passed)
- [x] 7.5 `cargo test -p sgc -- --test-threads=1` (497 unit tests plus all
  integration suites passed with 0 failures on 2026-07-26.)
- [x] 7.6 `cargo test -p sgpm -- --test-threads=1` (162 tests passed with 0
  failures on 2026-07-26.)
- [x] 7.7 `cargo test -p sgfmt` (12 unit tests passed and doc-tests passed
  with 0 failures on 2026-07-26.)
- [x] 7.8 `cargo test -p sglsp` (1 attribute verifier test and 168 LSP tests
  passed with 0 failures on 2026-07-26.)
- [x] 7.9 Run every reviewed realworld package through installed
  `sgpm check/test/fmt --check/doc/build/run --locked` as applicable.
  Stable main run `30190063481` passed the installed four-host realworld and
  reviewed package loops at the stable SHA.
- [x] 7.10 Run compatibility, sanitizer/leak, bounded fuzz, performance,
  distribution, provenance, upgrade, rollback, TLS, debug, and reactor jobs
  from the retained release evidence manifest.
  Stable `release-evidence.json` records all six successful main runs, four
  package and four transition jobs, provenance, fixture hashes, tool versions,
  and no platform skip for SHA `92c8f399f61b73d63990581c637da68572b6e133`.
- [x] 7.11 `openspec validate v0-2-release-candidate-closure --strict`
  (`Change 'v0-2-release-candidate-closure' is valid` on 2026-07-26.)
- [x] 7.12 `openspec validate --all --strict`
  (50 passed, 0 failed on 2026-07-26.)

## Archive Gate

- [x] `http-production-serving` is archived and canonical HTTP truth matches the
  released implementation.
- [x] Two consecutive v0.2 candidates pass complete one-SHA matrices and their
  artifacts/evidence remain available.
- [x] `v0.2.0` is published as a complete four-target set and installed-asset
  smoke passes on every supported host.
- [x] Upgrade, compatibility, and rollback are executable and non-destructive.
- [x] No P0/P1 release blocker is open; accepted P2/platform limitations are
  explicit in the support matrix and release notes.
- [x] All repository truth sources cite the retained release SHA/runs.
- [x] Strict change and repository-wide OpenSpec validation pass.
