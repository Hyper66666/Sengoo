## 1. Baseline And Gating

- [x] 1.1 Validate this change with `openspec validate stdlib-breadth-mainstream --strict`.
- [x] 1.2 Inventory current stdlib modules, examples, docs, LSP symbols, and runtime bridge files touched by this lane.
- [x] 1.3 Inventory existing `std::net` and HTTP runtime APIs and mark each as stable, compatibility-only, or internal.
- [x] 1.4 Confirm whether `owned-string-text` is landed; if not, keep public text-producing APIs on managed `Buffer` outputs.

## 2. Assertion Migration

- [x] 2.1 Add `std::assert` as the primary assertion module.
- [x] 2.2 Keep `std::error` assertion helpers working and document the compatibility period.
- [x] 2.3 Update examples and README to use `std::assert` for new assertion examples.

## 3. Text, Formatting, Regex, Log, And Time

- [x] 3.1 Add `std::string` and `std::fmt` APIs with byte/Unicode behavior documented.
- [x] 3.2 Add deterministic regex compile/match/capture/replace APIs with pattern/input/resource limits.
- [x] 3.3 Add `std::log` levels and sinks with deterministic output tests.
- [x] 3.4 Add `std::time` format/parse/duration helpers with timezone and invalid-input tests.

## 4. Filesystem, Config, Hash, Encoding, Compression

- [x] 4.1 Add glob helpers with deterministic ordering and symlink policy.
- [x] 4.2 Add recursive copy/delete policy helpers with explicit safety flags.
- [x] 4.3 Add file-watch support detection and portable unsupported-status behavior.
- [x] 4.4 Add TOML/INI config helpers with parse/write limits and diagnostics.
- [x] 4.5 Add SHA-style hash, base64, hex, gzip/zlib-class helpers with Buffer/String variants where accepted.

## 5. Network And HTTP

- [x] 5.1 Stabilize existing `std::net`/HTTP API names and document compatibility-only names.
- [x] 5.2 Add client helpers for method, URL, headers, timeout, request body, response status, and body copy.
- [x] 5.3 Add server helpers only for documented supported platforms and unsupported-status paths elsewhere.
- [x] 5.4 Add security tests for header/body limits, timeout, unsupported TLS, bind failure, and invalid handles.

## 6. Toolchain Wiring

- [x] 6.1 Wire each module through `sgc` stdlib import expansion.
- [x] 6.2 Wire LSP stdlib signatures and examples for each module.
- [x] 6.3 Update docs and example catalog.
- [x] 6.4 Ensure runtime source bundle/linking includes all bridge files.

## 7. Verification

- [x] 7.1 Run `cargo fmt --check`.
- [x] 7.2 Run `cargo test -p sgc stdlib_ -- --nocapture`.
- [x] 7.3 Run `cargo test -p sengoo-compiler stdlib_surface -- --nocapture`.
- [x] 7.4 Run `cargo test -p sglsp stdlib -- --nocapture`.
- [x] 7.5 Run `sgc check/build/run` for every new or modified stdlib example.

## Done Definition

- [x] All accepted module groups have public wrappers, docs, examples, LSP signatures, and status-category error behavior.
- [x] `std::error` remains assertion-compatible and runtime error taxonomy remains in `std::status`.
- [x] Existing partial `std::net` behavior is either stabilized or documented as compatibility/internal.
- [x] Resource limits and unsupported-platform behavior are tested.

## Archive Gate

- [x] `openspec validate stdlib-breadth-mainstream --strict` passes.
- [x] `openspec validate --all --strict` passes.
- [x] All verification commands above pass or have documented, accepted platform skips.
