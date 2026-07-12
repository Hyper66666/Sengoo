# workspace-audit

`workspace-audit` is the flagship realworld reference application for the
language-maturity roadmap. It recursively scans a bounded workspace tree,
reads optional JSON config, classifies source/test/manifest paths, uses a
string-keyed generic map for report counters, and writes structured JSON plus a
formatted text report. Application source contains no manual
`.free()`, `.drop()`, or `.close()` resource calls. Report scoring runs as four
joined worker jobs over the compiler-checked `ArcMutex<i64>` shared-state
surface and is cross-checked against the `AuditCheck` trait result.

Run it through the package loop:

```bash
cargo run -p sgpm -- update --manifest-path examples/realworld/workspace-audit/Sengoo.toml
cargo run -p sgpm -- check --locked --manifest-path examples/realworld/workspace-audit/Sengoo.toml
cargo run -p sgpm -- test --locked --manifest-path examples/realworld/workspace-audit/Sengoo.toml
cargo run -p sgpm -- fmt --check --locked --manifest-path examples/realworld/workspace-audit/Sengoo.toml
cargo run -p sgpm -- doc --locked --manifest-path examples/realworld/workspace-audit/Sengoo.toml
cargo run -p sgpm -- build --locked --manifest-path examples/realworld/workspace-audit/Sengoo.toml
```

Directory walking remains cooperative, while independent score dimensions use
the current scalar `ArcMutex<i64>` transition surface. Fully generic
`Arc<Mutex<T>>` remains tracked by the concurrency OpenSpec change.
