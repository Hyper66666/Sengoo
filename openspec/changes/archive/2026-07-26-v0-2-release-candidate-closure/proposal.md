## Why

Sengoo has already crossed the feature threshold for a useful native language:
`v0.1.0-rc.1` is published, the four-host toolchain workflow exists, and the
v0.2 native path includes ownership, generic collections, structured async,
source debugging, package locking, Unicode foundations, and production-shaped
HTTP routing/keep-alive/streaming on `main`.

The remaining gap is release coherence. HTTP TLS merged through PR #51, but
the `http-production-serving` owner still has composition, locked-fixture, and
archive residuals; several historical umbrella changes also describe stale
state. Current evidence is spread across different commits and CI runs. A
mainstream-quality release cannot be assembled by mixing successful results
from unrelated SHAs or by relying on files that exist only in a local
worktree.

This change closes the native v0.2 default path without adding language
breadth. It produces one converged mainline, two consecutive release-candidate
gates, reproducible installed-artifact evidence on every supported host, and a
rollback-tested `v0.2.0` release.

## What Changes

1. Converge from the latest remote `main`, integrate every release-relevant
   owner through reviewable commits/PRs, and reconcile stale active OpenSpec
   state without destructive history rewriting.
2. Complete and archive the existing `http-production-serving` owner. This
   change consumes its archived result; it does not duplicate HTTP API or
   runtime semantics.
3. Pin one release matrix to one immutable commit SHA across Windows x64,
   Linux x64, macOS x64, and macOS arm64. Installed artifacts, realworld
   packages, compatibility, host-specific TLS/debug/reactor evidence, safety,
   fuzz, and performance gates must all be attributable to that revision.
4. Require two consecutive v0.2 release candidates. Stable-contract changes
   after candidate 1 reset the count; candidate 2 must prove upgrade and
   rollback against retained candidate 1 artifacts and fixtures.
5. Publish `v0.2.0` only after checksummed archives, provenance, outside-
   checkout installation, compatibility, rollback, documentation, and strict
   OpenSpec gates are green.

This is one integration change, not another umbrella with new child changes.
Existing capability owners remain authoritative.

## Capabilities

### Modified Capabilities

- `integration-baseline`: require release convergence from the latest remote
  mainline, with reviewed branch ownership and no required untracked evidence.
- `production-hardening`: pin one-SHA host roles and executable consecutive-RC
  compatibility/reset rules for v0.2.0.
- `toolchain-distribution`: align the canonical release target set with the
  already supported four-host archive matrix.

## Impact

- Git/OpenSpec: integration inventory, owner reconciliation, archive ordering,
  and evidence tied to immutable remote SHAs.
- Runtime/stdlib: no new API owner; consumes the completed
  `http-production-serving` change and its canonical archive.
- CI/release: Windows x64, Linux x64, macOS x64, and macOS arm64 installed
  matrices; Linux safety/fuzz/performance role; platform TLS/debug/reactor
  roles; RC upgrade and rollback jobs.
- Documentation: README, language reference, migration guide, release notes,
  compatibility policy, and support matrix cite the same retained evidence.

## Non-Goals

- No new syntax, borrow model, trait system, macro system, or stdlib breadth.
- No production WASM, WASI, bytecode VM, or broad Cranelift parity claim.
- No HTTP/2, WebSocket-over-TLS, request-body streaming, or async middleware.
- No hosted public package-registry service requirement.
- No v1.0 source/ABI stability claim.
- No deletion of valuable branches or worktrees before their unique commits
  and evidence are audited.
