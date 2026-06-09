## Scope

This is a follow-up umbrella after the current production-readiness children. It
sets the order and done definitions for remaining mainstream-default work.

The umbrella is intentionally conservative:

- It records priorities and evidence requirements.
- It does not take ownership of every implementation delta.
- It prevents already-completed supported subsets from being re-specified as new
  work.

## Priority Model

```text
P0 Compile scale
  Blocks large-repo daily use and CI confidence.

P1 Async/runtime maturity
  Blocks service-style and concurrent tooling workloads.

P2 Ecosystem/release defaults
  Blocks routine team adoption, publishing, rollback, and IDE confidence.

P3 Stdlib thickness
  Blocks ergonomic parity for everyday scripts and small services.

P4 Language surface polish
  Blocks discoverability, expressiveness, and fewer "phase-only" restrictions.
```

The order is not a strict serial dependency for all implementation work, but it
is strict for archive claims. A lower-priority lane may land early only if it does
not weaken or obscure an unmet higher-priority gate.

## Lane Ownership

| Lane | Existing owner where possible | Follow-up ownership rule |
| --- | --- | --- |
| P0 Compile scale | `compile-scale-production-gate`; superseded evidence from `frontend-1000k-perf-gate` | Archive waits on required 100k + 1000k reference-host evidence; 2500k is optional/report-only stretch when runnable |
| P1 Async/runtime maturity | `async-reactor-futures`, `concurrent-async-runtime`, `runtime-hardening-ffi-async` | Open narrower follow-ups for missing async IO/default semantics |
| P2 Ecosystem/release defaults | `sgpm-alias-multiversion`, `ecosystem-toolchain-maturity`, `toolchain-internal-ux` | Clear package graph ownership before registry metadata changes |
| P3 Stdlib thickness | `stdlib-production-surface`, `stdlib-https-tls`, `stdlib-breadth-mainstream` | New modules need resource limits and matrix rows |
| P4 Language polish | `language-surface-expansion`; archived evidence from `try-and-match-ergonomics` and `owned-string-text` | Additive child changes own future relaxations; breaking cleanup requires migration spec |

## Evidence Rules

Every lane needs:

- OpenSpec validation for its change and `--all`.
- Tests that exercise success and stable failure behavior.
- User-facing docs or matrix rows when support is partial.
- At least one realworld example or package workflow when the capability is
  intended for mainstream use.

Evidence cannot be only "unsupported is documented" unless the lane explicitly
owns a deferred capability row.

## Support Matrix Contract

`examples/realworld/SUPPORT_MATRIX.md` and package-specific matrices remain the
single source of truth for user-facing status. Rows must use:

- `Supported`
- `Supported subset`
- `Platform-specific`
- `Deferred`
- `Accepted risk`

Rows must cite tests, examples, or documented skip evidence. Stale rows block
archive.

## Archive Strategy

This umbrella archives only after:

1. Higher-priority gates are closed or explicitly superseded by a narrower active
   change.
2. All active child changes it cites are archived or removed from the contract.
3. Support matrices no longer contradict canonical specs.
4. The compile-scale P0 gate has real reference-host evidence.
