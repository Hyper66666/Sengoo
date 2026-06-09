## Cross-compile

- `sgc build --target <triple>` with explicit sysroot/SDK env vars documented.
- v1 targets: `x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu` on reference hosts.

## Registry maturity

- This change is the active canonical owner for `sgpm-package-graph`.
  `sgpm-alias-multiversion` remains historical implementation evidence only.
- Copied-forward package graph requirements: dependency `package = "actual_name"`
  aliases, multiple resolved versions of the same package name, lockfile v2
  identities, v1 compatibility/migration, and metadata dependency edges.
- `sgpm metadata --format json` adds `yanked`, `features` per package version.
- `sgpm publish` validates feature manifest schema; yanked versions rejected on fresh resolve.

## LSP depth

- Go-to-definition across path/git dependency sources in workspace graphs.
- Signature help for stdlib imports already present; extend to dependency modules.

## Timings export

- `sgc build --timings-json <path>` writes schema-version-1 phase breakdown compatible
  with `frontend-compile-perf` phases.

## Non-goals

- Hosting a public registry with package search UI.
- Full rust-analyzer-grade refactorings.
