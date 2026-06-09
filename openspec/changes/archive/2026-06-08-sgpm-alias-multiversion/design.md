## Scope

Child change for `six-pillar-gap-closure` Pillar 3. Lockfile v2 field rules are
superseded by `openspec/changes/ecosystem-toolchain-maturity/specs/sgpm-package-graph/spec.md`.
This file remains as implementation evidence for the copied-forward delta.

## Manifest

```toml
[dependencies.my_alias]
package = "actual_name"
path = "../actual_name"
```

## Lockfile v2

- Header `version = 2`
- Package `id = "<name>@<version>+<source-key>"`
- Source keys canonicalized with `/` paths and stable git/registry encoding
- `[[dependency]]` records `alias`, `from`, `to`

## Migration

- v1 readable only for graphs without aliases or duplicate package versions
- `sgpm update` performs deterministic v1→v2 rewrite
- Locked commands never rewrite; incompatible expressibility fails with actionable
  `sgpm update` diagnostic

## Metadata

- `sgpm metadata --format json` exposes package identity and edge aliases separately
