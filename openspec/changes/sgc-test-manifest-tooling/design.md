## Scope

This is the P1 toolchain lane. It can be implemented independently from
language/runtime work except where tests use new syntax. It should first record
the existing sgpm/sgfmt/sglsp/sgc-doc baseline, then stabilize and extend it.

## Existing Baseline

The repository already includes:

- `tools/sgpm` with manifest parsing, resolver, lockfile, package runner,
  cache, scaffold, workspace, and integration tests.
- `docs/sgpm-quickstart.md` describing registry/cache/lockfile workflows.
- `tools/sgfmt` and `tools/sglsp`.
- `sgc doc` and `sgc bench` command surfaces.

Implementation agents should not duplicate these systems. They should harden,
document, and connect them.

## sgc test Shape

Accepted direct command shape:

```text
sgc test [PATH]
         [--filter TEXT]
         [--exact NAME]
         [--format text|json]
         [--nocapture]
         [--release]
         [--manifest-path PATH]
         [--locked]
```

`PATH` defaults to the current package or current directory when no manifest is
present. Test discovery includes `tests/**/*.sg` and any manifest-declared test
targets. Tests run shell-free through the same native execution policy as
`sgc run`.

## Manifest And Registry Policy

Existing `Sengoo.toml` and `Sengoo.lock` remain the project model. Registry work
means stabilizing the local/remote registry metadata protocol, cache layout,
lockfile source ids, and stale-lock diagnostics. A public registry service is
not required for this change.

## Done Definition

This lane is done when a project can run `sgc test` directly, `sgpm test`
delegates or behaves equivalently, manifest/lock/registry/cache behavior is
schema-tested, and formatter/docs/LSP/bench output is deterministic enough for
CI.
