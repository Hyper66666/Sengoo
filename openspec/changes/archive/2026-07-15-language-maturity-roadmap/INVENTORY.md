# Language Maturity Inventory

Snapshot date: 2026-07-15 (program closure).

## Gate model

| Gate | Status |
| --- | --- |
| Native mainstream Phases 0–4 | **Closed** (archived children + four-host release evidence) |
| Post-v1 portable backends | **Closed for agreed scope**: experimental scalar WASM archived; production bytecode VM cancelled |
| Full production WASM (WASI/Drop/multi-OS CI) | **Successor program** — not claimed by this roadmap archive |

Alternative backends did not block native mainstream release.

## Repository state

- Integration branch: `codex/production-hardening-v1` → PR #43 base `main`.
- Release baseline: `v0.1.0-rc.1` with four-host install/upgrade evidence.
- Performance reference: Actions `29327347740`.
- Installed realworld / reviewed package: Actions `29333253316`.

## Post-v1 owners (final disposition)

| Capability | Disposition |
| --- | --- |
| Backend entry coordinator | Archived `2026-07-15-wasm-and-bytecode-backends` |
| Experimental scalar WASM | Archived `2026-07-15-wasm-backend-v1` |
| Production bytecode VM | Cancelled / archived `2026-07-15-bytecode-vm-v1` |
| Production WASI + Drop WASM | Successor OpenSpec (not opened here) |

## Support honesty

- Portable targets SUPPORT_MATRIX row: **Experimental / deferred**.
- Native production claims remain on Phases 0–4 evidence only.
