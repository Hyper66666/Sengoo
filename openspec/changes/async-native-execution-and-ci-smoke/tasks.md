## 1. Async Frontend Boundaries

- [x] 1.1 Preserve `async` / `await` semantics through HIR and stop silently erasing phase-1 async constructs during lowering.
- [x] 1.2 Add explicit compiler diagnostics for unsupported async constructs such as non-async await operands and still-unimplemented concurrency/runtime forms.
- [x] 1.3 Add compiler tests that cover valid async function and async-block await usage plus invalid non-async await forms.

## 2. Async Lowering and Code Generation

- [x] 2.1 Introduce minimal async state-machine lowering for `async def` functions with start/poll/result handling.
- [x] 2.2 Generate a synchronous wrapper `main` when the user entrypoint is async so native executables still have a standard process entrypoint.
- [x] 2.3 Add codegen tests that assert async wrapper and runtime bridge symbols are emitted correctly.

## 3. Native Runtime Bridge

- [x] 3.1 Export scheduler bridge functions from `sengoo-runtime` as a native-callable async execution surface.
- [x] 3.2 Update `tools/sgc` native build and link flow to include the Rust async runtime bridge alongside `tools/stdlib/runtime.c`.
- [x] 3.3 Add end-to-end `sgc run` coverage for a Sengoo async example that awaits another async function and completes successfully.

## 4. LLM Scheduler Smoke Preset

- [x] 4.1 Add a deterministic `ci-smoke` preset to `bench/llm_scheduler_bench.py` with fixed tiny workload parameters.
- [x] 4.2 Preserve checksum parity validation and standard JSON report output when the smoke preset is used.
- [x] 4.3 Add smoke-oriented script or integration tests that verify preset expansion, successful report generation, and checksum mismatch failure behavior.

## 5. Documentation and Follow-up Capture

- [x] 5.1 Document the phase-1 async execution boundary and smoke preset usage where developers already look for benchmark/runtime guidance.
- [x] 5.2 Keep deferred async expansion goals explicitly recorded in the change artifacts so later follow-up changes have a stable starting point.
