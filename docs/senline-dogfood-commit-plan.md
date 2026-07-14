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

## Session 2026-07-15 handoff

### Clean revision

- Branch `codex/senline-service-dogfood` is clean after Lore commits.
- HEAD (after compare fix): `ed280b9a3`
- First stacked tip before compare fix: `a96518ddb`
- Safety checkpoint:
  `D:\Sengoo\.worktrees\_checkpoints\senline-service-dogfood-20260715-045753`
- PR: https://github.com/Hyper66666/Sengoo/pull/44
- CI dispatch: https://github.com/Hyper66666/Sengoo/actions/runs/29391088380

### Lore commits (base `1de09ccaf` → `ed280b9a3`)

1. `f3e2c538e` OpenSpec change
2. `f2a55b16f` Binary Buffer/stdio
3. `d7d53dc03` Compiler borrow/AddrOf (+ some sgc tests)
4. `dbff39d2d` Installed runtime + strict JSON payload (merged slice)
5. `bd6056d29` sgpm runtime-mode + transitive maps
6. `1777562d3` HTTP drop/copy lengths
7. `12965ef6d` senline-domain-worker packages
8. `802c689c2` senline-http-dogfood
9. `1577fd3c3` Differential/fault harness
10. `a96518ddb` Defect ledger / support / incubation
11. `ed280b9a3` Manifest compare null fix

### Local Windows evidence (not dual-host; do not check 4.8/5.9/6.5)

| Gate | Result |
| --- | --- |
| buffer/binary/handles tests | pass |
| runtime_distribution | 15/15 |
| stdlib_buffer_ / stdlib_json_ | pass |
| sgpm transitive | 2/2 |
| release tool build | pass |
| package-toolchain NoBuild | zip produced |
| install.ps1 smoke | sgc 0.1.0 (a96518ddb68f) |
| dual NoBuild package compare | `status=reproducible` (only `generated_at_utc` excluded) |
| installed worker `sgpm check/test/build --locked` with fake cargo first on PATH | pass (15 package tests + release exe) |

### Still open (30 tasks)

- **2.10** needs POSIX pipe + complete focused matrix, not Windows-only.
- **4.7–4.9** need green Ubuntu package smoke / dual-build from CI, not only local Windows.
- **4A.4 / 5.12–5.13** need full installed loops both hosts + packaging evidence.
- **5.9** needs Linux determinism digest matching Windows.
- **6.5–6.7** HTTP dual-host matrix + anti-deploy checks.
- **7.x / 8.x / 9.x** pin chain, soak (incl. case 44086), handoff.

### Next agent

1. Wait for run `29391088380` (and re-run on `ed280b9a3` if needed).
2. If Ubuntu package smoke green, extract Linux archive and record hashes; only then consider partial progress notes for 4.7.
3. Do **not** check 4.8 until both-host dual independent builds compare clean.
4. Run installed HTTP package loop and Linux determinism before 5.9/6.5.
5. Investigate single-worker 100k memory under 8.3 with checked-in sampler.
