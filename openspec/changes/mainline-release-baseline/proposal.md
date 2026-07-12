## Why

The current Sengoo implementation state is distributed across a divergent
development branch, a large dirty worktree, archived and active OpenSpec
records, and support documentation that does not always match code. This makes
new feature work risky: verification can pass locally without producing a
reviewable mainline or release candidate.

Mainstream usability requires a boring foundation: a clean clone, one source of
truth, reproducible tests, and evidence attached to the commit that contains the
behavior.

## Proposal

- Preserve the complete current worktree before integration.
- Inventory every changed/untracked path by capability owner and generated vs
  source status.
- Reconcile the development branch with latest `main` without discarding
  unexplained changes.
- Split the integrated work into reviewable, OpenSpec-owned commits.
- Reconcile active tasks, archived records, README, and support-matrix claims
  with implementation and test evidence.
- Establish the full pre-feature verification baseline on the integrated branch.
- Define safe cleanup and ignore rules for generated targets and test artifacts.

## Impact

- Git history and branch integration, active OpenSpec metadata, README/support
  matrix, CI verification documentation, and generated-artifact policy.
- No source-language API or runtime behavior is intentionally changed; fixes
  required to make the baseline pass remain in their owning changes.
- Parent: `language-maturity-roadmap`, Phase 0.

## Non-goals

- Squashing unrelated capability history into one commit.
- Resetting, checking out over, or deleting unexplained user changes.
- Marking incomplete feature tasks done merely because their code compiles.
- Cutting the public prerelease; distribution owns that later gate.
