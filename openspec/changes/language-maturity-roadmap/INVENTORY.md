# Language Maturity Inventory

Snapshot date: 2026-07-12.

This inventory is a planning baseline, not permanent truth. Refresh it after
Phase 0 integration and whenever a child change archives.

## Repository state

- Current development branch: `codex/toolchain-transcript-evidence`.
- After `git fetch --all --prune`, it is 29 commits behind and 3 commits ahead
  of local `main`, and 28 behind / 4 ahead of `origin/main`.
- The worktree contains 169 tracked paths with changes and 30 untracked path
  roots. The tracked diff is 165 files, approximately 20,181 insertions and
  2,012 deletions; the status count is larger because archived change files
  appear as deletes plus new archive paths.
- The branch's former upstream was gone.
- Generated build outputs occupy approximately 9.51 GiB under `target/` and
  0.28 GiB under `target-codex-async/`; both are generated cleanup candidates,
  not checkpoint source inputs.
- Workspace version remained `0.1.0`; no repository release tag was present.

These facts make `mainline-release-baseline` a blocking lane.

## Capability status

| Capability | Evidence | Planning status |
| --- | --- | --- |
| Ownership/Drop | P0 archived; compiler/native drop tests | Foundation complete; retain compatibility gates |
| Generics/traits | P0 archived; monomorphization, dyn subset, derives | Foundation complete; generic stdlib consumption remains |
| Strings/formatting | P0 archived; owned String, UTF-8 boundaries, formatting | Foundation complete; no new breadth before Phase 1 closes |
| Numeric types | Most source/LLVM semantics implemented; tasks remain partial | Close target policy/docs; keep Cranelift experimental |
| Generic collections | Scalar transition wrappers dominate | Highest implementation priority after baseline |
| Debug/test | Test framework and coverage complete; two DWARF tasks open | Small, high-value closure lane |
| Concurrency | Send/Sync complete; scalar Arc/locks/channel subsets | Generic storage, structured tasks, reactor evidence open |
| Registry/package graph | Code includes semver, aliases, multiversion, remote publish/cache paths | Tasks/spec must be reconciled before new implementation |
| Distribution | Windows/Linux workflow evidence; no real release tag; no macOS matrix | External-adoption blocker |
| WASM/bytecode | Capability matrix only; no complete backend | Deferred until stable ABI gate |
| Language reference | Archived with executable examples | Refresh at milestone boundaries |
| Flagship application | Archived package workflow proof | Re-run with generic collections and released toolchain |

## Scale and ecosystem snapshot

- 35 `.sg` stdlib modules.
- 9 manifest-backed realworld projects.
- 3 first-party packages (`sggame`, `sggui`, `sgplatform`).
- CI covers Ubuntu and Windows; macOS and ARM release evidence are absent.
- The support matrix contains many `Supported subset`, `Platform-specific`, and
  `Deferred` rows, so unit implementation must not be confused with default
  production support.

## Required reconciliation checks

1. Compare every open task with code and tests before assigning implementation.
2. Record whether evidence is unit, native integration, realworld, or release
   host evidence.
3. Update stale proposal `Why` sections that describe already implemented code.
4. Archive or supersede overlapping owner changes before implementation.
5. Recompute this inventory from the integrated mainline.
