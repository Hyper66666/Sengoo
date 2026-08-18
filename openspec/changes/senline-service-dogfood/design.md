## Context

Senline's `adopt-sengoo-backend-slice` change introduces Sengoo as a real participant in the v2 `submit-envelope` path. The first integration is deliberately out of process: a Rust supervisor sends verified, bounded domain facts to a Sengoo worker and compares the returned plan with the Rust reference. Rust remains the security reference monitor and the only mutation authority.

Sengoo already has native compilation, Buffer and JSON handles, synchronous standard I/O, a package workflow, and a serial plaintext HTTP server with an async request future. Product use exposes several concrete gaps: ordinary standard-I/O calls do not yet define exact partial-I/O framing behavior; Windows text-mode pipes can corrupt binary length prefixes; permissive JSON APIs do not expose enough strict object inspection; installed `sgc` can still depend on a source checkout/Cargo-built native runtime; C-to-Sengoo export-link evidence may skip linker failures; some panic paths terminate the process; and no immutable consumer evidence connects a real product failure to a fixed Sengoo artifact. These are recorded baseline defects, not claims of new Senline discovery.

The mutable `D:\Sengoo` checkout is not a release input. All implementation writes for this change are scoped to the clean `codex/senline-service-dogfood` worktree registered at `D:\Sengoo\.worktrees\senline-service-dogfood`; the mutable primary checkout remains untouched. Senline will consume only clean revisions and immutable Windows/Linux installed bundles whose complete contents are hashed. This change owns Sengoo code, packages, tests, distribution artifacts, and defect evidence. Senline owns the host sandbox, supervision, timeout, restart and circuit policy, facts construction, final contract validation, and all authoritative state changes.

## Goals / Non-Goals

**Goals:**

- Build and run a real `senline-domain-worker` from an installed Sengoo toolchain on Windows x64 and Linux x64.
- Implement bounded four-byte big-endian length-prefixed UTF-8 JSON over binary stdin/stdout with exact partial-I/O behavior.
- Add opt-in strict JSON parsing and object inspection needed to reject duplicate/unknown fields and malformed Unicode without changing existing permissive callers.
- Implement a deterministic, capability-minimal `submit-envelope` facts-to-plan module shared by the worker and a loopback HTTP dogfood package.
- Make native runtime artifacts first-class installed distribution inputs with complete target/ABI/link/dependency metadata.
- Turn every Senline-discovered Sengoo defect into a minimized regression and immutable red/minimize/fix/pin/green evidence chain.
- Exercise existing async HTTP serving in a development-only harness without presenting it as Senline ingress.

**Non-Goals:**

- Implementing or selecting cryptography, signatures, KDFs, randomness, recovery, authentication, replay state, prekey claims, acknowledgements, cursors, or database transactions in Sengoo.
- Giving Sengoo raw signed requests, ciphertext bytes, plaintext messages, tokens, credentials, private keys, recovery material, database rows/handles, or transaction handles.
- Making the Sengoo worker or HTTP harness authoritative for a Senline mutation.
- Promoting the current serial, plaintext, close-per-request HTTP server to internal-alpha or public ingress.
- Adding a general JSON Schema dialect, streaming JSON, HTTP/2, TLS serving, database persistence, or broad task-cancellation semantics.
- Requiring in-process Rust-to-Sengoo linking before the supervised worker can ship.

## Decisions

### 1. Freeze a narrow consumer contract and authority boundary

The initial operation is `submit-envelope` contract V1. The host sends `WorkerRequestV1`, containing an `EvaluationContextV1` and minimum-necessary `SubmitEnvelopeFactsV1`; the worker returns `SubmitEnvelopePlanV1`. The context carries the contract version, operation, evaluation ID, host-computed facts binding, operation epoch, and worker generation. The plan echoes those binding fields plus an exhaustive decision and stable reason.

The worker never consumes the canonical signed request and never selects an authenticated identity. It evaluates only facts already verified and bounded by Rust. A successful plan is advisory until Rust validates it against the exact context/facts and rechecks mutable state inside a new transaction. Rust remains responsible for TLS, request parsing and signature verification, freshness/replay/rate limits, authorization/revocation, cryptography, persistence, uniqueness, prekey/ACK/cursor state, audit, migrations, and final commit.

| Responsibility | Sengoo | Senline Rust kernel |
| --- | --- | --- |
| TLS, canonical signed bytes, signature verification | No access | Authoritative |
| Freshness, replay, rate limits, device authorization/revocation | Receives only eligible bounded facts | Authoritative |
| Cryptography, CSPRNG, secrets and key material | Forbidden | Authoritative through reviewed components |
| Database handles, transactions, persistence and migrations | Forbidden | Authoritative |
| Prekey, envelope, ACK and cursor mutation | Advisory plan only | Authoritative |
| Domain decision for the selected eligible operation | Deterministic candidate plan | Runs reference, validates exact binding and may reject |
| Final mutation and rollback | No authority | Authoritative |

Alternative: implement the entire v2 service in Sengoo immediately. Rejected because current serving, concurrency, database, panic-containment, fuzz, sanitizer, and soak evidence cannot protect Senline's existing security invariants.

Alternative: use Sengoo only for offline tools. Rejected because it would not exercise package installation, pipes, deterministic domain decisions, strict decoding, process lifetime, or continuous real request compatibility.

### 2. Use one-request-at-a-time framed binary standard I/O

Each message is a four-byte unsigned big-endian length followed by exactly that many UTF-8 JSON bytes. Input is capped at 32 KiB and output at 8 KiB before allocation or parse. The worker processes one request at a time, emits one response frame, and performs no implicit retry. EOF before a new prefix is clean shutdown; EOF during a prefix or payload, a zero/oversized length, trailing payload bytes, or surplus response frame is a deterministic protocol error.

The stdlib gains byte-safe Buffer access, big-endian `u32` helpers, offset-aware exact reads/writes, and explicit binary-mode initialization for standard streams. Windows must set stdin/stdout to `_O_BINARY` before any protocol I/O. Exact helpers loop over partial operations, distinguish clean EOF from truncation, validate offset/length without overflow, and never treat a short write as success.

Stdout is reserved for frames. Bounded stable diagnostic codes go to stderr; request values and parser text are not echoed. This supports a Rust supervisor without making Sengoo responsible for the host's 50 ms admission-to-validation deadline, four-process/128-queue pool, restart circuit, or OS sandbox.

Alternative: newline-delimited JSON. Rejected because arbitrary JSON strings and accidental output make record boundaries ambiguous and do not exercise binary pipe correctness.

Alternative: HTTP between Rust and Sengoo. Rejected for the first boundary because the current server is serial/plaintext and adds networking concerns to a pure planner.

### 3. Add strict JSON as an opt-in compatibility-preserving surface

Existing `json_parse` and `json_parse_buffer` remain permissive and source-compatible. A new strict parse entry point validates the exact input length, UTF-8, JSON grammar, full-input consumption, configured depth, integer range, Unicode escapes, surrogate pairs, and duplicate object keys. Object inspection exposes the exact decoded key count, keys, and values so application decoders can compare against an exhaustive allowlist and reject unknown/missing fields.

Strict failures also expose a stable machine-readable error kind for
unclassified syntax, duplicate fields, invalid UTF-8/Unicode, and trailing
bytes while preserving the existing status, offset, and human diagnostic.
Workers branch only on the error kind, never on diagnostic text. JSON builder
string creation accepts an explicit byte length so decoded `U+0000` and other
embedded bytes round-trip without C-string truncation.

V1 does not introduce general JSON Schema. The worker owns explicit per-contract decoders and validates field type, bounds, enum membership, required/optional presence, and unknown fields. Object key comparison is decoded Unicode scalar/UTF-8 equality with no normalization or case folding. This is enough for a secure fixed contract while respecting the existing follow-up gate for general schema validation and streaming JSON.

Alternative: post-process a permissively parsed map. Rejected because a map may already have collapsed duplicate keys and lost evidence needed for deterministic rejection.

### 4. Treat the installed native runtime as a release artifact

Windows and Linux distributions include the target-native `sengoo_runtime.lib` or `libsengoo_runtime.a` and every declared runtime dependency needed for native programs. The distribution manifest records target triple, runtime ABI version, payload SHA-256 hashes, ordered link arguments, dynamic dependencies, source revision, tool versions, and a build-manifest identifier.

Installed `sgc` discovers the runtime relative to its installed manifest and prefers it for normal native build/run/test. Building a runtime through Cargo remains an explicit Sengoo-source development mode only. Missing, mismatched, wrong-target, or incomplete runtime artifacts fail with a stable diagnostic rather than silently consulting `D:\Sengoo`, `SENGOO_ROOT`, a Cargo target directory, or a mutable cache.

Release smokes install into a fresh path outside the checkout, put a deliberately failing fake `cargo` first on PATH, and build/run the stdio+strict-JSON worker and async HTTP harness. Two independent builds per target compare normalized payload manifests; allowed provenance timestamps/signatures may differ, but payload hashes, ABI, link arguments, and dependency identities must match.

Alternative: let Senline invoke Sengoo from the source checkout. Rejected because it is not immutable, reproducible, portable, or representative of a user-installed language.

### 5. Keep the planner pure and packages locked

The facts-to-plan module has no network, filesystem, environment, clock, randomness, database, subprocess, direct FFI, or secret capability. It performs exhaustive decode into contract-specific types and exhaustive encode of one normalized plan. Repeating the same request produces byte-equivalent normalized output and does not retain state across evaluations.

`senline-domain-worker` and `senline-http-dogfood` use source-controlled locked dependencies. Their check/test/fmt/doc/release-build loops run with installed tools. The worker startup handshake reports protocol version, Sengoo revision, toolchain version, application version, and an embedded build-manifest identifier. These values allow the Rust host to reject inconsistency, but the host's independently verified external bundle hashes remain the trust root.

Alternative: embed a self-hash in the worker and trust it. Rejected because a replaced executable can lie about its own identity.

### 6. Use HTTP only as a loopback synthetic-data dogfood harness

The HTTP package binds an OS-selected ephemeral loopback address only, accepts only synthetic/non-secret V1 facts, enforces bounded header/body/time limits, and returns the same normalized plan/error contract. It tests `next_request_async`, timeout, pending-future drop cleanup, exactly-once response, and clean close on real Windows/Linux localhost connections.

No Senline Windows or Android client endpoint, deployment manifest, or internal-alpha route may target this harness. Its README and support evidence retain the existing serial/plaintext/`Connection: close` limits.

### 7. Make consumer defects traceable across revisions

Every Sengoo-owned failure follows one evidence state machine:

1. `red`: preserve the failing Senline fixture/transcript and classify ownership;
2. `minimize`: reduce it in this repository and add a failing compiler/runtime/stdlib/package regression;
3. `fix`: implement the smallest general fix and run affected Sengoo gates;
4. `pin`: commit the clean fix, build immutable installed bundles, record hashes/provenance, and update Senline's pin;
5. `green`: rerun the minimized regression plus Senline differential/leakage/integration gates against the pinned artifact.

Workarounds require an owner, linked defect, expiry condition, and removal test. They do not count as green. If fixture completion discovers no new defect, the process is rehearsed with a known framing/strict-JSON/installed-runtime defect or an injected failure and is labelled accordingly.

### 8. Freeze the exact V1 DTO and binding bytes before worker code

The V1 JSON surface is closed. `EvaluationContextV1` contains exactly
`contract_version`, `operation`, `operation_version`, `evaluation_id`,
`operation_epoch`, `worker_generation`, `execution_mode`, `worker_bundle_id`,
and `facts_binding`. `contract_version` is `1`, `operation` is
`submit-envelope`, identifiers and bundle IDs are opaque ASCII refs from 1
through 128 bytes, evaluation IDs are 32 lowercase hexadecimal characters,
and bindings are 64 lowercase hexadecimal characters. Integer fields encoded
as `u32` are no larger than `4294967295`; operation epoch and worker generation
are non-negative JSON-safe integers no larger than `2^53-1`.

`SubmitEnvelopeFactsV1` contains exactly `contract_version`,
`operation_version`, `identifiers`, `source_device_status`,
`source_device_capabilities`, `envelope_protocol_version`,
`ciphertext_length_bytes`, `idempotency_status`, `recipient_pending_count`,
`recipient_pending_limit`, `application_envelopes_used`,
`application_envelopes_limit`, `ciphertext_limit_bytes`, and `feature_flags`.
The identifiers object contains exactly `correlation_ref`,
`source_account_ref`, `source_device_ref`, `recipient_account_ref`,
`recipient_device_ref`, `conversation_ref`, and `envelope_ref`.
`source_device_status` is `active`; idempotency is one of `new`,
`exact_duplicate`, or `conflict`; capabilities and feature flags are sorted,
unique arrays from reviewed closed enums. Revoked, forged, stale, and
rate-limited requests never become this DTO.

`WorkerRequestV1` contains exactly `kind=evaluation`, `schema_version=1`,
`context`, and `facts`. `SubmitEnvelopePlanV1` contains exactly `kind=plan`,
`schema_version=1`, the exact echoed context and identifiers, `decision`,
`reason`, and `sengoo_module_revision`. Decisions are `store_and_enqueue`,
`duplicate_noop`, or `reject`; reasons are `accepted_new`, `exact_duplicate`,
`idempotency_conflict`, `recipient_queue_full`,
`application_budget_exhausted`, or `delivery_disabled`. Unknown operation
versions return `WorkerErrorV1` with `unsupported_operation_version` rather
than guessing a plan. Worker error envelopes contain only `kind=error`,
`schema_version`, `scope`, `code`, and a nullable `evaluation_id`; they contain
no arbitrary message. Framing, timeout, exit, overload, circuit, bundle, and
output-size errors remain host-only.

`sengoo_module_revision` is exactly 40 lowercase hexadecimal characters and
identifies the frozen planner contract fixture revision. It stays stable for
byte-equivalent V1 planner semantics and is not executable provenance. The
startup `sengoo_source_revision` identifies the actual immutable bundle source
revision and is checked independently with the external manifest.

Rust computes `facts_binding`; Sengoo only echoes it. Binding bytes start with
ASCII `senline.submit-envelope.binding.v1` plus NUL. Unsigned integers follow
in the field order above as big-endian `u32`, except epoch and generation which
are `u64`. Strings are `u32` byte length plus UTF-8 bytes. Arrays are `u32`
count plus encoded strings. The binding encodes every context field except
`facts_binding`, followed by every facts field and nested identifier in the
declared order, then SHA-256 is rendered as lowercase hexadecimal. No generic
JSON serialization or map iteration order is accepted as the canonical input.

The startup handshake contains exactly `kind=handshake`, `protocol_version`,
`sengoo_source_revision`, `toolchain_version`, `application_version`, and
`build_manifest_id`. The reported build ID is a consistency value only. Raw
fixtures and their metadata are byte-frozen in both linked changes; any field,
enum, encoding, or limit change requires a reviewed contract-version change.

### 9. Incubate missing ecosystem capabilities as reusable Sengoo packages

Senline is a demand driver for the Sengoo library ecosystem, not permission to
hide one-off helpers in the application. A missing capability is classified
before implementation:

- product DTOs and decisions remain product packages;
- domain-neutral composition over existing stdlib primitives starts as a
  locked pure Sengoo package beside the real consumer;
- a primitive that cannot be implemented safely above the runtime may extend
  `std::` only with native, compiler, LSP, compatibility, and platform tests;
- cryptography, TLS, production HTTP, durable databases, and OS sandboxing use
  reviewed bindings to mature implementations rather than new algorithms;
- capabilities that carry Senline security or mutation authority remain in
  Rust until a separate change proves an authority transfer.

The first incubated packages are `sgframing`, providing bounded `u32` big-endian
stdio frames and stable EOF/truncation/limit semantics, and
`sgjson_contract`, providing exact-object, required-field, kind, range, ASCII,
hex, closed-enum, and sorted-unique-array validation over opt-in strict JSON.
`senline_facts_to_plan` remains the product-specific typed codec/planner.

An incubated package may move to the repository-level `packages/` catalog only
after a second independent consumer or a reviewed protocol-foundation need,
stable documented API, locked source and installed-toolchain loops on Windows
and Linux, malformed/boundary coverage, and no skipped required gate. Moving a
surface into `std::` additionally requires compatibility, compiler import,
LSP, native runtime, and distribution evidence. Publication never substitutes
for these quality gates.

## Risks / Trade-offs

- [Strict JSON changes permissive behavior accidentally] -> Add new entry points, retain existing APIs unchanged, and run compatibility fixtures before archive.
- [Binary helper bugs corrupt framing or allocate from hostile lengths] -> Validate ranges before allocation, test every split point and EOF state, and fuzz raw frames with fixed caps.
- [Installed builds silently reach into the source checkout] -> Run from fresh paths with checkout variables cleared and fake-failing Cargo, and audit emitted link arguments/manifests for absolute paths.
- [Worker output is mistaken for authorization] -> Keep the authority boundary normative in both repositories; Rust independently validates every plan and owns all mutation.
- [HTTP harness is deployed as ingress] -> Enforce loopback binding in code/tests and add source/release checks rejecting client or deployment references.
- [Diagnostics leak request data] -> Emit allowlisted codes only, cap stderr, seed canaries, and scan package tests and evidence artifacts.
- [Product pressure removes workarounds without proving a fix] -> Require the immutable fixing artifact, Senline pin advance, and green consumer gate before removal.
- [Windows and Linux runtime artifacts diverge] -> Keep target-specific manifests and compare reproducibility within each target, never across targets.

## Migration Plan

1. Land strict regressions for framing, Windows binary mode, JSON, and installed-runtime discovery before implementation changes.
2. Add compatibility-preserving stdlib/runtime/toolchain fixes and build immutable Windows/Linux installed bundles.
3. Freeze V1 raw fixtures and implement the pure planner, then the framed worker around it.
4. Run the locked package loop outside the source checkout and hand the verified manifest/hash set to Senline for pinning.
5. Integrate only with Senline fixture/shadow modes; rollback is removal or disabling of the pinned worker bundle while Rust continues as reference and authority.
6. Add the loopback HTTP harness after the shared planner is green; never make it an ingress prerequisite.
7. Archive only after one complete red/minimize/fix/pin/green demonstration and strict validation in both repositories.

## Open Questions

None before implementation. Contract versions, size limits, authority ownership, compatibility policy, installed-runtime discovery rules, and harness scope are fixed by this design; any transfer of cryptographic, authentication, replay, transaction, persistence, or ingress authority requires a separate OpenSpec change.
