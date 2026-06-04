## Why

The current Sengoo standard library is already usable for small scripts, CLI
smoke tests, basic filesystem work, process metadata, sync stdio, scalar
collections, and reflection wrappers. The remaining gap to mainstream-language
ergonomics is now less about one missing helper and more about a few shared
foundations:

- fallible stdlib APIs mostly collapse errors into `1`, which makes scripts
  hard to diagnose and makes automated recovery awkward;
- runtime-produced text still depends on caller-managed `Buffer` handles, but
  `Buffer` does not yet carry enough common text/byte operations to support
  larger APIs cleanly;
- collection examples are scalar-only, so common string-keyed maps and text
  lists remain deferred;
- JSON/data-format helpers, recursive filesystem traversal, richer metadata,
  and process capture/control are explicitly gated by earlier specs but not yet
  specified.

This change defines the next standard-library usability wave before
implementation agents begin coding.

## What Changes

- Add a stable `std::status` error-code taxonomy and error description helpers
  so fallible modules can stop returning undifferentiated error values without
  changing the existing `std::error` assertion-helper module.
- Extend the managed `Buffer`/text boundary while preserving the current
  no-owned-string-return ABI.
- Add copied-text collection requirements for text lists and string-key maps
  without requiring full language-level owned strings.
- Add handle-based `std::json` requirements for parsing, querying, building,
  and serializing JSON through managed handles and `Buffer` outputs.
- Add portable filesystem metadata and deterministic recursive traversal
  requirements.
- Add a shell-free process command/output model for dynamic argv, stdout/stderr
  capture, cwd/env overrides, and timeouts.
- Migrate current fallible stdlib wrappers away from undifferentiated
  `error: 1` where a stable `std::status` category can be inferred.
- Split the stdlib C runtime bridge into a bundle rooted at `runtime.c` plus
  domain-specific sibling C sources.
- Keep async I/O, terminal control, shell parsing, streaming JSON, and package
  ecosystem changes out of this wave.

No **BREAKING** source-language change is intended.

## Non-Goals

- Do not specify a full owned-string return ABI or garbage collector.
- Do not require `Vec<&str>` / `HashMap<&str, &str>` to store borrowed language
  references with unclear lifetimes; text collection APIs must copy text into
  runtime-owned storage or managed buffers.
- Do not add implicit shell execution. Callers must still choose a shell
  executable explicitly if they want shell syntax.
- Do not require async I/O runtime wakeups, cancellation, or user-defined
  awaitable integration in this change.
- Do not change package-manager semantics, dependency aliasing, or multi-version
  dependency resolution.
- Do not repurpose `std::error`; it remains the existing assertion-helper
  module used by `examples/stdlib/03_error.sg`. Runtime status categories live
  in new `std::status`.

## Capabilities

### Modified Capabilities

- `stdlib-mainstream-usability`: Extends the standard-library requirements with
  the next wave of mainstream scripting and CLI ergonomics.

## Impact

- Affected source modules:
  - new `tools/stdlib/status.sg`
  - `tools/stdlib/ffi.sg`
  - `tools/stdlib/collections.sg`
  - `tools/stdlib/file.sg`
  - `tools/stdlib/dir.sg`
  - `tools/stdlib/process.sg`
  - new `tools/stdlib/json.sg`
  - `tools/stdlib/runtime.c`
  - new `tools/stdlib/runtime_shared.h`
  - new `tools/stdlib/runtime_collections.c`
  - new `tools/stdlib/runtime_json.c`
  - new `tools/stdlib/runtime_process.c`
- Affected tooling:
  - `tools/sgc/src/stdlib_imports.rs`
  - `tools/sgc/src/native_toolchain.rs`
  - `tools/sgc/src/toolchain_discovery.rs`
  - `tools/sglsp/src/stdlib.rs`
  - `examples/stdlib/README.md`
  - `tools/stdlib/README.md`
  - `docs/language-features.md`
- Public Sengoo syntax and typing rules are unchanged.
- Existing `import std::error` assertion examples remain source-compatible.
- Runtime ABI additions are expected, but existing stdlib functions must remain
  source-compatible.
- No new external dependency is required unless a later implementation agent
  updates this OpenSpec with a dependency-specific rationale.
