# Language Maturity Inventory

Snapshot date: 2026-07-13.

This inventory is a planning baseline, not permanent truth. Refresh it after
Phase 0 integration and whenever a child change archives.

## Repository state

- Current development branch: `codex/toolchain-transcript-evidence`, tracking
  the branch of the same name on `origin`.
- After `git fetch --all --prune` and the topology-only merge of PR #28's
  `origin/main` merge commit, the branch is 0 commits behind and 4 commits ahead
  of `origin/main`. The merge changed no files because the reviewed PR tree was
  already an ancestor of this branch.
- The primary worktree contains 11 tracked and 14 untracked entries. Every one
  is owned by the in-progress `enhance-sglsp-smart-completion` lane: `sglsp`,
  its editor/LSP documentation and evidence, plus the formatter support/tests
  needed by that lane. They are intentionally excluded from roadmap commits
  until their owner verifies them.
- Generated output occupies approximately 18.75 GiB under `target/`; `build/`
  is negligible and the former `target-codex-async/` cache is absent. These are
  generated cleanup candidates, not source inputs.
- Eight linked worktrees were inspected. Three only contain untracked
  `Cargo.lock` files, one contains two generated baseline logs, and one old
  `async-native-execution-sync` worktree contains 21 source/OpenSpec entries.
  That async worktree remains preserved pending an equivalence audit against
  the newer async implementation; it is not silently treated as integrated.
- Workspace version remains `0.1.0`; no repository release tag is present.

The source branch is integrated and reviewable, but host/release evidence and
the preserved async-worktree audit keep `mainline-release-baseline` open.

## Capability status

| Capability | Evidence | Planning status |
| --- | --- | --- |
| Ownership/Drop | P0 archived; compiler/native drop tests | Foundation complete; retain compatibility gates |
| Generics/traits | P0 archived; monomorphization, dyn subset, derives | Foundation complete; generic stdlib consumption remains |
| Strings/formatting | P0 archived; owned String, UTF-8 boundaries, formatting | Foundation complete; no new breadth before Phase 1 closes |
| Numeric types | Archived `numeric-type-system`; compiler, native runtime, and experimental Cranelift numeric suites | Phase 1 numeric gate complete; LLVM-text remains the production backend |
| Generic collections | Archived `generic-collections`; ABI-v1 RawVec/RawHashMap/RawBTree storage, ownership callbacks, lazy adapters, and default-library fixture | Phase 1 collection gate complete; compatibility names share the generic runtime |
| Debug/test | Test framework and coverage complete; two DWARF tasks open | Small, high-value closure lane |
| Concurrency | Send/Sync complete; scalar Arc/locks/channel subsets | Generic storage, structured tasks, reactor evidence open |
| Registry/package graph | Code includes semver, aliases, multiversion, remote publish/cache paths | Tasks/spec must be reconciled before new implementation |
| Distribution | Four-host workflow covers Windows x64, Linux x64, macOS x64, and macOS arm64 packaging/install; no real release tag | External-adoption blocker until an actual signed prerelease is cut |
| WASM/bytecode | Capability matrix only; no complete backend | Deferred until stable ABI gate |
| Language reference | Archived with executable examples | Refresh at milestone boundaries |
| Flagship application | Archived package workflow proof | Re-run with generic collections and released toolchain |

## Active implementation owners

| Capability | Active owner | Overlap disposition |
| --- | --- | --- |
| Integrated branch and truth sources | `mainline-release-baseline` | `language-maturity-roadmap` only coordinates and records gates |
| Native debug metadata and debugger UX | `debugger-and-test-framework` | `native-debug-info` supplies evidence and must archive into the owner before closure |
| Registry, resolver, publish, and release artifacts | `package-registry-and-distribution` | older package/toolchain changes are evidence lineage, not active owners |
| Shared-state concurrency and async IO | `concurrency-safety-and-async-io` | archived async/runtime changes remain historical foundations |
| Fuzz, sanitizer, ABI, performance, and release soak | `production-hardening-v1` | `mainstream-adoption-gap-closure` and `six-pillar-gap-closure` are historical umbrellas for this program |
| Alternative-backend entry decision | `wasm-and-bytecode-backends` | `wasm-backend-v1` and `bytecode-vm-v1` become implementation owners only after the entry review passes |
| Smart editor completion (outside this roadmap) | `enhance-sglsp-smart-completion` | owns the current dirty primary-worktree paths; roadmap work does not stage them |

`http-production-serving` remains an independent product-surface change rather
than an owner of a roadmap release gate.

## Scale and ecosystem snapshot

- 35 `.sg` stdlib modules.
- 15 manifest-backed realworld projects.
- 3 first-party packages (`sggame`, `sggui`, `sgplatform`).
- General CI covers Ubuntu and Windows. The distribution workflow now declares
  macOS x64 and arm64 jobs as well, but no real tagged release has yet supplied
  retained cross-host artifacts for this roadmap.
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
