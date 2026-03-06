## Why

Sengoo currently parses and type-checks `async` and `await`, but `sgc run example.sg` still cannot execute a Sengoo async entrypoint end to end. The benchmark side has the opposite problem: `bench/llm_scheduler_bench.py` works as an exploratory tool, but it lacks a fixed tiny smoke mode that can validate the path quickly and deterministically before broader performance work continues.

## What Changes

- Define a direct native async execution path so `sgc run <file>` can compile and run a Sengoo source file whose entrypoint is `async def main()`.
- Define the minimal phase-1 async execution contract: preserve `async`/`await` semantics through lowering and codegen, generate a synchronous wrapper entrypoint, and drive async execution through a native runtime scheduler bridge.
- Define explicit phase-1 async scope and failure behavior: support `async def` and `await async_fn(...)`, and fail clearly for unsupported constructs such as `async` blocks, arbitrary await operands, and advanced concurrency primitives.
- Define a deterministic CI smoke mode for `bench/llm_scheduler_bench.py` with fixed tiny workload parameters, report generation, and checksum parity validation between Python and Sengoo runners.
- Document the deferred follow-up goals for later async expansion, including `async` blocks, spawn/join-style concurrency, timer or IO-driven wakeups, and multithreaded scheduling.

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
