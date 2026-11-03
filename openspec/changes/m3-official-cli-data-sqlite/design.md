# Design

Ship owning CLI, data and SQLite packages. Implementation follows the approved program in
`docs/plans/2026-11-03-daily-development-python-ecosystem.md`.

Use existing compiler AST/types, versioned editor snapshots, deterministic package graphs,
and installed-runtime verification. Additive APIs preserve edition 2026 and existing
source/data compatibility. No successful CI or human evidence may be inferred from
configuration alone.

## Verification

Positive, negative, lifecycle and outside-checkout tests cover each public boundary;
the shared evidence ledger records actual commands, host and results.
