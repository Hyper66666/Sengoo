# Senline Worker Protocol V1

Generated from `fixtures/v1/metadata.json`. Manual changes must keep the fixture validator green.

- Frame: `u32_big_endian` length plus UTF-8 JSON
- Input maximum: `32768` bytes
- Output maximum: `8192` bytes
- In flight: `1`
- Stdout: `protocol_only`
- Opaque ASCII refs and worker bundle IDs: `1..128` bytes
- `u32` binding fields: `0..4294967295`
- Epoch and worker generation: `0..9007199254740991`

EvaluationContextV1 fields: `contract_version`, `operation`, `operation_version`, `evaluation_id`, `operation_epoch`, `worker_generation`, `execution_mode`, `worker_bundle_id`, `facts_binding`.

SubmitEnvelopeFactsV1 fields: `contract_version`, `operation_version`, `identifiers`, `source_device_status`, `source_device_capabilities`, `envelope_protocol_version`, `ciphertext_length_bytes`, `idempotency_status`, `recipient_pending_count`, `recipient_pending_limit`, `application_envelopes_used`, `application_envelopes_limit`, `ciphertext_limit_bytes`, `feature_flags`.

Identifier fields: `correlation_ref`, `source_account_ref`, `source_device_ref`, `recipient_account_ref`, `recipient_device_ref`, `conversation_ref`, `envelope_ref`.

Rust computes `facts_binding` from the typed V1 encoding. Sengoo only echoes it. The startup `build_manifest_id` is a consistency value, not artifact trust evidence.

`sengoo_module_revision` is the 40-character lowercase-hex planner contract fixture revision.
It is stable for a frozen planner contract and
does not attest the running binary. `sengoo_source_revision` in the startup
handshake identifies the immutable bundle source revision verified by Rust.

Worker protocol errors contain only `kind`, `schema_version`, `scope`, `code`,
and nullable `evaluation_id`. Strict parser kinds map to `duplicate_field`,
`invalid_unicode`, or `trailing_bytes`; unclassified syntax maps to
`malformed_json`. Exhaustive schema decoding maps unknown fields and enums to
`unknown_field` and `unknown_enum`; all other schema rejections map to
`malformed_json`. Every request-level rejection leaves the worker ready for
the next frame.
