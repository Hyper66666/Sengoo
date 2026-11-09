# sgschema 0.1.0 (experimental)

Sengoo JSON tool-contract validation, without network access, schema fetching,
type coercion, stdout output or tool execution. Minimum toolchain:
0.2.1-dev.0 with the current owned-Result fixes. No new runtime ABI.

`schema_validate(schema: JsonValue, value: JsonValue)` returns
`Result<SchemaValidation, i64>`. Keep both source JsonDocs alive during the call.
A rejected definition or instance is a successful Result with `valid=false`,
`stage` (`schema` or `instance`), JSON Pointer `pointer`, and `keyword`.
The numeric Result error channel is for runtime/allocation/invalid API bounds.

Supported keywords:

- `type`: one of object, array, string, integer, number, boolean, null.
- `properties`, `required`, boolean `additionalProperties`, single-schema `items`.
- Scalar `enum`; exact i64 values never compare through f64.
- `minLength`/`maxLength` (Unicode scalar count), `minItems`/`maxItems`.
- Integral `minimum`/`maximum`, applied to i64 instances only.
- String `title` and `description` annotations.

This is a deliberately explicit subset, not full JSON Schema. Fractional numeric
bounds, schema-valued additionalProperties, object/array enum members, type
unions, refs, patterns, combinators, defaults and all other keywords are rejected.
All schema branches are checked even if the corresponding instance field is
absent. NUL object keys are rejected before C-string-based field lookup.
Duplicate-name raw JSON detection is the parser/caller's responsibility; this
API accepts parsed values and does not claim to recover overwritten wire keys.

Depth is at most 32; schema plus instance have 65,536 visits by default.
`schema_validate_with_limit(schema,value,visits)` accepts 1..1,048,576 visits.
Capacity exhaustion returns an explicit rejection; it never validates a prefix.
JsonDoc byte/node admission is independent and remains the caller's choice.

Use an exact version from an offline registry or a local path during development:

```toml
[dependencies]
sgschema = { path = "../sgschema" }
```

`tests/schema.sg` exercises nested contracts, JSON Pointer escaping, exact large
integers, Unicode lengths, unsupported schemas, extra/missing fields, bounds and
capacity. Platform claims require executing the tests on each host.
