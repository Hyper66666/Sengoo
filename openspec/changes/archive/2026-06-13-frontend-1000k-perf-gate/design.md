## Scope

Child change for `six-pillar-gap-closure` Pillar 5.

## Supersession

`compile-scale-production-gate` supersedes this child change. It owns the
final reference-host evidence, CI ladder closure, and canonical spec promotion.
This change now archives as historical baseline context only and must not
re-apply overlapping spec deltas.

## Delta ownership

- `frontend-compile-perf`: 1000k absolute targets, regression gates, interning/memory evidence
- `frontend-build-performance`: ADDED preservation requirement only; canonical cache
  scenarios remain owned by the existing spec

## Baseline protocol

- Pin host profile, compiler rev, generator seed, C++ command in `INVENTORY.md`
- Median of three runs for RSS, frontend share, e2e time

## Targets (1000k default mode)

| Metric | Target |
| --- | --- |
| Peak RSS vs C++ | ≤ 1.8x |
| Frontend share | ≤ 65% |

## Regression gates

- +10% RSS, +5pp frontend share, +10% e2e vs checked-in snapshot produce CI evidence now
- Relative regression gates remain active after the absolute targets are met;
  snapshots may be updated only with checked-in before/after evidence
- PR-blocking enforcement starts only after the reference-host archive gate is
  green; until then CI runs the hard gate in report-only mode so unrelated
  branches are not blocked by an explicitly open performance target.

## Constraints

- Do not weaken runtime bundle fingerprint tests
