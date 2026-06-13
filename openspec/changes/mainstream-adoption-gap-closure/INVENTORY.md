# Current Inventory (mainstream-adoption-gap-closure)

## Status Snapshot (June 2026)

Baseline audit performed against the working tree on branch
`codex/mainstream-usable-loop`; updated after the `async-cancellation-semantics`
archive.

| Pillar | Priority | Current state | Primary evidence |
| --- | --- | --- | --- |
| A Source-level debugging | High | No debug metadata exists anywhere in the emitted IR; debugger docs are assembly-level | Search of `compiler/src/` finds zero `!dbg`/DI/DWARF references; `docs/debugging-native.md` |
| B Cancellation semantics | High | Supported subset archived: task cancellation boundaries, `select_cancel`, and cancellable process waits; broader cancellation propagation APIs remain future work | `openspec/specs/async-cancellation/spec.md`; `examples/realworld/SUPPORT_MATRIX.md` rows: task cancellation boundaries, select loser cancellation, process cancellation |
| C Production HTTP serving | High | Reactor-backed dynamic serving works; TLS server, keep-alive, streaming, callback handlers explicitly deferred | SUPPORT_MATRIX "HTTP server dynamic serving" row; archived `2026-06-11-async-http-serving` |
| D Toolchain distribution | Medium-high | Supported subset complete for Windows/Linux package dry-run: tag workflow path, checksummed archives, install scripts, coherent versions, and installed `sgc run` smoke are proven by PR #22; a real release tag has not yet been cut and macOS remains deferred | `.github/workflows/toolchain-distribution.yml`; `docs/toolchain-distribution-windows-smoke.transcript`; `docs/toolchain-distribution-linux-smoke.transcript`; PR #22 run `27460449147` |

## Child changes (created 2026-06-11)

| Child change | Pillar | Capability delta | Status |
| --- | --- | --- | --- |
| `native-debug-info` | A | new `native-debug-info` | Proposed, strictly validated; prerequisite archive satisfied; codegen edits unblocked |
| `async-cancellation-semantics` | B | new `async-cancellation` | Archived 2026-06-12; PR #17 Windows/Ubuntu evidence recorded |
| `http-production-serving` | C | ADDED requirements on `stdlib-http-server` | Proposed, strictly validated; sequenced after Pillar B |
| `toolchain-distribution` | D | new `toolchain-distribution` | Archived 2026-06-13 after PR #22 Windows/Linux dry-run evidence |

## Dependencies (not owned here)

| Change | State | Why it gates this program |
| --- | --- | --- |
| `codegen-ir-correctness-and-gate` | Archived as `2026-06-11-codegen-ir-correctness-and-gate`; latest core-conformance run `27366666789` passed | IR emission and the conformance gate must be trustworthy before debug-metadata emission edits the same codegen path |
| `compile-scale-production-gate` | Archived 2026-06-08 with passing 100k/1000k evidence; `frontend-1000k-perf-gate` archived 2026-06-13 as superseded baseline context | Phase 5 re-runs the compile-scale gate after `-g` work to prove no default-mode regression |
| `six-pillar-gap-closure` | Active umbrella, final verification open | Prior wave must not have its capabilities re-claimed by this wave's children |

## Pillar A â€?Source-level debugging

| Item | Current evidence |
| --- | --- |
| DI metadata in codegen | None: no `!dbg`, `DISubprogram`, `DICompileUnit`, or `dwarf` matches under `compiler/src/` |
| `-g` flag on `sgc` | Not present |
| Debugger docs | `docs/debugging-native.md` exists (Pillar 6 of prior wave) but documents symbol-less native attach |
| Cache interaction | Artifact cache fingerprints runtime/source bytes; no debug-mode dimension yet (`tools/sgc/src/cache.rs`) |

## Pillar B â€?Cancellation semantics

| Item | Current evidence |
| --- | --- |
| `spawn_task` / `cancel_task` / `task_status` | Supported subset with statuses `0..3`; canceled tasks stop at the next await point and report canceled status |
| `select` (2..8) | Existing `select` remains non-canceling; `select_cancel` supports homogeneous 2..8 operands with deterministic loser cancel/drop |
| `timeout_cancel(future, ms)` | Exists and consumes its future (`async-default-followups`); single-future only |
| Process cancellation | `ProcessHandle.wait_cancellable(timeout_ms)` maps killed waits to `STATUS_CANCELED` and is proven on Windows plus Ubuntu CI |
| Reactor interest cleanup | Pending async drop and select-cancel loser cleanup unregister listener interest in the supported subset |

## Pillar C â€?Production HTTP serving

| Item | Current evidence |
| --- | --- |
| Dynamic serving | Reactor-backed async + sync serial plaintext, `Connection: close` per request; localhost smoke through real `sgc` |
| Pending bounds | 503 pending-cap overflow, 504 close fallback, slow-client cleanup covered by `cargo test -p sengoo-runtime net` |
| Keep-alive | Not implemented (deferred in matrix) |
| Handler callbacks | Not implemented; applications pull requests and respond by handle |
| Streaming bodies | Not implemented |
| TLS server | Not implemented; client stacks exist (Schannel verified on Windows; rustls implemented with POSIX reference-host success evidence still pending under `six-pillar-gap-closure`) |

## Pillar D â€?Toolchain distribution

| Item | Current evidence |
| --- | --- |
| Obtaining toolchain | `.github/workflows/toolchain-distribution.yml` builds Windows/Linux release archives in dry-run/package-smoke mode; real tag publication uses the same package path but has not been exercised by a release tag |
| Release process | `docs/internal-release.md` plus the package workflow run the release smoke matrix before packaging and publication |
| CI workflows | PR #22 run `27460449147` succeeded on `package smoke (ubuntu-latest)` and `package smoke (windows-latest)` |
| Version reporting | Installed `sgc`, `sgpm`, `sgfmt`, and `sglsp` all reported `0.1.0 (6a84e21e3083)` in both platform transcripts |
| Install scripts | `scripts/install.sh` and `scripts/install.ps1` installed the packaged archives, then installed `sgc` ran `examples/01_hello.sg` with exit code 42 and a stdlib import smoke from runner temp outside the source checkout with exit code 0 |

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
| Task cancellation boundaries | Deferred | Supported subset (complete via `async-cancellation-semantics`) |
| Select loser cancellation | Deferred | Supported subset (`select_cancel`; complete via `async-cancellation-semantics`) |
| Process cancellation | Deferred | Supported subset (complete via `async-cancellation-semantics`) |
| HTTP server dynamic serving | Supported subset (close-only) | Supported subset incl. keep-alive, handlers, streaming, TLS-server row(s) |
| (new row) Source-level debugging | â€?| Supported subset (line-level) |
| (new row) Toolchain distribution | â€?| Supported (win-x64, linux-x64) |
