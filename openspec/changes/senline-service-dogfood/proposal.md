## Why

Senline needs Sengoo to participate in a real backend request path now so product work exposes concrete compiler, runtime, standard-library, packaging, and diagnostic defects. The integration must produce useful evidence without transferring Senline's cryptographic, authorization, replay, or transactional authority to an immature runtime.

## What Changes

- Add an installed-toolchain-built `senline-domain-worker` package that accepts bounded, versioned facts over framed standard I/O and returns a deterministic `submit-envelope` plan.
- Add exact binary framing support, including partial reads/writes, big-endian `u32` lengths, byte access, EOF handling, and Windows binary-mode standard streams.
- Add opt-in strict JSON object/schema inspection that rejects duplicate and unknown fields, invalid UTF-8 or Unicode escapes, trailing input, excess nesting, and out-of-range integers without silently changing permissive JSON callers.
- Make native runtime artifacts discoverable from an installed Sengoo distribution so the worker and an async HTTP dogfood package build and run outside the Sengoo source checkout without Cargo fallback.
- Add a loopback-only `senline-http-dogfood` package that reuses the pure facts-to-plan module and exercises the existing bounded, serial, plaintext HTTP server subset with synthetic non-secret facts only.
- Establish consumer-driven red/minimize/fix/pin/green evidence for each Senline-discovered Sengoo defect, including minimized regressions, fixing revision, immutable installed artifact hashes, and the Senline pin that consumes them.
- Keep the initial worker free of network, filesystem, environment, clock, randomness, database, direct FFI, and secret capabilities; its stdout is protocol-only and diagnostics are bounded on stderr.
- Explicitly leave raw request verification, device authorization and revocation, replay/freshness enforcement, cryptography, database transactions, prekey/ACK/cursor state, final plan validation, and every authoritative mutation in Senline's Rust security/transaction kernel.

## Capabilities

### New Capabilities

- `senline-service-dogfood`: Consumer-driven Sengoo worker and loopback HTTP packages, deterministic Senline plan contract, failure containment expectations, and cross-repository defect evidence.

### Modified Capabilities

- `stdlib-mainstream-usability`: Add opt-in exact binary standard-I/O helpers and strict JSON inspection needed by bounded framed workers while preserving existing permissive APIs.
- `toolchain-distribution`: Require installed per-target native runtime artifacts and metadata sufficient to compile and run native packages without a Sengoo source checkout or implicit Cargo fallback.

## Impact

- Affected Sengoo areas: native runtime and `std::io`/`std::json` bridges, `sgc` runtime discovery and linking, distribution manifests, Windows/Linux package smokes, new realworld packages, and focused compiler/runtime regressions.
- External consumer: Senline change `adopt-sengoo-backend-slice`, which pins a clean immutable Sengoo revision and complete worker bundle rather than consuming a mutable checkout.
- Runtime boundary: four-byte big-endian length-prefixed UTF-8 JSON on stdin/stdout, with 32 KiB maximum input and 8 KiB maximum output. The Senline Rust host owns deadlines, supervision, sandboxing, restart/circuit policy, contract validation, and mutation authority.
- Security scope: the packages process only verified, minimum-necessary domain facts or synthetic fixtures. They never receive ciphertext bytes, signatures, tokens, credentials, private keys, recovery material, database handles, or transaction handles.
