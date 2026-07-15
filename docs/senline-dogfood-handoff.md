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

## Known open Sengoo-side items

1. **Task 8.3** — single-worker 100k watchdog near case 44086 / memory growth; 1M soak not claimed.
2. **Task 7.5 / 9.5** — Senline pin advancement requires a writable Senline Git revision.
3. **Task 5.12 / 6.5 Linux loops** — complete when dual-host installed worker/HTTP CI transcripts land.

## Unsupported authority transfers

Do **not** move into Sengoo without a separate OpenSpec change:

- TLS / public or internal-alpha ingress
- Cryptography and secret material
- Authentication, replay mutation, prekey claim
- Durable transactions, persistence, migrations
- Final mutation authority
