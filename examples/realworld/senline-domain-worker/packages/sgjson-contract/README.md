# sgjson_contract

`sgjson_contract` is an incubating `0.x` Sengoo package for small, explicit,
closed JSON contracts. It builds on `std::json`; it does not implement a JSON
parser or a JSON Schema dialect.

## Input Contract

Every `JsonValue` passed to this package must belong to a live `JsonDoc`
created by `json_parse_strict` or `json_parse_buffer_strict`. `JsonValue` does
not retain parser provenance, so the package cannot detect a value produced by
the permissive parser or recover duplicate-key evidence that was already lost.

An exact object decoder combines:

1. `sgjson_exact_object_fields` with a contract-owned
   `fn(String) -> bool` callback for the closed field count and explicit
   allowlist. The callback must represent exactly the expected field set for
   that `expected_len`, not a superset shared by multiple schema versions.
2. One `sgjson_required_*` call for every declared field.

This rejects missing, additional, substituted, and wrong-typed fields without
coercion or defaults. The callback receives each actual decoded key as an
owned `String`; comparison is exact UTF-8 equality with no normalization or
case folding. An equal-count field substitution is therefore classified as
`SGJSON_UNKNOWN_FIELD` before required getters run. A key longer than the
caller's non-negative `max_key_len` also fails closed as
`SGJSON_UNKNOWN_FIELD`; an invalid negative limit returns
`SGJSON_OUT_OF_RANGE`.

The strict parser is a required precondition: the allowlist helper cannot
recover duplicate-key evidence after a permissive parser has collapsed it.
`json_parse_strict` and `json_parse_buffer_strict` reject duplicates before
this decoder pattern inspects fields.

## API

- Required values: string, integer, boolean, object, and array.
- Exact object fields through a contract-owned callback allowlist.
- Integer ranges and three-value closed enums.
- Bounded ASCII strings and exact-length lowercase hexadecimal strings.
- Bounded strictly sorted, duplicate-free string arrays.

Stable errors are `SGJSON_MISSING_FIELD`, `SGJSON_UNKNOWN_FIELD`,
`SGJSON_WRONG_KIND`, `SGJSON_OUT_OF_RANGE`, `SGJSON_UNKNOWN_ENUM`,
`SGJSON_INVALID_STRING`, `SGJSON_UNSORTED_OR_DUPLICATE`, and
`SGJSON_RUNTIME_FAILURE`. `SGJSON_WRONG_KIND` is reserved for a successfully
inspected JSON value whose kind does not match the contract. A failed runtime
lookup, inspection, or owned-string extraction returns
`SGJSON_RUNTIME_FAILURE` instead of being presented as malformed input.

## Incubation Status

- First consumer: `senline-domain-worker`.
- Independent consumers: none.
- Supported evidence: locked Windows source-development positive and negative
  package tests, API docs, release build, and a local publish dry run.
- Missing evidence: long-soak and fault-injection lifecycle gates, malformed
  corpus, installed Windows/Linux toolchains, package license metadata/files,
  registry publication, and an independent non-Senline consumer.

Project DTOs, Senline enums, coercion, defaulting, and general schema execution
do not belong in this package.
