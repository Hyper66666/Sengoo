## ADDED Requirements

### Requirement: `sgc run` SHALL execute Sengoo async entrypoints natively
`sgc run <file>` SHALL compile and execute a Sengoo source file whose entrypoint is `async def main()` without requiring a separate interpreter mode or alternate CLI workflow.

#### Scenario: Running an async Sengoo program through `sgc run`
- **WHEN** a user runs `sgc run example.sg` and `example.sg` defines `async def main()` that awaits another Sengoo async function
- **THEN** the compiler and native runtime pipeline complete successfully and the observed program result matches the awaited async semantics

### Requirement: The compiler SHALL preserve phase-1 async semantics through lowering and code generation
The compiler SHALL keep enough async structure through lowering and code generation to suspend at `await async_fn(...)`, resume execution, and materialize the awaited result in subsequent statements.

#### Scenario: Awaited value survives suspend and resume
- **WHEN** an async function awaits another Sengoo async function and then uses the awaited value in later computation
- **THEN** the lowered and generated program resumes after the await point with the awaited value available for the remaining computation

### Requirement: The compiler SHALL reject unsupported phase-1 async constructs explicitly
The compiler SHALL fail with actionable diagnostics when source uses async constructs that are outside the phase-1 execution contract.

#### Scenario: Awaiting a non-async operand is rejected
- **WHEN** source uses `await` on a literal, a synchronous function result, or another operand that is not a Sengoo async call result
- **THEN** compilation fails with a diagnostic that explains phase-1 await requires an async call result

#### Scenario: Async blocks are rejected in phase 1
- **WHEN** source uses an `async { ... }` block
- **THEN** compilation fails with a diagnostic that states async blocks are not supported in the current phase

### Requirement: Native execution SHALL use a runtime scheduler bridge
Compiled async programs SHALL execute through a native runtime bridge that can create scheduler state, drive the root async task to completion, and recover the final program result.

#### Scenario: Async wrapper entrypoint drives the root task
- **WHEN** a Sengoo program with an async entrypoint is compiled for native execution
- **THEN** the generated executable entrypoint can create scheduler state, poll the root task until completion, and return the completed async result through stable runtime bridge symbols
