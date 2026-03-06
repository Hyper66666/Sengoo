## Context

Sengoo already recognizes `async def` and `await` in the parser and type checker, but the execution path is incomplete. `await` is erased too early during lowering, `async` blocks are not represented as executable coroutine semantics, and `sgc run` still follows the existing native pipeline that expects a synchronous entrypoint.

The runtime side is also split today. The repository already has a Rust `CoroutineScheduler` in `runtime/src/async_runtime.rs`, but the native `sgc run` path links `tools/stdlib/runtime.c`, not the Rust runtime crate. That means the compiler can currently accept async syntax without having a stable native execution bridge to drive it.

`bench/llm_scheduler_bench.py` has the opposite shape: it already exercises a useful scheduler workload, but it is tuned for exploratory benchmarking rather than a deterministic tiny smoke path. A small, fixed preset is needed so the benchmark can validate correctness in seconds before broader performance work or CI automation is added.

## Goals / Non-Goals

**Goals:**
- Make `sgc run <file>` execute a Sengoo source file whose entrypoint is `async def main()`.
- Preserve minimal async semantics through lowering and code generation instead of erasing them before execution.
- Keep the existing native compile-and-run product shape: compile to IR, link a native binary, then execute it.
- Reuse the Rust scheduler model already present in `sengoo-runtime` instead of reimplementing async scheduling in `runtime.c`.
- Add a deterministic tiny smoke mode to `bench/llm_scheduler_bench.py` that validates parity between Python and Sengoo runners and emits the normal benchmark report format.
- Capture the deferred follow-up goals explicitly so phase-1 async stays intentionally bounded.

**Non-Goals:**
- Full language-level async coverage in this phase.
- `async` blocks, arbitrary await operands, spawn/join/select, timer wakeups, network or file IO wakeups, or multithreaded scheduling.
- A general `Future` trait model or user-defined awaitables.
- Replacing `tools/stdlib/runtime.c` wholesale with the Rust runtime crate.
- Adding or changing `.github/workflows/*` in this phase.
- Adding benchmark performance thresholds; smoke is for correctness and survivability, not gating throughput yet.

## Decisions

1. Keep phase-1 async on the normal native `sgc run` pipeline.
- The compiled program should still go through parse -> type check -> lowering -> LLVM IR -> native link -> execute.
- This avoids a split-brain product where async programs secretly use an interpreter or side execution path.
- Alternative considered: detect async sources in `sgc run` and execute them through a separate interpreter-like bridge. Rejected because it would create a second execution model with different behavior, observability, and failure modes.

2. Limit phase-1 async semantics to `async def` plus `await async_fn(...)`.
- This is the smallest execution contract that still satisfies the stated user goal: `sgc run example.sg` should directly run Sengoo async source.
- Unsupported constructs must fail explicitly instead of silently degrading into synchronous behavior.
- Alternative considered: support `async` blocks and arbitrary await operands immediately. Rejected because the current compiler does not preserve those semantics deeply enough, and broadening scope would delay a runnable end-to-end path.

3. Lower async functions into explicit start/poll/result state-machine entrypoints.
- Each compiled async function should have a concrete runtime-facing ABI that can be driven by the scheduler bridge.
- A synthetic synchronous wrapper `main` should be generated when the user entrypoint is async so the executable still has a native process entrypoint.
- Alternative considered: compile async functions as opaque heap futures with implicit protocol. Rejected because the current codegen/runtime surfaces are simpler to extend through explicit exported entrypoints and handles.

4. Expose the Rust scheduler through a native static-library bridge.
- `sengoo-runtime` should provide `extern "C"` bridge functions that create schedulers, spawn root tasks, poll work, and extract completed results.
- `tools/sgc` native linking should include that runtime artifact alongside the existing `tools/stdlib/runtime.c` object instead of forcing a scheduler rewrite in C.
- Alternative considered: reimplement scheduling in `runtime.c`. Rejected because the Rust scheduler already exists, and duplicating it in C would create maintenance drift immediately.

5. Represent unsupported async constructs as explicit compile-time diagnostics.
- Using `await` on a non-async operand, using `async { ... }`, or invoking future concurrency features before they exist must produce actionable diagnostics that state the phase-1 limit.
- Alternative considered: keep parsing/type checking permissive and fail later during codegen or linking. Rejected because users would get weaker diagnostics after more work has already happened.

6. Add benchmark smoke through a fixed preset instead of a separate script.
- `bench/llm_scheduler_bench.py` should keep one code path and one report format.
- A preset such as `--preset ci-smoke` should override workload parameters to a fixed, tiny configuration that completes quickly and still exercises both predefined scenarios.
- Alternative considered: add a second standalone smoke script. Rejected because it would duplicate runner construction, report generation, and parity logic.

7. Record deferred async goals as follow-up work, not phase-1 requirements.
- The next phases should cover `async` blocks, spawn/join-style concurrency, event-driven wakeups, multithreaded scheduling, and a more general awaitable model.
- Keeping them documented here prevents accidental scope creep while still preserving the roadmap.

## Risks / Trade-offs

- [Phase-1 async is intentionally narrow] -> Mitigation: reject unsupported forms with explicit diagnostics and describe the boundary in OpenSpec artifacts.
- [Async lowering touches multiple compiler stages] -> Mitigation: drive implementation with test-first coverage at parser/typeck/lowering/codegen/`sgc run` layers.
- [Native link portability becomes more complex] -> Mitigation: keep the existing `runtime.c` path intact and add the Rust async bridge as an additive native artifact.
- [Rust runtime staticlib integration can fail on some toolchains] -> Mitigation: add link-path verification in `tools/sgc` tests and keep rollback path to explicit compile-time rejection of async entrypoints.
- [Smoke preset may drift from the full benchmark path] -> Mitigation: implement smoke as parameter selection inside the same script and preserve the same report schema.

## Migration Plan

1. Tighten compiler diagnostics so unsupported async constructs fail clearly instead of appearing partially supported.
2. Preserve async semantics through lowering and introduce minimal async state-machine code generation.
3. Export the scheduler bridge from `sengoo-runtime` and update native linking in `tools/sgc`.
4. Generate the synchronous wrapper entrypoint for async `main` and add `sgc run` end-to-end coverage.
5. Add the benchmark smoke preset and keep report output compatible with existing consumers.
6. Leave advanced async features for follow-up changes after the native minimal path is stable.

Rollback strategy:
- If the runtime bridge or native linking proves unstable, disable async native execution behind an explicit compile-time rejection rather than leaving partially working behavior in place.
- Keep the benchmark smoke preset additive so it can be removed without affecting the existing benchmark interface.

## Open Questions

- Should phase-1 async entrypoints support only `i64`/`unit` returns, or should the result bridge immediately cover the broader set of codegen-supported scalar types?
- Should the long-term native runtime strategy continue to mix `runtime.c` and Rust runtime artifacts, or should those surfaces eventually converge into one packaging model?
- Once the smoke preset is stable, should the project expose it through a dedicated helper command or keep it as a raw benchmark-script preset?

## Deferred Follow-ups

- `async { ... }` block execution semantics.
- Spawn/join/select-style concurrency constructs.
- Timer- and IO-driven wakeups instead of pure cooperative polling.
- Multithreaded scheduler policies.
- General awaitable or trait-based async abstraction beyond compiler-owned async functions.
- CI workflow wiring once the smoke path is stable enough to gate automatically.
