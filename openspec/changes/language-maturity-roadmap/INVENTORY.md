# Language Maturity Inventory

Snapshot date: 2026-07-14.

This inventory is a planning baseline, not permanent truth. Refresh it after
Phase 0 integration and whenever a child change archives.

## Repository state

- Current development branch: `codex/production-hardening-v1`, tracking the
  branch of the same name on `origin` through PR #42. At this snapshot it is 0
  commits behind and 38 commits ahead of `origin/main`.
- Workspace version and the latest release tag are both `0.1.0-rc.1` /
  `v0.1.0-rc.1`.
- Phases 0-4 are integrated and independently archived. Production hardening
  retains fuzz, sanitizer, leak/longevity, ABI, compatibility, performance, and
  installed-release evidence rather than relying on unit-test counts.
- Actions run `29327347740` is the frozen performance reference. Runs
  `29333253316` and `29333253290` pass installed realworld/reviewed-package and
  package/install/upgrade gates on Windows x64, Linux x64, macOS arm64, and
  macOS x64.

The default native release path is integrated and reviewable. Remaining roadmap
work is the post-v1 portable-target entry decision and its two owner changes.

## Capability status

| Capability | Evidence | Planning status |
| --- | --- | --- |
| Ownership/Drop | P0 archived; compiler/native drop tests | Foundation complete; retain compatibility gates |
| Generics/traits | P0 archived; monomorphization, dyn subset, derives, and generic default-library consumption | Foundation and Phase 1 consumption complete; retain conformance gates |
| Strings/formatting | P0 archived; owned String, UTF-8 boundaries, formatting, and realworld ownership use | Foundation complete; retain compatibility gates |
| Numeric types | Archived `numeric-type-system`; compiler, native runtime, and experimental Cranelift numeric suites | Phase 1 numeric gate complete; LLVM-text remains the production backend |
| Generic collections | Archived `generic-collections`; ABI-v1 RawVec/RawHashMap/RawBTree storage, ownership callbacks, lazy adapters, and default-library fixture | Phase 1 collection gate complete; compatibility names share the generic runtime |
| Debug/test | Archived `debugger-and-test-framework`; discovery/fixtures/parametrization/coverage plus Windows CDB and Linux LLDB live source proofs | Retain O0 debugger transcripts and fail-closed release-host automation; optimized debug quality remains outside the supported subset |
| Concurrency | Archived `concurrency-safety-and-async-io`; generic Arc/locks/channel, bounded executor, structured task scopes, user-Future wake/poll hardening, and flagship concurrent workload | Actions run `29298052840` closes AsyncFile runtime evidence on all four release hosts; run `29298052830` closes the generated-code Ubuntu E2E |
| Registry/package graph | Reference-server e2e proves checksum/yank handling, alias+multiversion lock edges, hostile-archive rejection, and zero-network verified-cache locked check/test/build/run | Phase 2 resolver gate complete; retain protocol conformance tests |
| Distribution | `v0.1.0-rc.1` run `29259068988` passes checksummed install/upgrade on Windows x64, Linux x64, macOS x64, and macOS arm64 and publishes provenance-attested release assets | Phase 2 release gate complete; retain tag/version/tool-manifest coherence on every candidate |
| Production hardening | Archived `production-hardening-v1`; raw performance artifact, native safety, compatibility corpus, and four-host installed-release loops | Phase 4 complete; preserve blocking gates and evidence-linked support claims |
| WASM/bytecode | Capability matrix only; no complete backend | Deferred until stable ABI gate |
| Language reference | Archived with executable examples; refreshed for production numeric, generic, concurrency, and release semantics | Native release reference current through Phase 4 |
| Flagship application | Archived package workflow proof plus run `29333253316`; locked loop uses generic maps, numeric casts, recursive walk, owned formatting, and deterministic shared-counter work | Four-host installed-release proof complete |

## Active implementation owners

| Capability | Active owner | Overlap disposition |
| --- | --- | --- |
| Program coordination and truth sources | `language-maturity-roadmap` | Phases 0-4 are archived evidence lineage, not active implementation owners |
| Alternative-backend entry decision | `wasm-and-bytecode-backends` | Must freeze the backend-neutral MIR/runtime boundary before child implementation claims |
| WASM implementation | `wasm-backend-v1` | Owns emitter, wasm32 ABI, WASI subset, validation, and differential conformance after the entry gate |
| Bytecode implementation/value decision | `bytecode-vm-v1` | Owns go/no-go, format/verifier, VM ownership semantics, and differential conformance after the entry gate |

`http-production-serving` remains an independent product-surface change rather
than an owner of a roadmap release gate.

## Scale and ecosystem snapshot

- 35 `.sg` stdlib modules.
- 11 manifest-backed realworld projects.
- 3 first-party packages (`sggame`, `sggui`, `sgplatform`).
- General CI and release gates cover Windows x64, Linux x64, macOS x64, and
  macOS arm64. Release `v0.1.0-rc.1` and the Phase 4 four-host runs supply the
  retained distribution and installed-toolchain evidence.
- The support matrix contains many `Supported subset`, `Platform-specific`, and
  `Deferred` rows, so unit implementation must not be confused with default
  production support.

## Required reconciliation checks

1. Complete the stable-MIR/runtime-ABI entry review before expanding portable
   backend behavior.
2. Record whether evidence is unit, native integration, differential
   conformance, or release-host execution.
3. Keep the current scalar portable targets explicitly experimental until their
   owner changes meet archive criteria.
4. Recompute this inventory after each portable owner archives or is cancelled
   by an evidence-backed decision.
