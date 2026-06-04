## 1. Baseline And Gating

- [ ] 1.1 Validate this change with `openspec validate stdlib-breadth-mainstream --strict`.
- [ ] 1.2 Inventory current stdlib modules, examples, docs, LSP symbols, and runtime bridge files touched by this lane.
- [ ] 1.3 Inventory existing `std::net` and HTTP runtime APIs and mark each as stable, compatibility-only, or internal.
- [ ] 1.4 Confirm whether `owned-string-text` is landed; if not, keep public text-producing APIs on managed `Buffer` outputs.

## 2. Assertion Migration

- [ ] 2.1 Add `std::assert` as the primary assertion module.
- [ ] 2.2 Keep `std::error` assertion helpers working and document the compatibility period.
- [ ] 2.3 Update examples and README to use `std::assert` for new assertion examples.

## 3. Text, Formatting, Regex, Log, And Time

- [ ] 3.1 Add `std::string` and `std::fmt` APIs with byte/Unicode behavior documented.
- [ ] 3.2 Add deterministic regex compile/match/capture/replace APIs with pattern/input/resource limits.
- [ ] 3.3 Add `std::log` levels and sinks with deterministic output tests.
- [ ] 3.4 Add `std::time` format/parse/duration helpers with timezone and invalid-input tests.

## 4. Filesystem, Config, Hash, Encoding, Compression

- [ ] 4.1 Add glob helpers with deterministic ordering and symlink policy.
- [ ] 4.2 Add recursive copy/delete policy helpers with explicit safety flags.
- [ ] 4.3 Add file-watch support detection and portable unsupported-status behavior.
- [ ] 4.4 Add TOML/INI config helpers with parse/write limits and diagnostics.
- [ ] 4.5 Add SHA-style hash, base64, hex, gzip/zlib-class helpers with Buffer/String variants where accepted.

## 5. Network And HTTP

- [ ] 5.1 Stabilize existing `std::net`/HTTP API names and document compatibility-only names.
- [ ] 5.2 Add client helpers for method, URL, headers, timeout, request body, response status, and body copy.
- [ ] 5.3 Add server helpers only for documented supported platforms and unsupported-status paths elsewhere.
- [ ] 5.4 Add security tests for header/body limits, timeout, unsupported TLS, bind failure, and invalid handles.

## 6. Toolchain Wiring

- [ ] 6.1 Wire each module through `sgc` stdlib import expansion.
- [ ] 6.2 Wire LSP stdlib signatures and examples for each module.
- [ ] 6.3 Update docs and example catalog.
- [ ] 6.4 Ensure runtime source bundle/linking includes all bridge files.

## 7. Verification

- [ ] 7.1 Run `cargo fmt --check`.
- [ ] 7.2 Run `cargo test -p sgc stdlib_ -- --nocapture`.
- [ ] 7.3 Run `cargo test -p sengoo-compiler stdlib_surface -- --nocapture`.
- [ ] 7.4 Run `cargo test -p sglsp stdlib -- --nocapture`.
- [ ] 7.5 Run `sgc check/build/run` for every new or modified stdlib example.

## Done Definition

- [ ] All accepted module groups have public wrappers, docs, examples, LSP signatures, and status-category error behavior.
- [ ] `std::error` remains assertion-compatible and runtime error taxonomy remains in `std::status`.
- [ ] Existing partial `std::net` behavior is either stabilized or documented as compatibility/internal.
- [ ] Resource limits and unsupported-platform behavior are tested.

## Archive Gate

- [ ] `openspec validate stdlib-breadth-mainstream --strict` passes.
- [ ] `openspec validate --all --strict` passes.
- [ ] All verification commands above pass or have documented, accepted platform skips.
