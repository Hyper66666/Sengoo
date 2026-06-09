## 1. OpenSpec Draft

- [x] 1.1 Review the existing `stdlib-mainstream-usability` spec and current
  stdlib module surface.
- [x] 1.2 Add a proposal for the next standard-library usability wave.
- [x] 1.3 Add design notes that preserve the current Buffer/handle model.
- [x] 1.4 Add spec deltas for implementation agents to satisfy.
- [x] 1.5 Run `openspec validate stdlib-next-usability-wave --strict`.
- [x] 1.6 Run `openspec validate --all --strict`.

## 2. Error and Buffer Foundation

- [x] 2.0 Keep `tools/stdlib/error.sg` as the existing assertion-helper module,
  add new `tools/stdlib/status.sg`, and update docs/examples so reviewers do
  not infer a breaking `std::error` responsibility change.
- [x] 2.1 Add tests that show current fallible stdlib APIs preserve source
  compatibility while exposing distinct error categories.
- [x] 2.2 Add the stable `std::status` category constant table in both Sengoo
  source and the matching runtime bridge, using the numeric values specified in
  this change.
- [x] 2.3 Add `std::status` category name/message copy helpers and raw-code to
  category mapping helpers, including mappings for existing negative
  module-specific FFI/runtime codes.
- [x] 2.4 Extend `tools/stdlib/ffi.sg` and its runtime bridge with
  `Buffer` helpers for capacity, used byte length, clear, explicit byte-range
  copy, append/copy from `&str`, and UTF-8 validation, preserving `len()` as
  capacity.
- [x] 2.5 Update existing stdlib wrappers so new errors use the shared taxonomy
  where possible without changing successful results or raw helper behavior.
- [x] 2.6 Add examples covering status category checks and Buffer composition.
- [x] 2.7 Update `tools/stdlib/README.md`, `examples/stdlib/README.md`,
  `tools/sgc/src/stdlib_imports.rs`, and `tools/sglsp/src/stdlib.rs`.

## 3. Text Collections

- [x] 3.1 Add failing tests for copied text list append/get/set/remove/iterate
  behavior and deterministic string-key map iteration.
- [x] 3.2 Extend `tools/stdlib/collections.sg` and a domain-specific runtime
  bridge when practical with runtime-owned copied-text list handles.
- [x] 3.3 Add string-key map helpers for at least `&str -> i64` and
  `&str -> bool`, including key copy-out and deterministic iteration.
- [x] 3.4 Ensure inserted keys/text are copied so temporary `&str` inputs do not
  become dangling references.
- [x] 3.5 Specify and test duplicate-key replacement semantics plus byte-based
  ordering without Unicode normalization or locale collation.
- [x] 3.6 Add `examples/stdlib` coverage for text lists and string-key maps.
- [x] 3.7 Update docs and LSP stdlib signatures.

## 4. JSON Module

- [x] 4.1 Add parser/query tests for null, bool, number, string, array, and
  object values.
- [x] 4.2 Add `tools/stdlib/json.sg` with handle-based `JsonDoc` and value
  access wrappers.
- [x] 4.3 Implement runtime JSON parse/close/query/string-copy/serialize helpers
  in a domain-specific runtime bridge when practical instead of further growing
  `tools/stdlib/runtime.c`.
- [x] 4.4 Expose exact `i64` reads when representable and `f64` reads for JSON
  numbers.
- [x] 4.5 Add parse-error byte offset and message copy helpers.
- [x] 4.6 Add JSON construction helpers for object/array/string/number/bool/null
  values and serialization into `Buffer`.
- [x] 4.7 Enforce and document parser/builder resource limits for input bytes,
  nesting depth, and node count; failed parses must not return closeable
  partial handles.
- [x] 4.8 Wire `std::json` through `sgc`, `sglsp`, docs, and runnable examples.

## 5. Filesystem Metadata and Traversal

- [x] 5.1 Add tests for portable metadata fields on regular files,
  directories, absent paths, and unsupported fields.
- [x] 5.2 Extend `tools/stdlib/file.sg` or `tools/stdlib/dir.sg` with metadata
  helpers for kind, byte length, and modification time in Unix milliseconds.
- [x] 5.3 Add a recursive directory walk handle with max-depth and no-symlink
  default behavior.
- [x] 5.4 Ensure traversal order is deterministic by sorted path bytes and does
  not include `.` or `..`.
- [x] 5.5 Add close/free coverage for traversal handles.
- [x] 5.6 Add docs, LSP signatures, and examples for metadata and traversal.

## 6. Process Command and Capture

- [x] 6.1 Add tests that fixed-arity `process_run*` behavior remains unchanged.
- [x] 6.2 Add tests for dynamic literal argv entries, cwd override, env
  set/remove/clear, inherited streams, captured stdout/stderr, and nonzero child
  exit codes.
- [x] 6.3 Extend `tools/stdlib/process.sg` with command builder and
  `ProcessOutput` handle wrappers.
- [x] 6.4 Implement runtime command/output helpers in a domain-specific bridge
  when practical and without implicit shell invocation.
- [x] 6.5 Add timeout handling that reports timed-out output and documents
  platform limitations.
- [x] 6.6 Specify and test timeout exit-code semantics, partial output reads, and
  env-clear behavior when `PATH` is removed.
- [x] 6.7 Add close/free coverage for command and output handles.
- [x] 6.8 Wire docs, LSP signatures, and examples for process capture/control.

## 7. Cross-Cutting Verification

- [x] 7.1 Run OpenSpec strict validation for this change and all canonical
  specs.
- [x] 7.2 Run Rust compiler/runtime tests that cover stdlib import expansion and
  runtime ABI additions.
- [x] 7.3 Run `sgc check`, `sgc build`, and `sgc run` against every new
  `examples/stdlib` example.
- [x] 7.4 Run `sglsp` stdlib tests and confirm new public symbols/signatures are
  discoverable.
- [x] 7.5 Run formatting/linting commands used by the repository.
- [x] 7.6 Before archiving, modify the canonical gate requirements that
  previously deferred JSON, string collections, recursive traversal, and
  advanced process capture so the accepted spec no longer contradicts this
  change.
- [x] 7.7 Update the proposal/design/spec delta if implementation reveals a
  materially different API shape, especially if category numbers differ from
  this draft.

## 8. Review Feedback Closeout

- [x] 8.1 Update this change and the canonical spec so accepted JSON, string
  collections, recursive traversal, and process capture no longer contradict
  old deferred/gated text.
- [x] 8.2 Migrate current stdlib wrappers that still hard-code generic
  `error: 1` to stable `std::status` categories where the cause is known.
- [x] 8.3 Rewrite JSON/status examples to use public stdlib wrappers and
  explicit imports instead of raw C bridge symbols or implicit dependencies.
- [x] 8.4 Split stdlib runtime bridge code into anchor plus domain-specific C
  files and update sgc/test/native linking to compile the full runtime bundle.
- [x] 8.5 Add regression coverage for runtime bundle fingerprints, split-file
  linking, legacy status migration, and JSON bool wrong-kind errors.
- [x] 8.6 Re-run OpenSpec, formatting, build, clippy, Rust stdlib/LSP tests, and
  new/modified stdlib examples.
