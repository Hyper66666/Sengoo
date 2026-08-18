## Context

The production reference remains the native LLVM-text + clang +
`sengoo-runtime` path. The project has a published `v0.1.0-rc.1` toolchain and
substantial v0.2 implementation on `origin/main`, but the final HTTP TLS slice,
OpenSpec truth, and release evidence are not yet one mainline revision.

The release problem is therefore an integration and evidence problem, not a
feature-discovery problem. This design deliberately freezes new breadth until
the v0.2 native default path is released and rollback-proven.

## Decisions

### D1: Use one closure change and retain existing owners

This change owns cross-cutting release integration only. It creates no child
changes.

- `http-production-serving` continues to own Router, keep-alive, response
  streaming, and TLS server behavior and must archive before this change.
- `production-hardening` owns safety, compatibility, performance, fuzz, and
  release evidence rules.
- `integration-baseline` owns non-destructive mainline convergence.
- `toolchain-distribution` owns release archives, installation, checksums, and
  provenance.

### D2: Converge from latest remote main, not a stale local branch

The integration branch starts at the latest `origin/main`. Candidate work is
accepted only as a reviewable commit or PR whose owner, tests, and conflict
resolution are recorded. A branch already contained in main is marked
superseded/merged rather than merged again. Uncommitted work is checkpointed
before integration; unknown changes are never erased with destructive reset.

### D3: One immutable SHA is the release evidence key

Every required gate records the same candidate commit SHA and retained CI URL
or artifact identifier. Results from another SHA may diagnose a failure but do
not satisfy the candidate gate. Required proof cannot live only in scratch
logs, untracked files, local worktrees, or deleted CI artifacts.

### D4: Pin host responsibilities

| Host | Required release role |
| --- | --- |
| Windows x64 | Installed archive and upgrade loop; Schannel HTTPS client/server composition; CDB scalar debug smoke; native/FFI tests |
| Linux x64 | Installed archive and upgrade loop; rustls HTTPS client/server composition; LLDB debug smoke; sanitizer/leak, bounded fuzz, and reference performance gates |
| macOS x64 | Installed archive and upgrade loop; rustls HTTPS composition; native async/reactor and realworld package loop |
| macOS arm64 | Installed archive and upgrade loop; rustls HTTPS composition; native async/reactor and realworld package loop |

All four hosts run version coherence, checksum verification, outside-checkout
stdlib resolution, reviewed realworld packages, compatibility fixtures, and
strict release smoke. Platform-specific limitations remain documented and are
not silently counted as Supported.

### D5: Two consecutive candidates mean two retained release-shaped runs

Candidate 1 establishes the v0.2 Stable-surface baseline. Candidate 2 must:

- pass the complete required matrix on its own immutable SHA;
- install/upgrade from retained candidate 1 artifacts;
- check/test/build/run candidate 1 compatibility fixtures without source or
  lockfile rewriting;
- prove rollback to candidate 1 remains checksum-verified and non-destructive.

A P0/P1 fix that changes a Stable source, stdlib, CLI, manifest, lockfile,
diagnostic/protocol, or runtime ABI contract resets the sequence. Test-only,
documentation-only, or evidence-retention changes do not reset it when they do
not change observable Stable behavior.

### D6: Release publication is transactional

No partial target set is published as a successful release. Tag/workspace
version mismatch, missing host artifact, failed checksum/provenance, failed
outside-checkout smoke, or failed compatibility/rollback blocks publication.
The prior published toolchain remains available throughout candidate and stable
publication.

### D7: Truth sources close with the release

Before archive, the canonical specs, OpenSpec task state, README files,
language reference, compatibility policy, migration guide, release notes, and
`SUPPORT_MATRIX.md` must cite the retained candidate/stable SHA and runs. Stale
historical umbrella changes are archived or explicitly superseded; their
history is preserved.

## Dependency Order

```text
latest origin/main
  -> integrate and archive http-production-serving
  -> reconcile mainline/OpenSpec truth
  -> candidate 1 complete matrix
  -> candidate 2 upgrade/rollback complete matrix
  -> publish v0.2.0
  -> archive v0-2-release-candidate-closure
```

## Risks and Mitigations

- Cross-platform TLS may expose POSIX-only defects. Keep claims
  Platform-specific until the actual release-host test passes.
- Long matrices can mix evidence accidentally. Require SHA-labelled jobs and a
  generated retained evidence manifest.
- A late correctness fix can invalidate candidate stability. Apply the reset
  rule rather than waiving a gate.
- Branch cleanup can erase unique work. Inventory ancestry and dirty worktrees
  before any cleanup; cleanup itself is not an archive prerequisite.

## Archive Gate

Archive only after the stable tag and release assets exist, the prior release
rollback remains executable, all modified canonical specs are merged, every
task is evidence-backed, and strict validation passes on the release revision.
