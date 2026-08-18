## ADDED Requirements

### Requirement: The domain worker SHALL use a bounded versioned framed protocol

`senline-domain-worker` SHALL exchange exactly one request and one response at a time using a four-byte unsigned big-endian length followed by UTF-8 JSON. It SHALL reject input larger than 32 KiB, SHALL never emit output larger than 8 KiB, and SHALL reserve stdout exclusively for protocol frames.

#### Scenario: A complete request produces one complete response

- **WHEN** stdin supplies a valid `WorkerRequestV1` frame in arbitrary partial reads
- **THEN** the worker reads exactly the declared payload, evaluates it once, and writes exactly one complete `SubmitEnvelopePlanV1` frame despite partial writes
- **AND** no ordinary text precedes or follows the frame on stdout

#### Scenario: A malformed frame is rejected deterministically

- **WHEN** a prefix or payload is truncated, a declared length is zero or over 32 KiB, UTF-8 is invalid, trailing payload bytes exist, or a request schema is invalid
- **THEN** the worker returns the documented bounded protocol error when a response is possible or exits with the documented stable status
- **AND** it does not allocate from the unchecked declared length, retry the evaluation, or emit a partial plan

#### Scenario: Output cannot exceed its bound

- **WHEN** encoding a response would exceed 8 KiB
- **THEN** the worker emits the documented bounded internal error response or stable exit status
- **AND** no oversized or truncated success frame is written

### Requirement: Worker contracts SHALL bind plans to verified domain facts

Contract V1 SHALL decode `WorkerRequestV1` into `EvaluationContextV1` and `SubmitEnvelopeFactsV1` and SHALL encode `SubmitEnvelopePlanV1`. The plan SHALL echo the exact contract version, operation, evaluation ID, host-computed facts binding, operation epoch, and worker generation and SHALL contain only an exhaustive decision and stable reason allowed by V1.

#### Scenario: A valid submit-envelope request is planned

- **WHEN** the worker receives a strictly valid supported V1 context and minimum-necessary facts
- **THEN** it returns a plan whose binding fields exactly match the request and whose decision/reason pair is permitted by V1

#### Scenario: Contract fields are not extensible by accident

- **WHEN** a request has a missing, duplicate, unknown, wrong-typed, out-of-range, unknown-enum, or unknown-version field
- **THEN** exhaustive decoding rejects it before domain evaluation
- **AND** no best-effort or defaulted plan is returned

#### Scenario: A plan cannot claim host provenance

- **WHEN** the worker constructs a successful response
- **THEN** it echoes the opaque host-provided facts binding without recomputing or replacing it
- **AND** it does not emit a worker-authored digest as proof of request provenance

### Requirement: The submit-envelope planner SHALL be pure and deterministic

The shared Sengoo facts-to-plan module SHALL depend only on its typed V1 inputs, SHALL retain no request state, and SHALL have no network, filesystem, environment, clock, randomness, database, subprocess, direct FFI, credential, or secret capability.

#### Scenario: Identical facts produce identical plans

- **WHEN** the same normalized context and facts are evaluated repeatedly in one worker and across fresh workers built from the same artifact
- **THEN** the normalized plan bytes are identical on Windows x64 and Linux x64
- **AND** no earlier evaluation changes the result

#### Scenario: Prohibited values never enter the contract

- **WHEN** contract and fixture leakage tests inspect every input/output field
- **THEN** private keys, recovery material, plaintext, ciphertext bytes, raw signatures, tokens, credentials, connection strings, SQL, database rows, raw runtime handles, and transaction handles are absent

### Requirement: Sengoo SHALL remain outside Senline security and transaction authority

The worker SHALL produce advisory domain plans only. Senline's Rust security/transaction kernel SHALL remain solely responsible for TLS, canonical request parsing and signature verification, freshness and replay enforcement, account-device binding, revocation, rate limits, cryptography, durable persistence, transactions, uniqueness, prekey/ACK/cursor state, migrations, final plan validation, and every authoritative mutation.

#### Scenario: A successful worker plan has no direct side effect

- **WHEN** the worker returns an eligible or accepting plan
- **THEN** no Senline state changes unless the Rust kernel independently validates the plan and commits the operation under its own current checks
- **AND** the worker has no handle or API capable of bypassing that kernel

#### Scenario: Sengoo fails or disagrees

- **WHEN** the worker crashes, times out, emits malformed output, returns a stale binding, or differs from the Rust reference
- **THEN** the Sengoo package performs no mutation and makes no authorization decision
- **AND** Senline applies the fixed failure policy owned by its host-side change

### Requirement: The worker bundle SHALL be immutable and host-verifiable

Release-shaped worker bundles SHALL include the worker, every runtime dependency, a manifest with protocol/application/toolchain/source versions, per-file SHA-256 hashes, target and ABI metadata, licenses/SBOM inputs, and an embedded build-manifest identifier. The identifier reported by the startup handshake SHALL be a consistency value, not a self-authenticating trust root.

#### Scenario: Startup identity matches the verified bundle

- **WHEN** the host independently verifies every bundle file and starts the worker
- **THEN** the worker handshake reports the expected protocol version, Sengoo revision, toolchain version, application version, and embedded manifest identifier

#### Scenario: A bundle or handshake is inconsistent

- **WHEN** a file hash is wrong, a dependency is missing, or a handshake value differs from the external manifest
- **THEN** the bundle is ineligible for Senline pinning or execution
- **AND** no self-reported worker hash overrides the mismatch

### Requirement: Worker diagnostics SHALL be bounded and non-secret

The worker SHALL write only allowlisted stable diagnostic codes plus bounded development metadata to stderr, SHALL never copy arbitrary protocol values into diagnostics, and SHALL continue draining or terminate deterministically without blocking on diagnostic output.

#### Scenario: Malicious values reach an error path

- **WHEN** a rejected request contains randomized canaries in every text field
- **THEN** stdout contains only the bounded protocol response and stderr contains no canary or raw parser input

#### Scenario: Diagnostics exceed their budget

- **WHEN** repeated failures would exceed the package diagnostic byte/rate budget
- **THEN** later diagnostic detail is suppressed under a stable code
- **AND** protocol progress does not block on stderr

### Requirement: A loopback HTTP package SHALL dogfood the same pure planner

`senline-http-dogfood` SHALL bind only an OS-selected ephemeral loopback address, accept only bounded synthetic/non-secret V1 facts, reuse the exact facts-to-plan module, and return the same normalized V1 plan/error contract through the existing serial plaintext HTTP subset.

#### Scenario: A localhost synthetic request matches worker evaluation

- **WHEN** a supported Windows or Linux host sends the same valid synthetic V1 facts through the HTTP harness and framed worker
- **THEN** both paths return byte-equivalent normalized plans

#### Scenario: Non-loopback serving is requested

- **WHEN** configuration attempts to bind the harness to a non-loopback interface or fixed externally reachable endpoint
- **THEN** startup fails with a stable diagnostic before accepting a request

#### Scenario: Async request cleanup remains bounded

- **WHEN** an async next-request operation times out or its pending future is dropped, or a request is answered/closed
- **THEN** the existing server remains reusable where specified, each surfaced request is answered exactly once, and close releases pending resources
- **AND** no broader cancellation or ingress-readiness claim is made

#### Scenario: Product routing cannot target the harness

- **WHEN** release/source checks inspect Senline client endpoints and deployment manifests
- **THEN** no Windows client, Android client, internal-alpha route, or production route targets this package

### Requirement: Consumer-discovered defects SHALL follow a red-to-green evidence chain

Every Sengoo-owned failure discovered through Senline SHALL have a durable record connecting the original consumer failure, minimized Sengoo reproducer, failing regression, fixing clean revision, immutable installed artifacts, Senline pin update, and passing consumer verification.

#### Scenario: A genuine Sengoo defect is discovered

- **WHEN** a Senline fixture, differential run, package build, or shadow run exposes a Sengoo-owned failure
- **THEN** the failure is minimized and committed as a failing Sengoo compiler/runtime/stdlib/package test before the general fix
- **AND** green is recorded only after Senline consumes the immutable fixing artifact and reruns the linked gate

#### Scenario: A workaround is temporarily required

- **WHEN** Senline needs a bounded workaround before the Sengoo fix is pinned
- **THEN** the record includes an owner, linked defect, expiry condition, and removal test
- **AND** the workaround does not count as a fixed or green Sengoo defect

#### Scenario: No new defect appears before fixture completion

- **WHEN** the first fixture corpus completes without a newly discovered defect
- **THEN** the full evidence loop is rehearsed with a known framing, strict-JSON, or installed-runtime defect or an injected failure
- **AND** the evidence labels the rehearsal instead of claiming a new discovery

### Requirement: V1 DTO fields and facts-binding bytes SHALL be closed and byte-frozen

The worker request, plan, handshake, and error objects SHALL use only the exact
fields, bounds, and closed enums declared by Decision 8. Rust SHALL compute the
facts binding from the versioned typed big-endian encoding declared there;
Sengoo SHALL only echo it. Generic JSON serialization order SHALL NOT define
the binding. Raw reviewed fixtures and their hashes SHALL be byte-identical in
the linked Senline and Sengoo changes. Opaque identifier and bundle refs SHALL
be ASCII strings from 1 through 128 bytes. `u32` binding fields SHALL NOT
exceed `4294967295`; epoch and generation SHALL NOT exceed the JSON-safe
integer maximum `2^53-1`.

`sengoo_module_revision` SHALL be the exact 40-character lowercase-hex planner
contract fixture revision. It SHALL NOT be treated as executable provenance;
the independently verified bundle manifest and startup
`sengoo_source_revision` identify the actual build.

#### Scenario: A field or enum is not declared by V1

- **WHEN** a request, plan, handshake, or error contains an extra or unknown field or enum
- **THEN** strict decoding rejects the complete object without selecting a value or plan

#### Scenario: JSON spelling differs but typed facts are equal

- **WHEN** equivalent typed facts arrive with a different allowed JSON key order
- **THEN** Rust produces the same typed binding bytes and SHA-256
- **AND** neither host nor worker hashes raw JSON serialization as the facts binding

#### Scenario: A worker changes a binding input

- **WHEN** the plan changes any context field, opaque reference, fact, array order, epoch, generation, mode, or bundle ID
- **THEN** Rust rejects the plan before any transaction or mutation

#### Scenario: A nominally valid reference exceeds its byte bound

- **WHEN** an identifier or worker bundle reference is empty, non-ASCII, or longer than 128 bytes
- **THEN** strict decoding rejects the request before planner evaluation

#### Scenario: A plan reports the fixture revision

- **WHEN** the worker emits `sengoo_module_revision`
- **THEN** it matches the frozen planner contract fixture revision
- **AND** Rust still verifies the actual source revision and complete bundle independently

### Requirement: Product-discovered library gaps SHALL strengthen the Sengoo ecosystem

A domain-neutral capability missing during Senline implementation SHALL be
implemented as a reusable locked Sengoo package or a reviewed mature-runtime
binding, with red/green tests and explicit ownership. Product-specific schema
and policy SHALL remain outside general libraries. A package SHALL NOT graduate
to the shared catalog or standard library merely because one Senline path uses
it.

#### Scenario: Existing stdlib primitives can compose the missing capability

- **WHEN** the worker needs bounded framing or strict contract decoding and Buffer, exact I/O, and strict JSON primitives already exist
- **THEN** the capability is implemented in domain-neutral pure Sengoo packages with independent tests
- **AND** the Senline codec depends on those packages instead of duplicating their state machine or validation helpers

#### Scenario: The missing capability is security-critical infrastructure

- **WHEN** the product needs cryptography, TLS, durable database transactions, production HTTP, or OS sandboxing
- **THEN** no new security algorithm or infrastructure engine is hand-written for convenience
- **AND** the capability remains Rust-owned or uses a separately reviewed binding to a mature implementation

#### Scenario: An incubated package is proposed for graduation

- **WHEN** a package is proposed for the shared catalog or `std::`
- **THEN** its stable API, independent consumer or foundation rationale, Windows/Linux installed loops, malformed/boundary tests, documentation, and non-skipped quality gates are recorded
