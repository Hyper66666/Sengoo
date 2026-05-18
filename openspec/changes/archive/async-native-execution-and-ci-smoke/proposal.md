## Why

Sengoo now parses and type-checks `async` and `await`, and the native async path is already exercised by tests. The remaining work for this change is to document the shipped execution boundary and the benchmark smoke preset where developers already look for runtime and benchmark guidance.

## What Changes

- Document the native async execution path so `sgc run <file>` can run a Sengoo source file whose entrypoint is `async def main()`.
- Document the current async execution surface and the remaining limits in the developer-facing docs and change artifacts.
- Document a deterministic CI smoke mode for `bench/llm_scheduler_bench.py` with fixed tiny workload parameters, report generation, and checksum parity validation between Python and Sengoo runners.
- Keep the deferred follow-up goals explicit so later async expansion has a stable starting point.

## Capabilities

### New Capabilities
- `async-native-execution`: Direct native execution of Sengoo async programs through `sgc run`, including compiler/runtime integration and explicit phase-1 async scope.
- `llm-scheduler-ci-smoke`: Deterministic tiny smoke coverage for `bench/llm_scheduler_bench.py`, including fixed presets, report output, and parity checks.

### Modified Capabilities
- `<none>`: This change introduces narrower execution-focused capabilities instead of changing the broader roadmap capability definition.

## Impact

- Affected code: `compiler/`, `runtime/`, `tools/sgc`, `bench/llm_scheduler_bench.py`, and related tests/examples.
- Affected interfaces: `sgc run`, native linker/runtime integration, and benchmark CLI usage for smoke validation.
- Affected architecture: async lowering/codegen, native runtime bridging, executable entrypoint generation, and benchmark verification workflow.
