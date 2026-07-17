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
  [`29595215669`](https://github.com/Hyper66666/Sengoo/actions/runs/29595215669)
  on tip `3e747e63b`:
  - core-language + dual-host differential + binary I/O **green**
  - installed product loops **green** (worker framed + HTTP plan equality +
    `malformed_json`@200 + GET@400) on Ubuntu and Windows
  - **worker** dual-package compare **ok=true** (33 identical payloads) both hosts
  - **HTTP** dual-package still fails closed on equal-size executable hash
    (task **8.7** remains open; product probes still ran)

## Known open Sengoo-side items

1. **Task 7.5 / 9.5** — Senline pin advancement requires a writable Senline Git revision (**Blocked** outside this worktree).
2. **Task 7.2 / 7.4 / 7.7** — true red-first defect history + complete pin/green chain not reconstructed; evidence keeps `red_status=pending-commit` and `fixing_commit=null`.
3. **Task 8.7** — HTTP dual-package executable hash still diverges under fail-closed compare (worker dual-package is bit-identical on both hosts).
4. **P2 compiler debt:** ordinary by-value legacy-handle Drop still skipped (language ABI); worker uses product-level owning helpers.

## Unsupported authority transfers

Do **not** move into Sengoo without a separate OpenSpec change:

- TLS / public or internal-alpha ingress
- Cryptography and secret material
- Authentication, replay mutation, prekey claim
- Durable transactions, persistence, migrations
- Final mutation authority
