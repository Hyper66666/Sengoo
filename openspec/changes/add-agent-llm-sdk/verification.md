# Verification — sgllm experimental 0.1.0

Host: DESKTOP-FUAQMFB, Windows x64, 2026-11-09. Source-runtime native builds.
Baseline Sengoo revision: `ea803f1913f9d08f6393d412b0eb4b1de3055b3e`.
Baseline CLI revision: `c603b84022bb3afeec4f3fb217dd016abd48f028`.
The working trees contain the SDK and concurrent integration changes.

## Frozen sources and tools

- `packages/sgllm/src/lib.sg` SHA256:
  `b0a5bf45e6b907ba33cfcf2bd543d20df958f8418643229a40bfd3a09cb8f736`.
- Compiler: `target/response-retry-compiler/debug/sgc.exe`, SHA256
  `6507e0e2eff15fd375054334f4e4ff6bccf540431f5f81bd56625f7f0d616252`.
- Formatter: rebuilt `target/agent-formatter/debug/sgfmt.exe`, SHA256
  `d075a1a0fd51a8e2434650ea542f5f252f526198ff1bf0ba27a7fce3ea7ba814`.
- Native linker/compiler: verified local Clang 19.1.7 MSVC target.
- Package manager: `target/debug/sgpm.exe` (toolchain paths are explicit in the runner).

## Executable verification

`packages/sgllm/tests/run.py` copies package sources into a fresh temporary
directory outside both repositories, sets the copied module map and compiles
every named test using source-development mode. A separate manifest project
uses the copied package by relative dependency, creates its lockfile and checks/
runs locked without modifying that lockfile. No CLI module is available there.

Coverage:

- Existing three-protocol, strict completion, tool/usage, rich JSON metadata,
  continuation and Unicode protocol tests.
- HTTP framing and SSE byte fragmentation; ordered image encoding/validation
  remains explicitly capability-gated.
- Config rejection before workers: unknown URL/protocol/model, queue bounds,
  serialized request byte limits and credential header injection.
- Four simultaneous native byte streams, three protocols, split UTF-8 and
  JSON arguments, canonical calls and usage. Premature result retrieval fails
  without stopping the active job.
- HTTP500+Retry-After, malformed/incomplete HTTP200, actual truncated HTTP,
  partial SSE, idle timeout, four cancelled
  jobs, no implicit retry and final worker/handle/request-byte reclamation.
- Twenty repeated invalid endpoints, malformed events, rejected completions,
  SSE overflow and invalid/oversized HTTP slices, comparing String, Buffer,
  bounded JSON and opaque handle counters after each round.
- All nine package SG files formatted and `sgfmt --check` clean.
- The complete `examples/stream.sg` builds and runs against a synthetic Responses
  peer, producing Unicode streamed text and usage.

Pre-format, resource-fixed proof (17 passing steps) remains at
`packages/sgllm/target/pre-format-resource-fixed-acceptance.json`.
An earlier 15-step report predating the additional owning-error audit remains
`target/pre-resource-fix-acceptance.json`; it is not the final candidate evidence.
The post-format run with the original compiler passes all 28 steps and is
preserved as `packages/sgllm/target/old-compiler-final-acceptance.json`, SHA256
`91f831068c476230bf8044bfa5dcf6e842be1371dbba14ae36502e8ac5ae1766`.
Those steps include nine formatter checks, seven test builds and executions,
the example build/Responses execution and three locked external package steps.

The integration selects the subsequently repaired temporary-ownership compiler
at `target/agent-temporary-compiler/debug/sgc.exe`, SHA256
`deb91ea7223b5cf6c41d9fed585db4af73f5a555183470e0c42c9210ee30432d`.
The separate full run with that compiler passes all 28 steps at
`packages/sgllm/target/acceptance.json`, SHA256
`a874ddad1857baed0cf7c2c5d3a2053814527b52eff9ec0c27fa60e048d9aca6`.
The source hash in that report matches the frozen library above and the runner
verified that all snapshotted inputs remained unchanged. HTTP peers report no
provider errors. Original-compiler proof is retained separately.

The integration subsequently selects an additional compiler repair for unused
owning `?` statement values, SHA256
`f7ea03a89c27ccedf8e6f2f8c81fe7ab6008887dd028fd8848102774e2f5dde5`.
That candidate's combined SDK/CLI qualification belongs to the integration
owner; the passing deb91 report does not certify the newer binary. SDK source
does not bypass or suppress the language regression. No final toolchain release
is claimed by this component evidence.

The later streaming sample uses public `std::io` raw stdout and flush on
nonempty deltas rather than builtin printing for each event. The library source
hash remains unchanged. A targeted 11-step format/example run with compiler
`69a42135586f5fa56debe10f35e51e4423a926463e88c3f3bfa5f5e1f596ddce`
passes at `packages/sgllm/target/exact-stream-69a-acceptance.json`, SHA256
`e08109b5807f277ef225f0ee4de0ceeea2111aa860986a7102a27902dd056c5a`.
Its exact stdout is `世界😀\n17\n8\n`; sample binary SHA256 is
`067ab3801875f8f88ecb2a84c38a3f2f7a2dbc21b17ad4c7dabcba1de5543481`.
This verifies the changed output path, not the newer compiler's entire SDK
resource gate. The final selected compiler's complete gate remains separate.

An attempted format using the stale `target/debug/sgfmt.exe` exposed a toolchain
selection issue: its old parser split `return self.method()` into `return;`
and a separate call. The malformed program failed native compilation. Minimal
before/after files remain under `packages/sgllm/target/formatter-return-self-*`;
the failed report is `target/lossy-old-formatter-acceptance.json`. The integration
owner verified that current parser source already supports self expressions,
added a semantic regression and rebuilt the formatter above. SDK returns were
restored and all nine SG files reformatted with that verified executable; no
source workaround suppresses the regression. The old formatter is not qualified.

## Failure ownership audit

Sengoo's current Result convention drops an owning payload only on success.
Every explicit SDK owning failure payload now carries a zero handle. In
particular, `vec_new()` creates a runtime vector even when empty, so SSE errors
use a typed zero-handle `Vec` literal, not an allocated empty vector. Locals
remain owned by the function on early return and are dropped there. Diagnostics
are retrieved from the job rather than returned in partially owned error values.

Byte-slice offsets/counts and decoder admission are validated before output
allocation. The SDK still uses the language's supported ownership subset; this
audit does not claim all compiler temporary or async cancellation paths sound.
Any separate compiler repair must be independently verified before a later
toolchain candidate is qualified.

## Boundaries

Only the listed Windows host was run. These are native/loopback/package consumer
checks, not live API, clean-machine, sanitizer or four-platform qualification.
Sencode adapter and distribution gates belong to the integration owner and are
not completed by this package's tests. The first package is experimental and does
not imply stable ABI, generic object Python binding or a hosted registry.

## Final completion toolchain, 2026-11-09

All 28 full-run steps pass in
`target/agent-completion/llm/evidence.json`, SHA256
`392797ba45349cd531e6600ada788e09f53640f07a5678e587497bfa15c5bcbf`.
This includes nine format checks, all seven native/loopback test groups, the
exact streaming example and an external sgpm update/locked-check/locked-run.
The four-way stream/cancellation, protocol failures and repeated resource tests
remain enabled; this is not `--example-only` qualification.

The compiler SHA256 is
`7f6343885839b8dfbcd9e03509a3da73739c1ee88d87224fd01fcb1ec5fca7d7`,
runtime C is
`eaa900898e670775f8019a8ddc363c714861db6d9887f6c04d04cb97575995d5`,
and io.sg is
`7cc532211d6dabf47eb78bba51680d3270fd8cc61b82bbc7bb6c212fa80ab88f`.
The runner now records compiler, clang, formatter and sgpm paths/hashes, plus
all standard-library/runtime inputs. SDK, foundation and tool unchanged-input
guards all pass, including detection of newly added or removed source files.
The earlier runner report remains at
`target/agent-completion/llm/pre-runtime-registry-lock.json` with its original
scope. No Windows fixture success is extended to live providers or other hosts.
