# sgpm Quickstart

`sgpm` is Sengoo's project-level package manager MVP. It is intentionally
offline-first: the current implementation supports local `path` dependencies
only. Registry dependencies, lockfiles, workspaces, and publishing are deferred
to follow-up OpenSpec changes.

## Create a Package

```bash
sgpm new hello
cd hello
sgpm check
sgpm build
sgpm run
```

`sgpm new hello` creates:

```text
hello/
|-- Sengoo.toml
|-- src/
|   `-- main.sg
|-- tests/
`-- .gitignore
```

## Manifest

The MVP manifest format is `Sengoo.toml`.

```toml
[package]
name = "hello"
version = "0.1.0"
edition = "2026"

[bin]
path = "src/main.sg"

[dependencies]
math_utils = { path = "../math_utils" }
```

Supported fields:

- `[package].name`: package name.
- `[package].version`: semantic version, validated with `semver`.
- `[package].edition`: currently `2026`.
- `[bin].name`: optional executable output name.
- `[bin].path`: package entry file, default `src/main.sg`.
- `[lib].path`: library entry file, default `src/lib.sg`.
- `[dependencies]`: path-only dependencies, written as `{ path = "..." }`.

Unsupported dependency forms fail fast:

```toml
[dependencies]
foo = "1.0.0"                    # rejected: registry not implemented
bar = { version = "1.0.0" }      # rejected: registry not implemented
baz = { git = "https://..." }    # rejected: git not implemented
```

## Common Commands

```bash
# Resolve and print the path-dependency graph.
sgpm tree

# Type-check the package graph in topological order.
sgpm check

# Build the package graph into target/debug.
sgpm build

# Build the package graph into target/release with -O2.
sgpm build --release

# Build and execute the root package binary.
sgpm run -- arg1 arg2

# Run every .sg file under tests/ for each package in the graph.
sgpm test

# Format src/**/*.sg using sgfmt.
sgpm fmt

# Check formatting without writing.
sgpm fmt --check

# Remove the root package target/ directory.
sgpm clean
```

All package graph commands accept:

- `--manifest-path PATH`: path to `Sengoo.toml`, or a package directory.
- `-v` / `--verbose`: print delegated `sgc` / `sgfmt` commands.
- `--release`: for `build`, `run`, `check`, and `test` command groups where
  applicable.

## Tool Discovery

`sgpm` locates `sgc` and `sgfmt` in this order:

1. Environment overrides: `SGPM_SGC`, `SGPM_SGFMT`.
2. `PATH`.
3. Workspace `target/debug` and `target/release`.

This keeps tests and local development deterministic while allowing installed
toolchains to work without extra configuration.
