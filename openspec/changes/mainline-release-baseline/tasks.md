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
- [ ] 1.3 Classify paths by OpenSpec owner and generated/source status; flag
  unknown ownership rather than guessing.
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
- [ ] 2.2 Integrate capability slices in dependency order, preserving user
  changes and resolving conflicts with tests.
- [ ] 2.3 Split changes into reviewable Lore-protocol commits tied to owning
  OpenSpec changes; do not create one opaque mega-commit.
- [ ] 2.4 Confirm no required commit remains only on a gone upstream branch or
  abandoned worktree.

## 3. Reconcile truth sources

- [ ] 3.1 Audit every active change task against code/tests; record partial
  evidence and remove stale `Why` claims.
- [ ] 3.2 Update `language-maturity-roadmap/INVENTORY.md` from integrated main.
- [ ] 3.3 Update README and `examples/realworld/SUPPORT_MATRIX.md` to match
  capability and host evidence, without promoting skips to success.
- [ ] 3.4 Identify overlapping active umbrellas and record one active
  implementation owner in the roadmap inventory. Preserve the support matrix's
  `Upstream spec/change` column as historical evidence lineage rather than
  misusing it as an active-owner registry.

## 4. Integrated verification

- [ ] 4.1 Run fmt and supported-feature clippy with warnings denied.
- [ ] 4.2 Run workspace/compiler/runtime/sgc/sgpm/sglsp tests from the integrated
  revision.
- [ ] 4.3 Run realworld locked package loops and toolchain distribution dry-run
  smoke on Windows and Linux CI.
- [ ] 4.4 Run `openspec validate mainline-release-baseline --strict` and
  `openspec validate --all --strict`.
- [ ] 4.5 Push the integration branch and verify the exact tested SHA is visible
  on the configured GitHub remote.
