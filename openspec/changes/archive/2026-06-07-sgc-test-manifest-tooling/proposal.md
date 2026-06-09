## Why

Sengoo already has substantial tooling: `sgpm`, `Sengoo.toml`, lockfiles,
registry/cache support, `sgfmt`, `sgc doc`, `sglsp`, and benchmarking support
exist in the repository. The mainstream gap is stability and integration, not
inventing the whole ecosystem from zero. The missing center is a direct
`sgc test` surface that sgpm and CI can align around.

## Proposal

- Stabilize the existing `Sengoo.toml`, `Sengoo.lock`, sgpm resolver,
  registry, and cache behavior with protocol/version diagnostics.
- Add direct `sgc test` discovery, filtering, stdout/stderr capture, exit
  status reporting, and optional JSON output.
- Align existing `sgpm test` with `sgc test` once the direct command exists.
- Harden existing `sgfmt`, `sgc doc`, `sglsp`, benchmark/profiling, and
  project-template surfaces with deterministic output and CI-ready checks.

## Impact

- Updates `tools/sgc`, `tools/sgpm`, `tools/sgfmt`, `tools/sglsp`, docs,
  examples, and integration tests.
- Existing sgpm manifests and lockfiles remain supported unless explicitly
  rejected by stable diagnostics for malformed or stale metadata.
- Registry/cache work is protocol stabilization, not a new public-registry
  launch.

## Non-Goals

- No mandatory public package registry.
- No dependency on network access for local-package workflows.
- No package script execution that bypasses explicit command allowlists.
- No unstable machine-readable output without schema tests.
