## Scope

This is the P1 standard-library breadth lane. It should be implemented as
module-sized slices, each with its own examples and tests, but archived as one
stdlib breadth change only after all accepted modules meet the wiring and
resource-limit gates.

## Dependency Model

- `std::status` categories from the existing usability wave remain the error
  taxonomy.
- Managed `Buffer` outputs remain valid for every text-producing helper.
- Owned `String` outputs are allowed only after `owned-string-text` lands.
- `std::error` keeps its assertion-helper role; runtime status names stay in
  `std::status`.

## Module Boundaries

Accepted module groups:

- `std::assert`: assertions, with `std::error` compatibility wrappers.
- `std::string` and `std::fmt`: text construction, split/join/trim/replace,
  formatting of primitive values, and explicit byte/Unicode boundaries.
- `std::regex`: compile, match, captures, replace, limits, and diagnostics.
- `std::log`: levels, stderr sink, optional file sink where portable, and
  deterministic test output.
- `std::time`: monotonic duration helpers and civil date/time format/parse
  helpers with explicit timezone support rules.
- `std::fs` extensions: glob, recursive delete/copy policy helpers, and file
  watch support detection.
- `std::config`: TOML and INI parse/write helpers with documented input limits.
- `std::hash`, `std::encoding`, `std::compress`: SHA-style hashes, base64, hex,
  gzip/zlib-class workflows where supported.
- `std::net` and `std::http`: stabilize the existing network/http baseline,
  then expose documented client/server helpers with timeout, headers, status,
  and body handling.

## Network Policy

The repository already contains a partial network/runtime baseline. This change
must first inventory and document that baseline, then decide which names are
stable. New APIs must report unsupported TLS, bind, DNS, or socket operations
through stable status categories instead of failing through unresolved runtime
symbols.

## Done Definition

Every accepted module group has source wrappers, runtime bridge behavior where
needed, docs, examples, LSP symbols, status errors, and negative tests for
invalid input/resource limits.
