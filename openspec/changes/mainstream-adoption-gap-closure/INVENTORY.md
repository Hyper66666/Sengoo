# Current Inventory (mainstream-adoption-gap-closure)

## Status Snapshot (June 2026)

Baseline audit performed against the working tree on branch
`codex/mainstream-usable-loop` after the `async-http-serving` archive.

| Pillar | Priority | Current state | Primary evidence |
| --- | --- | --- | --- |
| A Source-level debugging | High | No debug metadata exists anywhere in the emitted IR; debugger docs are assembly-level | Search of `compiler/src/` finds zero `!dbg`/DI/DWARF references; `docs/debugging-native.md` |
| B Cancellation semantics | High | Three Deferred matrix rows with no active owner | `examples/realworld/SUPPORT_MATRIX.md` rows: task cancellation boundaries, select loser cancellation, process cancellation |
| C Production HTTP serving | High | Reactor-backed dynamic serving works; TLS server, keep-alive, streaming, callback handlers explicitly deferred | SUPPORT_MATRIX "HTTP server dynamic serving" row; archived `2026-06-11-async-http-serving` |
| D Toolchain distribution | Medium-high | Source build only (`cargo build --release`); release process documented but no published versioned binaries or install scripts | `docs/internal-release.md`; absence of release packaging workflow in `.github/workflows/` |

## Dependencies (not owned here)

| Change | State | Why it gates this program |
| --- | --- | --- |
| `codegen-ir-correctness-and-gate` | Active, tasks unstarted | IR emission and the conformance gate must be trustworthy before debug-metadata emission edits the same codegen path |
| `frontend-1000k-perf-gate` | Active, awaiting reference-host absolute targets | Phase 5 re-runs its gate to prove `-g` work did not regress default-mode compile performance |
| `six-pillar-gap-closure` | Active umbrella, final verification open | Prior wave must not have its capabilities re-claimed by this wave's children |

## Pillar A — Source-level debugging

| Item | Current evidence |
| --- | --- |
| DI metadata in codegen | None: no `!dbg`, `DISubprogram`, `DICompileUnit`, or `dwarf` matches under `compiler/src/` |
| `-g` flag on `sgc` | Not present |
| Debugger docs | `docs/debugging-native.md` exists (Pillar 6 of prior wave) but documents symbol-less native attach |
| Cache interaction | Artifact cache fingerprints runtime/source bytes; no debug-mode dimension yet (`tools/sgc/src/cache.rs`) |

## Pillar B — Cancellation semantics

| Item | Current evidence |
| --- | --- |
| `spawn_task` / `cancel_task` / `task_status` | Exist with statuses `0..3`; pending-task cancel covered by async tests; propagation contract unspecified |
| `select` (2..8) | Non-canceling loser policy pinned by `async-reactor-futures`; losers dropped via normal cleanup |
| `timeout_cancel(future, ms)` | Exists and consumes its future (`async-default-followups`); single-future only |
| Process cancellation | `ProcessHandle.wait(timeout_ms)/kill` exist; no cancellation-capable wait; matrix row Deferred |
| Reactor interest cleanup | Pending async drop unregisters listener interest (HTTP server row) — pattern to reuse for loser cancellation |

## Pillar C — Production HTTP serving

| Item | Current evidence |
| --- | --- |
| Dynamic serving | Reactor-backed async + sync serial plaintext, `Connection: close` per request; localhost smoke through real `sgc` |
| Pending bounds | 503 pending-cap overflow, 504 close fallback, slow-client cleanup covered by `cargo test -p sengoo-runtime net` |
| Keep-alive | Not implemented (deferred in matrix) |
| Handler callbacks | Not implemented; applications pull requests and respond by handle |
| Streaming bodies | Not implemented |
| TLS server | Not implemented; client stacks exist (Schannel verified on Windows; rustls implemented with POSIX reference-host success evidence still pending under `six-pillar-gap-closure`) |

## Pillar D — Toolchain distribution

| Item | Current evidence |
| --- | --- |
| Obtaining toolchain | `cargo build --release` from a cloned workspace only |
| Release process | `docs/internal-release.md` defines versioned binary smoke matrix and rollback (docs only) |
| CI workflows | Build/test/perf/realworld-e2e workflows exist; no tag-triggered packaging/publish workflow |
| Version reporting | Workspace version `0.1.0` in `Cargo.toml`; per-tool `--version` coherence unverified |
| Install scripts | None |

## Tracked-but-not-in-this-wave candidates

Recorded so they are not silently lost; each needs its own future change:

- JSON streaming / JSON5 (matrix Deferred)
- Unicode/string breadth beyond owned-String subset
- Iterator/combinator stdlib surface
- Panic/backtrace observability (no `backtrace` support in `runtime/src/`)
- All-host owned-fd reactor readiness (matrix Deferred)
- macOS host support (explicit non-goal this wave)
- Public registry hosting (explicit non-goal)

## Matrix rows this program intends to move

| SUPPORT_MATRIX row | From | To (target) |
| --- | --- | --- |
| Task cancellation boundaries | Deferred | Supported subset |
| Select loser cancellation | Deferred | Supported subset (`select_cancel`) |
| Process cancellation | Deferred | Supported subset |
| HTTP server dynamic serving | Supported subset (close-only) | Supported subset incl. keep-alive, handlers, streaming, TLS-server row(s) |
| (new row) Source-level debugging | — | Supported subset (line-level) |
| (new row) Toolchain distribution | — | Supported (win-x64, linux-x64) |
