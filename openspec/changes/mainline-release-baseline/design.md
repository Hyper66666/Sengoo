## Context

At the 2026-07-11 inventory snapshot, the active branch and local `main` had
diverged, the active branch had no upstream, and the worktree contained changes
across compiler, runtime, stdlib, tools, docs, tests, examples, and OpenSpec.

The integration must preserve work and expose conflicts; it must not hide them
through destructive reset or a giant opaque commit.

## Decisions

### Decision 1: Create a recoverable checkpoint first

Before branch integration, create a local commit, bundle, or patch set that can
restore all source and documentation changes. Generated targets are inventoried
separately and need not be preserved when their provenance is verified.

### Decision 2: Classify paths before staging

Every path is classified as one of:

- source owned by an active/archived change;
- test/evidence for that source;
- documentation/OpenSpec truth update;
- generated cache/artifact;
- unknown and requiring human-visible review.

Unknown paths are never silently removed.

### Decision 3: Integrate by capability ownership

Merge/rebase resolution is performed in reviewable slices matching OpenSpec
owners. Shared documentation is updated after source/test slices are green.
Commit messages follow the repository Lore protocol.

### Decision 4: Verification evidence belongs to the integrated commit

Results from an earlier branch are useful diagnostics but do not close the
baseline. Required commands run after integration and their outcome is recorded
in tasks/PR evidence.

### Decision 5: Facts are generated from repository state where possible

Counts, active changes, release tags, CI host matrices, and task status are
recomputed rather than copied from stale prose. Documentation records snapshot
dates where facts can change.

## Integration sequence

1. Record branch graph, remotes, status, untracked inventory, and disk-heavy
   generated directories.
2. Create a recoverable checkpoint.
3. Fetch and inspect latest `main` without mutating the checkpoint.
4. Reconcile source/test slices, resolving conflicts with test evidence.
5. Reconcile OpenSpec/docs from actual behavior.
6. Run the baseline verification matrix.
7. Push a reviewable branch and confirm remote visibility.

## Baseline verification matrix

- `cargo fmt --all -- --check`
- workspace clippy with warnings denied for supported feature sets
- `cargo test --workspace --locked -- --test-threads=1`
- native runtime feature tests
- realworld sgpm locked loops
- sglsp tests
- release/toolchain dry-run smoke
- `openspec validate --all --strict`

Commands may be split by CI job, but all required jobs must attach to the same
integrated revision.

## Risks

- **Conflict amplification:** resolve one owner slice at a time and rerun its
  focused tests before continuing.
- **Generated-file loss:** delete only paths verified as reproducible and inside
  the workspace target/cache policy.
- **False task closure:** require named evidence and keep partial tasks open.
- **Remote mismatch:** verify the pushed commit SHA exists on the configured
  GitHub remote before archive.
