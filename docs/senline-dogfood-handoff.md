# Senline Service Dogfood Handoff

Recorded: 2026-07-15  
Sengoo branch: `codex/senline-service-dogfood`  
Clean source revision (this handoff document's parent tip may advance): see Git `HEAD`.

## Authority summary

| Concern | Owner |
| --- | --- |
| TLS, signed request verification, freshness/replay, device auth/revocation, rate limits | Senline Rust |
| Cryptography, transactions, prekey/ACK/cursor, persistence, migrations, final mutation | Senline Rust |
| Bounded facts → plan evaluation over framed stdio | Sengoo `senline-domain-worker` |
| Loopback synthetic HTTP dogfood (not ingress) | Sengoo `senline-http-dogfood` |
| Sandbox, supervisor, shadow/guarded/alpha, rollback | Senline Rust |

## Protocol surface

- V1 framed worker protocol: four-byte BE length + UTF-8 JSON
- Input ≤ 32 KiB, output ≤ 8 KiB, one request at a time, protocol-only stdout
- Fixtures: `examples/realworld/senline-domain-worker/fixtures/v1/`
- Generated protocol notes: `fixtures/v1/docs/generated/protocol-v1.md`
- Differential corpus metadata: `fixtures/v1/differential-corpus-v1.json`

## Installed layout (toolchain)

Produced by `scripts/package-toolchain.ps1` and verified by
`toolchain-distribution` CI (run `29419695542` on `ba0d03ae3`):

- Windows x64 / Linux x64 archives with native runtime library
- Dual independent A/B builds with normalized manifest compare
- Allowed provenance differences only (e.g. `generated_at_utc`)

## Worker package

`scripts/package-senline-worker.ps1` builds with installed `sgpm`/`sgc` and emits:

- `senline_domain_worker[.exe]`
- `fixtures/`
- `worker-manifest.json` (payload path, size, SHA-256)

## Defect evidence

- Ledger: `docs/senline-dogfood-defects.md` (SGDOG-2026-001 .. 015)
- Schema: `docs/senline-dogfood-evidence.schema.json`
- Records: `docs/senline-dogfood-evidence.v1.json`
- Support boundary: `docs/senline-dogfood-support.md`

## Evidence index

| Doc | Covers |
| --- | --- |
| `docs/senline-dogfood-determinism-evidence.md` | Task 5.9 dual-host digests |
| `docs/senline-dogfood-resource-methodology.md` | Tasks 8.3 / 8.4 sampler policy |
| `docs/senline-dogfood-latency-evidence.md` | Task 8.4 bulk means (partial) |
| `docs/senline-dogfood-repro-packages.md` | Task 8.7 worker/HTTP dual package (Windows) |
| `docs/senline-dogfood-defects.md` | SGDOG ledger + resource observation |
| `docs/senline-dogfood-support.md` | Authority / promotion boundary |
| `docs/senline-dogfood-evidence.v1.json` | Durable evidence records |

## Latest dual-host CI

- core-conformance run
  [`29573240622`](https://github.com/Hyper66666/Sengoo/actions/runs/29573240622)
  on tip `7a812a525` (product loops present; subsequent review reopened several
  task claims that overstated completion).

## Known open Sengoo-side items

1. **Task 7.5 / 9.5** — Senline pin advancement requires a writable Senline Git revision (blocked outside this worktree).
2. **Honest open after review remediation (still unchecked):** 5.12, 5.13, 6.5, 6.6, 7.2, 7.4, 7.7, 8.3 (partial), 8.7, 9.1, 9.2, 9.3 — see `tasks.md`.
   - Compare gate is fail-closed on executable hash mismatch (8.7 will stay red until dual packages are bit-identical).
   - Resource sampler v2 adds OLS / 10k-window / handle plateau / JSONL / `private_bytes` naming (needs 1M re-soak).
   - Evidence red/fix commits reset to `pending-commit` / null until true red-first history exists.
3. **P2 compiler debt:** ordinary by-value legacy-handle Drop still skipped (language ABI); worker uses product-level owning helpers.

## Unsupported authority transfers

Do **not** move into Sengoo without a separate OpenSpec change:

- TLS / public or internal-alpha ingress
- Cryptography and secret material
- Authentication, replay mutation, prekey claim
- Durable transactions, persistence, migrations
- Final mutation authority
