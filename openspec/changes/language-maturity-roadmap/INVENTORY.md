# Language Maturity Inventory

Snapshot date: 2026-07-15 (refreshed after WASM review re-open).

This inventory is a planning baseline, not permanent truth. Refresh it after
each phase boundary archive and whenever portable-owner status changes.

## Gate model (two distinct completion lines)

| Gate | Meaning | Blocks umbrella archive? |
| --- | --- | --- |
| **Native mainstream (Phases 0–4)** | Default production language, libraries, release, concurrency, hardening | **No** for product release; **Yes** historically for starting this roadmap |
| **Post-v1 portable backends (Phase 6+)** | Experimental WASM / cancelled bytecode VM | **Yes for full `language-maturity-roadmap` archive only** |

Per the roadmap requirement *“Alternative backends SHALL wait for a stable ABI
checkpoint”* / *“neither backend blocks the earlier mainstream-default
release”*:

- **Native mainstream-default release is closed** (Phases 0–4 archived with
  cross-host evidence).
- **This umbrella change remains open** until post-v1 portable owners honestly
  finish or are split out. Do not confuse the two.

## Repository state

- Active portable work branch: `codex/production-hardening-v1`.
- Integration PR targeting **`main`**:
  https://github.com/Hyper66666/Sengoo/pull/43  
  (PR #42 targets `codex/debugger-live-composites` and is **not** the mainline
  integration vehicle.)
- Workspace / release tag baseline: `0.1.0-rc.1` / `v0.1.0-rc.1`.
- Phases 0–4: integrated and independently archived with four-host evidence.
- Actions run `29327347740` remains the frozen performance reference.
- Runs `29333253316` / `29333253290` pass installed realworld/reviewed-package
  and package/install/upgrade gates on Windows x64, Linux x64, macOS arm64,
  and macOS x64.

## Capability status

| Capability | Evidence | Planning status |
| --- | --- | --- |
| Ownership/Drop | P0 archived | Native foundation complete |
| Generics/traits | P0 archived | Native foundation complete |
| Strings/formatting | P0 archived | Native foundation complete |
| Numeric types | Archived `numeric-type-system` | Phase 1 complete; LLVM-text production backend |
| Generic collections | Archived `generic-collections` | Phase 1 complete |
| Debug/test | Archived `debugger-and-test-framework` | Phase 1 complete (O0 subset) |
| Concurrency | Archived `concurrency-safety-and-async-io` | Phase 3 complete with four-host reactor/AsyncFile evidence |
| Registry/package graph | Archived `package-registry-and-distribution` | Phase 2 complete |
| Distribution | `v0.1.0-rc.1` four-host release | Phase 2 complete |
| Production hardening | Archived `production-hardening-v1` | Phase 4 complete |
| WASM | Active `wasm-backend-v1` (experimental scalar) | **Open** — pure-core scalar only; WASI/Drop deferred |
| Bytecode VM | Archived `2026-07-15-bytecode-vm-v1` with NO-GO | **Cancelled** as production VM |
| Backend coordinator | Archived `2026-07-15-wasm-and-bytecode-backends` | Entry contract closed; children own remaining work |
| Language reference / flagship | Archived | Native release reference current through Phase 4 |

## Active implementation owners

| Capability | Active owner | Overlap disposition |
| --- | --- | --- |
| Program coordination / truth sources | `language-maturity-roadmap` (open until post-v1 honesty closes) | Phases 0–4 are archived lineage, not active implementers |
| Experimental scalar WASM | `wasm-backend-v1` (**active**) | Owns pure-core scalar emitter, `main: () -> i64`, ABI validation, signedness, fail-closed diagnostics |
| Production WASI / ownership Drop on WASM | *none yet* | Deferred follow-up; must not be claimed under experimental scalar |
| Bytecode production VM | **none** (cancelled) | Historical archive `2026-07-15-bytecode-vm-v1` + `docs/bytecode-vm-value-review.md` |
| Alternative-backend entry decision | **archived** `wasm-and-bytecode-backends` | No longer an active implementer |

`http-production-serving` remains an independent product-surface change.

## Required reconciliation checks

1. Keep native mainstream claims separate from experimental portable claims.
2. Record evidence level (unit / native / realworld / release-host).
3. Keep SUPPORT_MATRIX portable row as Experimental/deferred until WASM owner
   archives an agreed scope.
4. Refresh this inventory when `wasm-backend-v1` archives or a post-v1 split
   change is created.
