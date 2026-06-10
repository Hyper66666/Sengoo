## Why

The `six-pillar-gap-closure` program closed internal workflow gaps: stdlib
production surface, async reactor subset, package aliases, language surface,
toolchain UX, and perf gate scaffolding. Follow-up work has since landed or
specified supported subsets for concurrent async, HTTPS/TLS, graphics packages,
and realworld package loops.

Sengoo is now **internally usable** but not yet **mainstream-production ready**
relative to Rust, Go, and TypeScript. The compile-scale gate has since been
closed by focused reference-host evidence and is no longer the current blocker.
Five concrete default-readiness gates remain:

1. **Async defaults**: user-defined `Future::poll`, all-host owned-fd readiness,
   cancellation boundaries, and public cleanup-wrapper lowering.
2. **HTTPS/TLS evidence**: POSIX/reference-host trusted success, hostname
   mismatch, and runtime roundtrip proof.
3. **Stdlib compression**: real gzip-compatible `std::compress` APIs with
   resource limits and realworld fixture proof.
4. **Language polish**: pinned cfg predicates, FFI rejection parity,
   payload-enum async frame decision, and match/try diagnostic parity.
5. **Package/release defaults**: deterministic package artifacts, registry
   publish evidence, realworld release fixture, and toolchain release smoke.

This umbrella coordinates Blocks 1-5 into one auditable mainstream-production
readiness program. Each named child change owns canonical deltas and archives
independently; Block 0 remains closed compile-scale evidence.

## Child changes

| Block | Child change | Canonical ownership |
| --- | --- | --- |
| 0 Compile scale evidence | `compile-scale-production-gate` (closed/superseded) | `frontend-compile-perf`, `frontend-build-performance` |
| 1 Async defaults | `async-default-followups` | `async-default-followups`, async/runtime support matrix |
| 2 HTTPS/TLS evidence | `stdlib-https-tls` | `stdlib-mainstream-usability` |
| 3 Stdlib compression | `stdlib-default-followups` | `stdlib-mainstream-usability` |
| 4 Language polish | `language-default-polish` | `language-default-polish` |
| 5 Package/release defaults | `package-release-defaults` | `sgpm-package-graph`, `tooling-mainstream-ecosystem` |

## Relationship to six-pillar

- Block 0 is historical evidence and must not reopen as a competing blocker
  unless new measurements regress below the accepted gate.
- Block 5 depends on archived package-graph ownership and must not duplicate the
  alias/multiversion resolver semantics already in canonical specs.
- Blocks 1-5 are the current front-five implementation owners for the next
  mainstream-production push.

## Archive rule

This umbrella archives only after the active named child changes validate, pass
archive gates, and are archived or explicitly deferred in canonical specs.
