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

- [ ] 1.1 Checkpoint the verified dirty state of
  `codex/v0-2-http-tls`; retain a reviewable diff and Lore commit evidence.
- [ ] 1.2 Rebase/merge the HTTP owner from current `origin/main`, resolve shared
  runtime/stdlib/compiler/docs conflicts by behavior and tests, and open a PR.
- [ ] 1.3 Prove every other release-relevant branch is merged, superseded, or
  intentionally deferred; preserve unique commits before optional cleanup.
- [ ] 1.4 Reconcile active OpenSpec task state with implementation and archive
  historical umbrellas that no longer own work. Do not rewrite archived
  history merely to change old evidence wording.
- [ ] 1.5 Record the converged remote main SHA; verify no required source, test,
  fixture, certificate, transcript, or evidence exists only untracked or in an
  obsolete worktree.

## 2. HTTP production owner closure

- [ ] 2.1 Complete every `http-production-serving` task and archive that change
  into canonical `stdlib-http-server`.
- [ ] 2.2 Run Windows Schannel and POSIX rustls real-handshake tests with CA and
  hostname verification; no `verify=false` or plaintext fallback counts.
- [ ] 2.3 Prove managed TLS Buffers, Vec Router, keep-alive, and chunked
  streaming compose through installed `sgc` and the locked
  `http-echo-service` fixture.
- [ ] 2.4 Preserve C-only fallback linkability and `STATUS_UNSUPPORTED` behavior
  without shadowing native runtime strong symbols.
- [ ] 2.5 Update network docs and `SUPPORT_MATRIX.md` from retained host runs.

## 3. One-SHA release matrix

- [ ] 3.1 Push one candidate integration SHA and generate a retained evidence
  manifest keyed by full SHA and version.
- [ ] 3.2 On Windows x64, Linux x64, macOS x64, and macOS arm64, install the
  packaged archive outside the checkout and run version, stdlib, locked
  package, reviewed realworld, compatibility, upgrade, and rollback smokes.
- [ ] 3.3 Run host-role evidence from design D4: Schannel/CDB on Windows,
  rustls/LLDB/safety/fuzz/perf on Linux, and rustls/reactor/native package loops
  on both macOS architectures.
- [ ] 3.4 Run Linux sanitizer/leak and bounded fuzz gates over compiler,
  manifest/lock/archive, runtime handle/FFI, and portable artifact boundaries;
  retain minimized regressions for any fix.
- [ ] 3.5 Run the committed 100k/1000k compile and representative runtime gates;
  budget changes require reviewed benchmark evidence.
- [ ] 3.6 Reject mixed-SHA, skipped-required-job, expired-artifact, or local-only
  evidence as a release pass.

## 4. Candidate 1

- [ ] 4.1 Set coherent workspace/tool/runtime/stdlib versions for
  `v0.2.0-rc.1`; fail tag/workspace mismatch before publication.
- [ ] 4.2 Pass the complete section 3 matrix on candidate 1 and retain all four
  archives, checksums, provenance, evidence manifest, and compatibility inputs.
- [ ] 4.3 Prove `v0.1.0-rc.1` retained projects check/test/build/run under
  candidate 1 without silent source, manifest, or lockfile rewriting.
- [ ] 4.4 Publish candidate 1 only after every required target passes; preserve
  the previous release for rollback.

## 5. Candidate 2 and consecutive compatibility

- [ ] 5.1 Create candidate 2 from fixes that preserve the frozen Stable surface,
  or reset the candidate sequence when a P0/P1 fix changes that surface.
- [ ] 5.2 Pass the complete section 3 matrix on candidate 2 using its own SHA.
- [ ] 5.3 Install/upgrade from retained candidate 1 to candidate 2 and run
  candidate-1 source/manifest/lockfile/diagnostic/ABI fixtures unchanged.
- [ ] 5.4 Roll back from candidate 2 to candidate 1 with checksum verification;
  retained compatible packages pass and newer incompatible artifacts fail with
  actionable version diagnostics.
- [ ] 5.5 Retain both candidate evidence manifests and record whether the
  consecutive count was reset, with the behavior-changing commit if it was.

## 6. Stable release and truth sources

- [ ] 6.1 Reconcile README/README.zh-CN, language reference, compatibility
  policy, migration guide, internal release docs, release notes, and
  `SUPPORT_MATRIX.md` against the candidate-2 SHA and retained runs.
- [ ] 6.2 Keep experimental WASM/bytecode/Cranelift and platform-specific gaps
  explicitly outside the v0.2 Supported claim.
- [ ] 6.3 Publish `v0.2.0` as one complete four-target set with checksums and
  provenance only after sections 1-5 are complete.
- [ ] 6.4 Install `v0.2.0` outside the checkout on every supported host and rerun
  the release smoke from the published assets, not workflow staging paths.
- [ ] 6.5 Prove rollback to the previous published toolchain remains available
  and non-destructive after stable publication.

## 7. Final verification

- [ ] 7.1 `cargo fmt --all -- --check`
- [ ] 7.2 `cargo clippy -p sengoo-compiler -p sengoo-runtime -p sgc -p sgpm -p sgfmt -p sglsp --all-targets -- -D warnings`
- [ ] 7.3 `cargo test -p sengoo-compiler --lib`
- [ ] 7.4 `cargo test -p sengoo-runtime --lib --features native-bridge`
- [ ] 7.5 `cargo test -p sgc -- --test-threads=1`
- [ ] 7.6 `cargo test -p sgpm -- --test-threads=1`
- [ ] 7.7 `cargo test -p sgfmt`
- [ ] 7.8 `cargo test -p sglsp`
- [ ] 7.9 Run every reviewed realworld package through installed
  `sgpm check/test/fmt --check/doc/build/run --locked` as applicable.
- [ ] 7.10 Run compatibility, sanitizer/leak, bounded fuzz, performance,
  distribution, provenance, upgrade, rollback, TLS, debug, and reactor jobs
  from the retained release evidence manifest.
- [ ] 7.11 `openspec validate v0-2-release-candidate-closure --strict`
- [ ] 7.12 `openspec validate --all --strict`

## Archive Gate

- [ ] `http-production-serving` is archived and canonical HTTP truth matches the
  released implementation.
- [ ] Two consecutive v0.2 candidates pass complete one-SHA matrices and their
  artifacts/evidence remain available.
- [ ] `v0.2.0` is published as a complete four-target set and installed-asset
  smoke passes on every supported host.
- [ ] Upgrade, compatibility, and rollback are executable and non-destructive.
- [ ] No P0/P1 release blocker is open; accepted P2/platform limitations are
  explicit in the support matrix and release notes.
- [ ] All repository truth sources cite the retained release SHA/runs.
- [ ] Strict change and repository-wide OpenSpec validation pass.
