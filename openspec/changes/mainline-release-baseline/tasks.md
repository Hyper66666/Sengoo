## 1. Preserve and inventory

- [x] 1.1 Record branch/upstream/remotes, divergence from `main`, dirty tracked
  paths, untracked paths, and generated-directory sizes.
  - Recorded in `language-maturity-roadmap/INVENTORY.md` after a 2026-07-12
    fetch/prune: branch/upstream state, local and remote-main divergence,
    tracked/untracked counts, diff size, and generated target sizes are
    explicit.
- [x] 1.2 Create and verify a recoverable checkpoint containing every source,
  test, documentation, example, and OpenSpec change.
  - Commit `db37d8bb8` preserves 218 non-generated paths (25,330 insertions,
    1,756 deletions) with an explicit directive to split by OpenSpec owner
    before final integration. `git status` was clean immediately after the
    commit, proving no source/test/doc/example/OpenSpec path remained only in
    the worktree.
- [x] 1.3 Classify paths by OpenSpec owner and generated/source status; flag
  unknown ownership rather than guessing.
  - The refreshed roadmap inventory assigns all 25 primary-worktree entries to
    `enhance-sglsp-smart-completion`, records generated target sizes separately,
    and preserves the older async worktree for an explicit equivalence audit.
- [x] 1.4 Verify cleanup targets resolve inside the workspace before removing
  generated caches; update ignore/cleanup documentation where needed.
  - Added `/target-*/` beside the canonical `/target/` ignore rule. Before
    deleting `target-codex-async`, its resolved absolute path was required to
    remain directly under the workspace and its leaf name to start with
    `target-`; source/untracked checkpoint inputs were not removed.

## 2. Reconcile mainline

- [x] 2.1 Fetch latest remote state and compare local `main`, `origin/main`, and
  the development branch.
  - `git fetch --all --prune` confirms the development branch is 29 behind / 3
    ahead of local `main`, 28 behind / 4 ahead of `origin/main`, and its former
    upstream remains gone.
- [x] 2.2 Integrate capability slices in dependency order, preserving user
  changes and resolving conflicts with tests.
  - Merged local `main` into `codex/toolchain-transcript-evidence`, preserving
    the advanced compiler/stdlib work while accepting the newer registry and
    portable-backend surfaces. Conflict follow-ups restored strict native
    collections execution, DWARF statement lines, test discovery source maps,
    and race-free runtime object publication.
- [x] 2.3 Split changes into reviewable Lore-protocol commits tied to owning
  OpenSpec changes; do not create one opaque mega-commit.
  - Numeric and generic-collection work is recorded as independent,
    evidence-bearing commits; `c4d0e9864` contains only the collection ABI,
    stdlib, tests, fixture, documentation, and its OpenSpec archive.
- [ ] 2.4 Confirm no required commit remains only on a gone upstream branch or
  abandoned worktree.

## 3. Reconcile truth sources

- [x] 3.1 Audit every active change task against code/tests; record partial
  evidence and remove stale `Why` claims.
  - The roadmap audit rechecked debugger/native-DI, registry/distribution,
    concurrency, hardening, and both portable backend owners. Implemented
    subsets, local failures, host-only evidence, and true code gaps are now
    separated in the inventory and support documentation.
- [x] 3.2 Update `language-maturity-roadmap/INVENTORY.md` from integrated main.
- [x] 3.3 Update README and `examples/realworld/SUPPORT_MATRIX.md` to match
  capability and host evidence, without promoting skips to success.
  - Distribution is described as configured rather than released, the current
    four-host failure is not counted as artifact evidence, and real-binary
    package loops explicitly reject skipped-tool success.
- [x] 3.4 Identify overlapping active umbrellas and record one active
  implementation owner in the roadmap inventory. Preserve the support matrix's
  `Upstream spec/change` column as historical evidence lineage rather than
  misusing it as an active-owner registry.

## 4. Integrated verification

- [x] 4.1 Run fmt and supported-feature clippy with warnings denied.
  - `cargo fmt --all -- --check` and
    `cargo clippy --workspace --all-targets -- -D warnings` pass on the
    integrated Windows worktree.
- [x] 4.2 Run workspace/compiler/runtime/sgc/sgpm/sglsp tests from the integrated
  revision.
  - `cargo test --workspace --all-targets` passed the compiler (1017), runtime
    (68), sgc unit (459), and all integration targets except two test-report
    schema/source-map assertions. Their fix was then verified by 20 command
    tests and the complete 2/2 `test_discovery` e2e target.
- [ ] 4.3 Run realworld locked package loops and toolchain distribution dry-run
  smoke on Windows and Linux CI.
- [x] 4.4 Run `openspec validate mainline-release-baseline --strict` and
  `openspec validate --all --strict`.
  - `npx.cmd @fission-ai/openspec validate --all --strict` reports 47 passed,
    0 failed, including `mainline-release-baseline`.
- [ ] 4.5 Push the integration branch and verify the exact tested SHA is visible
  on the configured GitHub remote.
