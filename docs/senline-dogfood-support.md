# Senline Dogfood Support Record

Recorded: 2026-07-15

This record separates behavior demonstrated by Sengoo package tests from
authority and promotion behavior that only the Senline Rust host can prove.
It is deliberately target-specific. A skipped platform or installed loop is
pending, never inferred from a source-development run on another host.

## Package Evidence

| Evidence scope | Status | Current evidence | Explicit limit |
| --- | --- | --- | --- |
| source-development-local | proven | On the recorded Windows development host, `senline-domain-worker` passes its locked package tests, real binary parent/child fixtures, strict JSON rejection/recovery, partial I/O, deterministic error, leakage-canary, and Buffer-lifecycle regressions. The HTTP package passes locked check/test/build and one local synthetic worker/HTTP byte-equivalence run. | This is not Senline pin evidence and does not prove an installed archive, Linux behavior, or the complete localhost matrix. |
| installed-windows-x64 | pending | The distribution tests can reject missing, tampered, wrong-target, wrong-ABI, and checkout-fallback runtime content. | No clean immutable Windows archive has yet run the complete installed worker and HTTP loops. |
| installed-linux-x64 | pending | Linux archive/install and package commands are represented in CI design. | No current-revision Ubuntu artifact and execution transcript exists yet. |
| `senline-domain-worker` package | source-only | The product package consumes `sgframing`, `sgjson_contract`, and the pure `senline_facts_to_plan` module. | Installed Windows/Linux, immutable bundle, SBOM/provenance, and reviewed Senline pin remain pending. |
| `senline-http-dogfood` package | source-only | The product harness binds `127.0.0.1:0`, reuses the worker codec/planner, passes two locked package tests, and returned a byte-identical normalized plan for one local synthetic fixture. | Installed Windows/Linux loops, larger or segmented requests, full malformed/timeout/drop/close coverage, and promotion checks remain pending. |

The local distribution manifest remains dirty/prebuilt and records
`release_eligible=false`. It cannot be promoted by renaming it, copying it to
another directory, or setting a self-asserted flag. The complete installed
manifest, payload hashes, clean source revision, and consumer-side verification
must agree before any status above changes to proven.

## Authority Boundary

| Claim | Ownership | Sengoo-side status |
| --- | --- | --- |
| planner transport and pure fixture evaluation | Sengoo package | Proven only for the local source-development evidence listed above. |
| sandbox and supervisor | Senline-owned | Senline Rust must prove process isolation, limits, restarts, deadlines, stderr draining, and whole-tree cleanup. |
| shadow | Senline-owned | Senline Rust remains authoritative and owns differential evidence and every mutation. |
| guarded-development | Senline-owned | Senline Rust owns promotion records, agreement checks, fail-closed behavior, and epoch changes. |
| internal-alpha | Senline-owned | Senline Rust owns admission, automatic demotion, stale-result rejection, and rollback eligibility. |
| rollback | Senline-owned | Senline Rust owns the switch, epoch increment, worker termination, and authoritative Rust fallback. |
| production ingress | Senline-owned | The HTTP dogfood harness is never TLS, public ingress, internal-alpha routing, or a client endpoint. |

TLS, cryptography, signed-request verification, freshness/replay, device
authorization and revocation, rate limits, durable transactions, persistence,
migrations, final plan validation, and all mutation remain in Senline Rust.
Moving any one of them requires a separate reviewed OpenSpec change.

The authority-transfer gate is explicit: TLS, cryptography, authentication authority,
replay mutation, prekey claim, durable transactions, persistence, migrations,
public ingress, internal-alpha ingress, and final mutation authority cannot move
into Sengoo under this change. Each transfer requires a separate reviewed OpenSpec
change with its own threat model, compatibility contract, implementation plan,
and verification evidence.

## Promotion Rules

The following are independent gates and cannot substitute for one another:

1. A locked source-development loop detects package and language regressions.
2. A clean target-specific installed archive proves checkout-independent
   resolution, linking, and execution with fake-failing Cargo.
3. A reproducibility comparison proves two builds of the same clean revision
   have the required identical payload and runtime identities.
4. Senline independently verifies the immutable bundle and advances its pin.
5. Senline reruns the linked differential, leakage, malformed-output,
   containment, resource, and rollback gates.

Until all applicable gates are recorded, terms such as sandboxed, supervised,
shadow-ready, guarded, internal-alpha-ready, rollback-proven, or production
supported are not Sengoo support claims.
