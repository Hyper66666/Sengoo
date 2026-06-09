## Why

The accepted stdlib usability work covers paths, buffers, status errors, JSON,
collections, directory/file/process helpers, and runtime bundle splitting. The
next mainstream gap is breadth: everyday CLI and service code still needs
assertions, owned text and formatting, regex, logging, time, config, hashing,
encoding, compression, filesystem glob/watch policies, and a stabilized network
surface.

## Proposal

- Add `std::assert` as the primary assertion module while keeping existing
  `std::error` assertion helpers compatible.
- Add bounded `std::string`/`std::fmt`, `std::regex`, `std::log`, and
  `std::time` APIs aligned with the owned-string and status taxonomy changes.
- Add bounded filesystem/config/hash/encoding/compression helpers with explicit
  platform and resource limits.
- Stabilize the existing partial `std::net` and HTTP runtime baseline before
  expanding client/server APIs.
- Update canonical stdlib gating requirements so accepted data-format and
  network expansions are no longer contradicted by old deferred wording.

## Impact

- Updates source-level stdlib modules, runtime bridges, docs, examples, LSP
  stdlib signatures, and `sgc` stdlib import wiring.
- May depend on `owned-string-text` for String-returning APIs. Until that lane
  lands, APIs that produce text must use managed `Buffer` outputs.
- Keeps existing `std::error` assertion examples working during the migration.

## Non-Goals

- No implicit shell execution.
- No regex engine with unbounded catastrophic backtracking.
- No public network/TLS promise beyond explicitly documented host support.
- No silent Unicode normalization, locale, or timezone policy.
- No removal of `std::error` assertion helpers in this change.
