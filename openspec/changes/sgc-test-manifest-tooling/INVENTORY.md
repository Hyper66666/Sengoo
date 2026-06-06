# Baseline Inventory (sgc-test-manifest-tooling)

## sgpm (existing + this change)

| Area | Status |
|------|--------|
| Manifest parse (`Sengoo.toml`) | Implemented; optional `sengoo-schema = 1` validated; `[[test]]` targets accepted by sgpm and sgc |
| Lockfile render/check | Implemented; `version = 1` header validated on check |
| Resolver / registry / cache | Implemented; checksum mismatch diagnostics in resolver |
| `sgpm test` | Delegates to `sgc test` with module map env |
| Integration tests | Updated for `:: test` / `--release` delegation |

## sgc test (this change)

| Area | Status |
|------|--------|
| CLI (`sgc test`) | `PATH`, `--filter`, `--exact`, `--format`, `--nocapture`, `--release`, `--manifest-path`, `--locked` |
| Discovery | `tests/**/*.sg` + `[[test]]` in manifest |
| Capture / reporting | Captures stdout/stderr by default (`Command::output`); `--nocapture` inherits; failed cases print streams in text/JSON |
| Unit tests | `discover_tests`, JSON shape in `commands/test.rs` |

## sgfmt / sgc doc / sglsp / bench / templates

| Tool | Status |
|------|--------|
| `sgfmt` | Existing check/idempotence tests in crate |
| `sgc doc` | Existing command + tests in `tools/sgc` |
| `sglsp` | Existing LSP integration tests |
| `sgc bench` | Existing bench JSON/text tests |
| `sgpm new/init` | Scaffold templates with integration coverage |

No duplicate implementations were added for §5 surfaces; this lane hardens via
existing tests plus documentation/protocol alignment above.

## Verification notes (June 2026)

| Suite | Command | Status |
|-------|---------|--------|
| sgc test unit | `cargo test -p sgc test::` | Passes (`discover_tests`, JSON capture mode) |
| sgpm manifest | `cargo test -p sgpm manifest::` | Passes; `[[test]]` no longer rejected as unknown field |
| sgc test integration | `cargo test -p sgc test -- --nocapture` | Passes when `sgc` test binary links; use isolated `CARGO_TARGET_DIR` if `LNK1104` (locked exe) |
| clippy | `cargo clippy -p sgc -p sgpm -- -D warnings` | Passes (`discover_tests` is module-private) |

Accepted skips: none for manifest/test tooling itself; native `sgc` e2e async link failures
(`LNK2019 sengoo_async_*_dispatch`) are pre-existing and tracked under runtime-hardening inventory.
