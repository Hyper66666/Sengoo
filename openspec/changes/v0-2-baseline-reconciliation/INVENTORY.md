# v0.2 Baseline Inventory

Snapshot taken from `origin/main` at `7e9e4d910` on 2026-07-16.

## Active changes on main

| Change | Current role | M0 action |
| --- | --- | --- |
| `native-debug-info` | Unique debug metadata owner | Retain; consumed by M2 |
| `http-production-serving` | Unique production HTTP owner | Retain; consumed by M3 |
| `mainstream-adoption-gap-closure` | Older adoption umbrella | Reconcile/archive after remaining children close |
| `six-pillar-gap-closure` | Older internal maturity umbrella | Reconcile evidence and archive or supersede |

## Valuable work outside main

| Branch/worktree | Evidence | Required action |
| --- | --- | --- |
| `codex/toolchain-transcript-evidence` root worktree | Dirty `sglsp`, `sgfmt`, docs, and `enhance-sglsp-smart-completion` files | Checkpoint and review; integrate through M2 without losing user changes |

Additional worktrees SHALL be classified as merged, unique, obsolete, or
generated before M0 archives. This table is a starting snapshot, not permission
to delete any branch or worktree.

## Known reconciliation items

- `SUPPORT_MATRIX.md` still references `wasm-backend-v1` as reopened although
  the agreed experimental-scalar change is archived.
- Root install examples must consistently use the published
  `v0.1.0-rc.1` channel until a later release exists.
- Active umbrella task state must be recomputed from archives and current tests.
- The smart-completion OpenSpec must enter version control with its implementation
  or be explicitly superseded by `v0-2-developer-loop`.
