# Senline Service Dogfood — Lore Commit Plan

Recorded: 2026-07-15  
Worktree: `D:\Sengoo\.worktrees\senline-service-dogfood`  
Branch: `codex/senline-service-dogfood`  
Base: `1de09ccafa7e8f182af68e82352e2d4be39496b0`  
Safety checkpoint: `D:\Sengoo\.worktrees\_checkpoints\senline-service-dogfood-20260715-045753`

## Rules

- Only this worktree is writable. Never reset/clean. Never edit `D:\Sengoo` primary checkout.
- Exclude generated `target/` and `examples/**/build/` (already gitignored).
- Do not check off tasks 4.8, 5.9, or 6.5 until dual-host CI evidence exists.
- Preserve single-worker 100k resource degradation evidence for task 8.3; sharded 5.11 success is not soak success.
- Lore message fields: title; why; Constraint; Rejected; Confidence; Scope-risk; Directive; Tested; Not-tested.

## RED/GREEN strategy for already-green working tree

Pure historical RED-only commits cannot be recreated without rewriting the
working tree or inserting non-compiling intermediate commits. Policy for this
branch:

1. Each capability commit ships the minimized regression **with** the smallest
   general fix, and names the SGDOG id(s).
2. Defect ledger (`docs/senline-dogfood-defects.md`) and evidence schema remain
   the durable RED transcript; after each fix commit, evidence `fixing_commit`
   and `red_commit` fields are updated in a later docs commit once SHAs exist.
3. Optional verification: a temporary worktree at base + test-only patch may be
   used later for 7.2/7.7 rehearsal without mutating this worktree.

## Ordered commits

| # | Title (intent) | Paths (owners) | Tasks / defects |
|---|----------------|----------------|-----------------|
| 1 | Record OpenSpec change and authority fixtures | `openspec/changes/senline-service-dogfood/**` | 1.x control plane |
| 2 | Add binary Buffer/I/O exact helpers and regressions | `tools/stdlib/runtime*.{c,h}`, `io.sg`, `ffi.sg`, buffer/binary tests, sglsp surface | 2.x, SGDOG-001/006/008 |
| 3 | Add opt-in strict JSON and length-aware builders | `runtime_json.c`, `json.sg`, json tests/fuzz | 3.x, SGDOG-002/003/007/009/010 |
| 4 | Fix early-return borrow and nested AddrOf codegen | `compiler/**` | SGDOG-011/012 |
| 5 | Make installed native runtime first-class distribution | `tools/sgc/src/installed_runtime.rs`, native_toolchain, package/install scripts, distribution tests, Cargo sha2 | 4.x, SGDOG-004 |
| 6 | Propagate runtime-mode and transitive module maps in sgpm | `tools/sgpm/**` | SGDOG-005/014 |
| 7 | Release ready HTTP future drop and request-copy length | `runtime/src/net*`, `tools/stdlib/net.sg`, http_request_strings | 6.x partial, SGDOG-013/015 |
| 8 | Add sgframing/sgjson_contract and senline-domain-worker | `examples/realworld/senline-domain-worker/**` (source only) | 4A, 5.x source |
| 9 | Add loopback senline-http-dogfood harness | `examples/realworld/senline-http-dogfood/**` | 6.1–6.4 |
| 10 | Add differential/fault harness and contract fixtures | remaining `tools/sgc/tests/senline_*`, realworld test glue, CI workflows | 5.11 evidence harness, CI prep |
| 11 | Publish dogfood defect ledger, support, incubation docs | `docs/senline-*`, library-incubation | 7.1, 7.6, 8.8, 4A.1 |

After clean commits: run focused Windows gates for 2.10 path, package Windows
archive for 4.7 path, and leave 4.8/5.9/6.5 unchecked until Ubuntu evidence.

## Resource risk retained for 8.3

- Sharded 100k (8 workers × 12,500) completed with digest
  `16aebd9ec476d602c9c0d0082ee9e25a87c520c333d6dd3afeb314f8c39ea128`.
- Single-worker 100k hit a 3600s watchdog around case **44,086** with growing
  working set / declining throughput. **Do not treat shards as soak pass.**
