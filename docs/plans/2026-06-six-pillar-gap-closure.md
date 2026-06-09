# Six-Pillar Gap Closure Implementation Plan

**Goal:** Close the six structural gaps blocking confident internal development at
scale, as specified in `openspec/changes/six-pillar-gap-closure/`.

**Architecture:** One umbrella integration contract plus six independently
validated and archived child changes, integrated in Phase 6. Start with Pillar 6
structured assertion output and real e2e so other lanes have credible
verification.

**Tech stack:** Rust workspace, Sengoo stdlib/runtime bridges, `sgpm` resolver,
`sglsp`, OpenSpec, CI perf benchmarks.

---

## Phase 0 — Setup

- Read `INVENTORY.md` and `design.md` dependency map.
- Run `openspec validate six-pillar-gap-closure --strict`.
- Archive completed upstream changes before a child claims the same canonical
  capability, or record the upstream change as an explicit blocker.
- Create the six child changes named in `proposal.md`.
- Put canonical capability deltas in the child changes, not in the umbrella.

## Phase 1 — Pillar 6 (unblock)

1. Extend existing `std::assert` typed helpers with structured failure messages.
2. Add real `realworld-e2e` integration tests.
3. Write `docs/debugging-native.md`, `docs/editor-setup.md`, `docs/internal-release.md`.

**Exit:** one realworld test uses asserts; CI job definition exists.

## Phase 2 — Pillars 1 + 4 (parallel)

**Lane A (`stdlib-production-surface`):** additive `_string` return APIs → string
collections → JSON cap → recursive IO → pipes.

**Lane B (`language-surface-expansion`):** pinned attributes table → class trait
lists → dynamic native i64 ABI arity 0..8 → async diagnostic audit.

**Exit:** realworld examples contain no `ffi_buffer_*`; class trait smoke test passes.

## Phase 3 — Pillar 3

- Manifest `package` rename field.
- Lockfile v2 package identity `(name, version, source)` with aliases on edges.
- Compatible v1 reads and deterministic `sgpm update` migration.
- Resolver + metadata tests.

**Exit:** workspace fixture with alias + two versions resolves and builds.

## Phase 4 — Pillar 2 (long pole)

- Reactor + TCP/timer futures.
- `Future` trait + flow relaxation.
- Homogeneous variadic select for 2..8 operands with rotating poll order.

**Exit:** async TCP example + native tests green on Windows/Linux CI.

## Phase 5 — Pillar 5

- Perf baselines in INVENTORY.
- Frontend memory work + CI gate.

**Exit:** 1000k absolute RSS/frontend-share gates pass on the pinned reference
host; interim mitigation does not close the child change.

## Phase 6 — Integration

- Confirm all six child changes are archived.
- Refresh SUPPORT_MATRIX.
- Run full verification §8 from `tasks.md`.
- Mark tasks complete only with proof.

---

## Suggested parallel staffing

| Stream | Owner focus | Crates |
| --- | --- | --- |
| Stdlib/runtime | P1, partial P2 IO | `tools/stdlib`, `runtime` |
| Compiler | P2, P4 | `compiler` |
| Tooling | P3, P6 | `sgpm`, `sgc`, `sglsp` |
| Perf | P5 | `compiler`, `sgc` pipeline |

---

## Verification commands

```powershell
cargo fmt --check
cargo test -p sengoo-compiler --lib
cargo test -p sengoo-runtime --lib --features native-bridge
cargo test -p sgc
cargo test -p sgpm
cargo test -p sglsp
cargo clippy -p sgc -p sgpm -p sengoo-compiler -p sengoo-runtime -- -D warnings
# realworld-e2e job (see tasks.md)
# advanced_pipeline_bench.py gate
openspec validate six-pillar-gap-closure --strict
openspec validate --all --strict
```
