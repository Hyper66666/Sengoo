## Implementation

- [x] Reproduce and identify the synchronous ANSI launch boundary without weakening the M1 Unicode-path test.
- [x] Share strict UTF-16/environment helpers with synchronous run and pipeline launch.
- [x] Preserve quoting, capture, timeout and ABI1 result behavior; release conversion allocations on failure.
- [x] Add native and SG real-child Unicode/resource regressions and an evidence runner.

## Verification

- [x] Native probe passes Unicode executable/cwd/argv/env, empty/quoted arguments, pipeline, timeout, 30 resource rounds and 120 invalid inputs on Windows x64.
- [x] SG generated program passes 20 synchronous Unicode launches with exact production-runtime resource baselines.
- [x] Record source/tool identities, commands and output in `target/selfhost/process-utf8/evidence.json` with no changed sources (Windows x64, 2026-11-09; frozen baseline `cc70247ec5bd9531bea751a08288980da40ecc68` plus recorded runtime/test source hashes).
- [x] Run `openspec validate fix-windows-process-utf8-launch --strict`.
- [x] Re-run the complete M1 gate with frozen correction `fc1a686a2953adcbd14731cc6f93270cfb9cf34d`, updated source lock and original Unicode-path fixture (`target/selfhost/m1-run2/evidence.json`, 44 steps passed, no changed sources).
