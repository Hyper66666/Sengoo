# senline-facts-to-plan

`senline-facts-to-plan` is the Senline product module for the
`submit-envelope` V1 decision contract. It is deliberately not a general
Sengoo ecosystem package and is not a candidate for `std::` promotion.

## Boundary

- The worker wire layer validates the closed JSON contract before constructing
  these DTOs.
- Closed capability and feature-flag arrays are represented semantically as
  `has_submit_envelope_v2` and `enqueue_delivery_enabled` booleans.
- Owned strings keep decoded context and identifiers independent from the JSON
  document. `plan_submit_envelope_v1` consumes the request and moves its exact
  context and identifiers into the returned plan.
- `senline_empty_worker_request_v1` is only a safe fallback value for Sengoo
  `Result` handling. It is not a valid request.
- Decision and reason codes are stable product-specific `i64` values.
  `senline_decision_name` and `senline_reason_name` map known values to the V1
  wire names and return an empty string for an unknown value.

The planner applies this priority order:

1. exact duplicate;
2. idempotency conflict;
3. recipient queue full;
4. application budget exhausted;
5. missing submit capability or enqueue flag;
6. accepted new envelope.

The package imports only `std::string`. It has no network, file, environment,
clock, randomness, process, database, or FFI authority.

## Verification

From this package directory:

```text
sgpm update --check
sgpm --runtime-mode source-development check --locked
sgpm --runtime-mode source-development test --locked
sgpm fmt --check --locked
sgpm --runtime-mode source-development doc --locked
sgpm --runtime-mode source-development build --release --locked
```

The package tests cover the four frozen fixture decisions plus queue-full and
both delivery-disabled inputs, precedence, exact context/identifier echoing,
stable numeric codes, and wire-name helpers.
