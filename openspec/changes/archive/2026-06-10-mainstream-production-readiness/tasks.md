## 0. Inventory

- [x] 0.1 Refresh `examples/realworld/SUPPORT_MATRIX.md` stale async/TLS rows
  after child archive decisions. Stale baseline async rows reconciled:
  `Async IO wakeups`, `Multi-operand select`, and `Select loser cancellation`.
- [x] 0.2 Record current front-five baseline rows in `INVENTORY.md`.
- [x] 0.3 Run `openspec validate mainstream-production-readiness --strict`.

## 1. Child scaffolding

- [x] 1.1 Record historical child changes: compile-scale,
  async-reactor-futures, concurrent-async, runtime-hardening-ffi-async,
  https-tls, ecosystem.
- [x] 1.2 Run `openspec validate --all --strict`.
- [x] 1.3 Record current front-five child changes:
  `async-default-followups`, `stdlib-https-tls`,
  `stdlib-default-followups`, `language-default-polish`, and
  `package-release-defaults`.

## 2. Integration

- [x] 2.1 Each current front-five child change archives or explicitly defers its
  claims with support-matrix wording.
- [x] 2.2 SUPPORT_MATRIX reflects async, HTTPS/TLS, compression, package/release,
  and language/tooling capability rows with proof paths.
- [x] 2.3 Re-run before archive: `openspec validate --all --strict` passes.

## Archive Gate

- [x] All current front-five child changes archived or explicitly deferred with
  canonical support-matrix wording.
- [x] Re-run before archive:
  `openspec validate mainstream-production-readiness --strict` passes.
- [x] Re-run before archive: `openspec validate --all --strict` passes.
