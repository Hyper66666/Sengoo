# Project-Driven Library Incubation

Senline is a real consumer used to expose missing Sengoo capabilities. A
consumer gap should improve the reusable library surface instead of producing
an application-only workaround, but reuse does not automatically justify a
standard-library API.

## Classification

| Capability | First home | Rule |
| --- | --- | --- |
| Product DTO, policy, or decision | Product package | Product names and versions never enter a general library. |
| Domain-neutral composition over existing stdlib primitives | Incubating pure Sengoo package | Keep dependencies locked and API independent of the first consumer. |
| Primitive unavailable above the runtime | `std::` or runtime change | Require compiler, LSP, compatibility, native, and distribution tests. |
| Cryptography, TLS, production HTTP, durable database, OS sandbox | Binding to a mature implementation | Do not create a new security algorithm or infrastructure engine for convenience. |
| Senline security, supervision, transaction, or mutation authority | Senline Rust | Authority transfer requires a separate reviewed OpenSpec change. |

The first incubating packages are `sgframing` and `sgjson_contract` under the
real worker's `packages/` directory. They use no Senline names or limits.
`senline_facts_to_plan` remains a product package. A separate `sgvalidation`
package is deferred until a non-JSON second consumer demonstrates real reuse.

## Required Package Gates

Every claimed platform runs these commands with a clean installed toolchain:

```text
sgpm update --check
sgpm --runtime-mode installed check --locked
sgpm --runtime-mode installed test --locked
sgpm fmt --check --locked
sgpm --runtime-mode installed doc --locked
sgpm --runtime-mode installed build --release --locked
sgpm publish --dry-run --locked --format json
```

Source-development runs are useful red/green evidence but are not publication
or consumer-pin evidence. A missing tool, skipped smoke, dirty revision, local
absolute path, or partial platform matrix is recorded as unverified, never
green. `sgpm publish` packages files; it does not replace check or test gates.

## Graduation

An incubating `0.x` package may move to the repository-level `packages/`
catalog after all of the following are true:

- a second independent consumer exists, or a reviewed protocol-foundation
  rationale makes the API independently owned;
- the API is documented, domain neutral, and protected by boundary, malformed,
  deterministic-error, and resource-lifetime tests;
- locked source and installed-toolchain loops pass on every claimed Windows and
  Linux target without required skips;
- publish dry-run is reproducible and contains license, provenance, and no
  mutable or absolute development path;
- SemVer and support expectations are recorded.

Stable `1.0` additionally requires one consumer outside the originating
project and immutable registry content verification. A `std::` proposal
requires at least three consumers across two application domains and two
Sengoo releases, plus a separate OpenSpec change covering compiler imports,
CLI, LSP, docs, examples, compatibility, and installed distribution behavior.

## Current Boundary

`sgframing` owns only length-prefix validation, clean EOF versus truncation,
bounded allocation, and exact I/O. It does not own JSON, retries, diagnostics,
deadlines, supervision, or process policy.

`sgjson_contract` owns exact object shape and typed validation over
`json_parse_*_strict`. It does not implement JSON Schema, coercion, defaults,
business DTOs, or a second parser.

Senline Rust continues to own facts binding, plan hashing, bundle verification,
TLS, authentication, signatures, replay and revocation, cryptography, worker
pool and circuit policy, OS sandboxing, durable transactions, persistence,
migrations, and every authoritative mutation.
