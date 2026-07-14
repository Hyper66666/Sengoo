# Senline Domain Worker

This realworld package is the Sengoo side of the linked Senline
`adopt-sengoo-backend-slice` change. The root package will own bounded framed
stdio and exhaustive V1 decoding. `senline_facts_to_plan` is the pure planner
module shared with the later loopback HTTP dogfood package.

Domain-neutral capabilities are incubated beside the first consumer:

- `sgframing` owns bounded big-endian framing and exact stdio composition.
- `sgjson_contract` owns closed-object and typed validation helpers over
  strict `std::json` documents.
- `senline_facts_to_plan` remains product-specific and is not a stdlib
  candidate.
- `senline_build_identity` is a generated product package that embeds startup
  consistency values from reviewed bundle inputs.

The source-development worker now consumes both incubating packages in a real
binary stdin/stdout loop. It strictly decodes the complete V1 context and
facts DTOs, evaluates the product planner, emits deterministic plans, returns
the frozen unsupported-version and bounded protocol errors, recovers after
each rejected request, and shuts down cleanly on EOF. Real parent/child tests
cover all five frozen request/response cases plus schema and strict-parser
recovery in a single process per test.

The worker has no TLS, authentication, replay, cryptography, persistence,
transaction, clock, randomness, filesystem, environment, network, subprocess,
or mutation authority. Senline Rust verifies minimum-necessary facts, computes
the facts binding, validates every returned plan, re-reads mutable state in a
new transaction, and remains the only mutation authority.

The checked-in `fixtures/v1` corpus is the byte-frozen contract source. Stdout
is protocol-only. Plan `sengoo_module_revision` identifies the frozen planner
contract fixture revision. `scripts/generate-build-identity.ps1`
deterministically writes both the `senline_build_identity` source and external
handshake JSON from the source revision, toolchain/application versions, and
bundle build-manifest identity. The checked-in generated values are
fixture-mode inputs and are not pin evidence. Release packaging must regenerate
them from its reviewed manifest; Senline still verifies every external bundle
file and rejects any self-reported identity mismatch.

Release-mode stderr has an empty allowlist: request bytes, parser text, field
values, and arbitrary messages are never emitted. Host-owned exit status and
bounded error envelopes carry stable failure categories. Development metadata
requires a separate reviewed allowlist before it may appear on stderr.

Strict JSON exposes stable machine-readable kinds for duplicate fields,
invalid Unicode, trailing input, and unclassified syntax. The worker snapshots
that kind immediately after a failed parse, before creating an error document,
and never parses diagnostic text. Length-aware JSON building preserves owned
strings containing `U+0000`; checked-in raw malformed fixtures and process
tests lock the subtype mapping and recovery behavior.
