# Review remediation (REQUEST CHANGES)

## M1 language-coherence (ACTIVE again)
- Implemented last-use borrow termination + borrow-escapes-owner + use-after-partial-move
- Implemented Trait::method associated-function resolution
- Gate tests expanded (non-Copy partial move, last-use, Trait::method)
- Re-merge delta into openspec/specs/memory-management

## M3 production-stdlib (ACTIVE again)
- Cursor EOF zero-cap -> BUFFER_TOO_SMALL
- cursor_free ownership; Reader/Writer traits declared; Cursor/Fd helpers
- Unicode 17 full casefold/property tables remain Deferred (docs/unicode-v0-2.md honest)
- Delta requirements merged into openspec/specs/stdlib-mainstream-usability
- http-production-serving still open residual

## M4 stability-contract (ACTIVE again)
- Fixed v0.2.0-rc.1 smoke import; sgpm test --locked green
- Stability delta merged into production-hardening
- Two consecutive multi-host RC still residual

## Umbrella
- Remains active with RESIDUAL.md; not claimed fully closed until remote gates
