# sgpm Quickstart

`sgpm` is Sengoo's project-level package manager MVP. The current
implementation supports local `path` dependencies, git dependencies resolved
through a root-package cache, local file registries with semantic-version
dependency constraints, and remote registry dependency fetches through a
package cache. `sgpm update` writes a `Sengoo.lock` snapshot for the resolved
package graph, including resolved commits for git dependencies and selected
registry versions. `sgpm publish --registry <name>` can publish the selected
package into a configured local file registry, and `sgpm publish` can upload to
`[registries.default].url`. Workspace manifests can select member packages with
`--package`, run supported package graph commands across all members with
`--workspace`, inherit workspace-level registries, and write one root
workspace lockfile for all members. Local publish dry-runs can also create
package artifacts for inspection. `sgpm doc` generates package API docs through
`sgc doc`, preferring `[lib]` entries when present.

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

To initialize an existing directory instead, run:

```bash
mkdir hello
cd hello
sgpm init
```

`sgpm init [name]` defaults the package name to the current directory name.
Use `--path PATH` to initialize another directory. Existing unrelated files are
preserved, and `sgpm` refuses to overwrite existing scaffold files.

Pass `--lib` to `sgpm new` or `sgpm init` to create a library package with
`[lib] path = "src/lib.sg"` instead of a binary entry point:

```bash
sgpm new math_utils --lib
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

- `[package].name`: package name. Package, dependency, and optional binary
  names may contain lowercase ASCII letters, digits, `_`, and `-`.
- `[package].version`: semantic version, validated with `semver`.
- `[package].edition`: currently `2026`.
- `[bin].name`: optional executable output name.
- `[bin].path`: package entry file, default `src/main.sg`.
- `[lib].path`: library entry file, default `src/lib.sg`.
- Target entry paths must be relative files within the package root. Commands
  fail during dependency resolution when a declared entry is missing or
  escapes the package directory, before an unusable package can be published.
- `[registries.<name>].path`: local file registry root. Registry paths are
  resolved relative to the package manifest, or relative to the workspace root
  when declared in a workspace manifest.
- Registry names must start and end with a lowercase ASCII letter or digit;
  internal characters may also use `_`, `-`, and `.`. The same rule applies to
  dependency `registry = "..."` selectors.
- `[registries.<name>].url`: remote registry base URL for package downloads
  and uploads. A registry must specify exactly one of `path` or `url`.
- `[registries.<name>].token_env`: optional environment variable containing a
  bearer token for remote registry requests. It is valid only with `url`.
- `[workspace].members`: workspace member packages. Literal paths point at
  member directories or manifests; trailing `/*` expands direct child
  directories that contain `Sengoo.toml`. Workspace member package names must
  be unique.
- `[dependencies]`: local path dependencies written as `{ path = "..." }`, or
  git dependencies written as `{ git = "...", rev = "..." }`, or registry
  dependencies written as a version string for `[registries.default]` or as
  `{ version = "...", registry = "..." }`. `rev` is optional and may be a
  commit, branch, or tag accepted by `git checkout`. Dependency keys currently
  must match the target package's `[package].name`; renamed dependency aliases
  are not supported yet. A package name must also resolve to one manifest
  across the graph until renamed or multi-version dependencies are supported.

Supported dependency forms:

```toml
[dependencies]
local_utils = { path = "../local_utils" }
baz = { git = "../baz.git" }              # cached under target/sgpm/git
qux = { git = "../qux.git", rev = "a1b2c3d4" }
foo = { version = ">=1.0.0, <2.0.0", registry = "local" }
```

Library packages expose a Sengoo module with `[lib]`. Applications can import
the dependency name and use declarations from its library entry file:

```toml
# ../math_utils/Sengoo.toml
[package]
name = "math_utils"
version = "0.1.0"
edition = "2026"

[lib]
path = "src/lib.sg"
```

```sg
// src/main.sg
import math_utils;

def main() -> i64 {
    math_utils_answer()
}
```

`sgpm check`, `sgpm build`, and `sgpm test` expose resolved `[lib]` entries to
`sgc` as importable modules. During `sgpm build`, pure library packages are
type-checked before dependent binary packages are compiled. Running `sgpm run`
from a pure library package fails with a prompt to add `[bin]`. Library package
tests also receive their package's own `[lib]` module, so `tests/*.sg` files can
import the public package name. If a dependency declares both `[bin]` and
`[lib]`, imports use `[lib].path` while build and run keep using `[bin].path`.

`sgpm` local registries use this directory layout:

```text
registry/
`-- foo/
    |-- 1.0.0/
    |   `-- Sengoo.toml
    `-- 1.2.0/
        `-- Sengoo.toml
```

Declare the registry in the package manifest, or in a workspace root when the
package is selected from a workspace:

```toml
[registries.local]
path = "../registry"

[dependencies]
foo = { version = ">=1.0.0, <2.0.0", registry = "local" }
```

`sgpm` chooses the highest package version satisfying the semver requirement.
If separate dependencies require incompatible versions of the same registry
package, resolution fails with a version-conflict diagnostic that names the
package and both constraints.

Remote registries use the same dependency syntax. `sgpm` fetches
`GET <url>/api/v1/packages/<package>` for a JSON version index shaped like:

```json
{"versions":[{"version":"1.2.0","checksum":"<sha256-hex>"}]}
```

It selects the highest version satisfying the semver requirement, downloads
`GET <url>/api/v1/packages/<package>/<version>/download`, checks the optional
SHA-256 checksum, and unpacks the package into
`target/sgpm/registry/<registry>/<package>/<version>/`. Downloads unpack into
a sibling staging directory first; the cache version becomes visible only
after its manifest and source entries validate.

## Workspaces

Workspace roots use a `Sengoo.toml` with `[workspace]` instead of `[package]`.
Package graph commands can be run from the workspace root by selecting a member
with `--package <name>`. If the workspace has exactly one member, the member is
selected automatically. Use `--workspace` to run supported package graph
commands across every member in package-name order.

```toml
[workspace]
members = ["packages/*"]

[registries.local]
path = "../registry"
```

```bash
sgpm tree --manifest-path Sengoo.toml --package app
sgpm build --manifest-path Sengoo.toml --package app
sgpm update --manifest-path Sengoo.toml --package app
sgpm check --manifest-path Sengoo.toml --workspace
sgpm update --manifest-path Sengoo.toml --workspace
sgpm publish --registry local --manifest-path Sengoo.toml --package app
```

Workspace-level registries are inherited by selected members, and member
manifests can override a registry name locally. `sgpm update --workspace`
writes a single `Sengoo.lock` next to the workspace manifest, covering all
selected members and their dependency graphs. `run`, `publish`, `cache list`,
and `cache clean` remain single-package commands; use `--package <name>` for
those.

## Common Commands

```bash
# Print the installed sgpm version.
sgpm --version

# Initialize the current directory as a package.
sgpm init

# Create a reusable library package.
sgpm new math_utils --lib

# Resolve and print the package graph.
sgpm tree

# Print package graph metadata as JSON for tools and CI.
sgpm metadata --format json
sgpm metadata --format json --manifest-path Sengoo.toml --workspace

# Resolve the package graph and write Sengoo.lock.
sgpm update

# Verify Sengoo.lock is current without rewriting it.
sgpm update --check

# Reclone git dependency caches before writing Sengoo.lock.
sgpm update --refresh

# List local sgpm cache entries for this selected package.
sgpm cache list

# Remove cached git dependency checkouts.
sgpm cache clean --git

# Remove cached remote registry packages.
sgpm cache clean --registry

# Type-check the package graph in topological order.
sgpm check

# Build the package graph into target/debug.
sgpm build

# Build only when Sengoo.lock matches the current graph.
sgpm build --locked

# Build the package graph into target/release with -O2.
sgpm build --release

# Build and execute the selected package binary.
sgpm run -- arg1 arg2

# Run every .sg file under tests/ for each package in the graph.
sgpm test

# Run tests with the release profile (-O2).
sgpm test --release

# Format src/**/*.sg using sgfmt.
sgpm fmt

# Check formatting without writing.
sgpm fmt --check

# Generate API docs under target/doc.
sgpm doc

# Remove the selected package target/ directory.
sgpm clean

# Validate the selected package and write target/package/<name>-<version>.tar.gz.
sgpm publish --dry-run

# Publish the selected package into [registries.local].path.
sgpm publish --registry local
```

All package graph commands accept:

- `--manifest-path PATH`: path to `Sengoo.toml`, or a package directory.
- `--package NAME`: workspace member package to operate on when
  `--manifest-path` points at a workspace root.
- `--workspace`: operate on every workspace member for `build`, `check`, `test`,
  `fmt`, `doc`, `tree`, `metadata`, `clean`, and `update`. It cannot be combined with
  `--package`.
- `-v` / `--verbose`: print delegated `sgc` / `sgfmt` commands.
- `--release`: for `build`, `run`, `check`, and `test` command groups where
  applicable.
- `--locked`: for `build`, `check`, `run`, `test`, `fmt`, `doc`, `tree`, and
  `publish`; fail before invoking delegated `sgc` / `sgfmt` tools or packaging if
  `Sengoo.lock` is missing or stale.

`sgpm doc` runs `sgc doc` for each package in the selected graph. The default
output is `target/doc` for a single selected package, or package-named
subdirectories when dependencies are documented alongside the root package.
Pass `--output DIR` to choose a different output directory for a single
package selection.

`sgpm test` uses the debug profile by default (`sgc run -O 0`) and the release
profile with `--release` (`sgc run -O 2`). Source discovery errors fail
`sgpm test` and `sgpm fmt` instead of silently skipping unreadable `.sg` files.

Git dependencies are cloned into `target/sgpm/git/` under the selected package.
Remote registry dependencies are unpacked into `target/sgpm/registry/`.
Incomplete cached packages are validated and downloaded again automatically.
Failed downloads remove their staging directory instead of exposing a partial
cache version. Git clones and refreshes also complete in sibling staging paths;
an incomplete checkout is rebuilt automatically without exposing a partial
replacement. Staged git checkouts must pass manifest and target-entry
validation before replacing an existing cache, so a broken refresh preserves
the previous usable checkout.
Local
git repositories work fully offline; remote git URLs use the installed `git`
CLI and therefore require normal network access. `sgpm update --refresh`
removes the matching git dependency checkouts from that cache and reclones them
before writing `Sengoo.lock`; use it when a branch or local git source has moved
and you want the lockfile to record the new commit. `sgpm cache list` prints
existing git checkouts and remote registry package versions.
`sgpm cache clean --git` removes cached git checkouts, while
`sgpm cache clean --registry` removes downloaded registry packages. Neither
command touches normal build artifacts.

`sgpm update` writes `Sengoo.lock` next to the selected package manifest.
`sgpm update --workspace` writes one `Sengoo.lock` next to the workspace
manifest. The lockfile is a generated TOML snapshot containing the selected
root package name or workspace member names, dependency-first package entries,
package versions, local `path+...` sources, `git+...#<commit>` sources,
`registry+<registry>/<package>@<version>` sources, manifest paths, and direct
dependency names for the currently resolved package graph. Use
`sgpm update --check` or `sgpm update --workspace --check` in CI to fail when
the lockfile is missing or stale without rewriting it. Lockfile updates stage
the generated snapshot beside the final path before replacement, so a failed
write does not truncate the previous snapshot.

`sgpm publish --dry-run` packages the selected package only. It writes a `.tar.gz`
archive plus `.sha256` checksum under `target/package/`, includes project source
files such as `Sengoo.toml`, `src/`, and `tests/`, and excludes build artifacts
under `target/`. Dependency resolution validates target entry files before
creating the archive. Package file enumeration errors fail the publish instead
of silently omitting unreadable files. Pass `--output DIR` to write the
generated files somewhere else.

`sgpm publish --registry <name>` publishes the selected package into
`[registries.<name>].path` using the local registry layout
`<registry>/<package>/<version>/`. It copies source package files and excludes
`.git/`, `target/`, and registry output directories. Publishing refuses to
overwrite an existing package version, so increment `[package].version` before
republishing the same package. Local publishes stage files beside the final
version directory and rename the completed staging directory into place, so a
copy failure does not expose a partial version or block a retry.

For remote registries, declare a URL and optional token env var:

```toml
[registries.default]
url = "https://registry.example.invalid"
token_env = "SENGOO_REGISTRY_TOKEN"
```

`sgpm publish` uploads the generated `.tar.gz` artifact to
`POST <url>/api/v1/packages/<package>/<version>` with `x-sengoo-package`,
`x-sengoo-version`, and `x-sengoo-checksum` headers. When `token_env` is set,
the environment variable is sent as a bearer token. `sgpm publish --registry
<name>` uses the same remote path when the named registry is configured with
`url`.

## Tool Discovery

`sgpm` locates `sgc` and `sgfmt` in this order:

1. Environment overrides: `SGPM_SGC`, `SGPM_SGFMT`.
2. `PATH`.
3. Workspace `target/debug` and `target/release`.

This keeps tests and local development deterministic while allowing installed
toolchains to work without extra configuration.
