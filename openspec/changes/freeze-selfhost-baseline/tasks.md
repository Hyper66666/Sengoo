## Implementation

- [x] Define authoritative corpus groups and explicit experimental/host exclusions.
- [x] Implement reference/candidate one-warmup/ten-sample runner with AB/BA ordering and exact source/tool identity checks.
- [x] Record OS root lifetime peak memory plus bounded process-tree observations and raw outputs.
- [x] Implement actual embedded LSP stdio lifecycle and expected diagnostic assertions.
- [x] Separate provisional measurement from release qualification and preserve raw reports when attaching later evidence.

## Verification

- [x] Complete development harness trial: eight workloads, one warmup and ten measured samples each; real root RSS and LSP lifecycle checked (`target/selfhost/m0/evidence.json`, Windows x64, 2026-11-09). Concurrent Cargo load makes this trial provisional and ineligible for promotion.
- [x] Complete ten measured reference samples for all eight daily workloads (`target/selfhost/m0-frozen/evidence.json`, quiet-host, 101 raw records, no source/tool changes).
- [x] Bind successful independent release/source qualification to the measured tool bytes (`target/selfhost/reference-qualification/evidence.json`: 1178 compiler + 11 formatter + 64 package + 192 LSP tests, zero skipped/filtered, O2 core native resources pass).
- [x] Run corpus identity, negative qualification/input-change/development-promotion and measurement summary checks (`target/selfhost/m0-negative-evidence/evidence.json`, seven checks passed).
- [x] Run `openspec validate freeze-selfhost-baseline --strict`.
- [x] Run all strict specification checks after integration (97 passed, 0 failed on Windows x64, 2026-11-09).
- [x] Register M0 evidence with the umbrella; leave full compatibility, final candidate comparison and installed/cross-host gates open.
- [x] Prove exact incremental-result checking with a real stale revision-7 executable rejected as revision 8, followed by a rebuilt executable producing 8 (`target/selfhost/m0-stale-negative-final/evidence.json`).

Full reference revision `c70400a9cae30edc666dcccfdfc867155875853b`,
Windows x64 DESKTOP-FUAQMFB, 2026-11-09. Frozen measured report SHA256:
`7d25ae8661ff7d5b3117988d6ff45658f421504f51239640583c65284d73bfc5`.
Independent release qualification SHA256:
`0e7be2e6e305288cfe89afb93bee244a50f9122471f4cca8e0630693c0a03ee9`.
741 measured source/corpus inputs, eight ten-sample workloads. Earlier loaded
and weak-edit trial reports remain preserved and do not replace this run.
