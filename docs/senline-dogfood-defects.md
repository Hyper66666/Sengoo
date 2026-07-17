# Senline Sengoo Dogfood Defects

Recorded: 2026-07-14

These records cover defects exposed while implementing the linked Senline
change `adopt-sengoo-backend-slice` in the isolated
`codex/senline-service-dogfood` worktree. All fixes below are still uncommitted
working-tree changes. Their status remains `open` until a clean Sengoo commit,
immutable Windows/Linux artifacts, and the reviewed Senline pin exist.

No record contains live Senline payloads, identifiers, credentials, or
secrets. The reproductions use synthetic literals only.

## SGDOG-2026-001: Buffer Extension Re-exposed Cleared Bytes

- Classification: `sengoo-standard-library`
- Owner: Sengoo runtime/stdlib
- Status: `open (local regression green)`
- Consumer requirement: checked byte access for binary worker framing
- RED: `stdlib_buffer_zeroes_gaps_before_exposing_extended_bytes`
- Failure: `Buffer.clear()` retained bytes and a later high-offset byte/u32
  write expanded `used_len`, making the stale gap readable again.
- Fix: zero every gap before extending `used_len`; exact stdin reads now stage
  bytes and commit only after the complete read succeeds.
- GREEN: `cargo test -p sgc stdlib_buffer_zeroes_gaps_before_exposing_extended_bytes`
- Remaining gate: partial-write/error injection and Linux pipe evidence

## SGDOG-2026-002: Strict JSON Broke Permissive Unicode Compatibility

- Classification: `sengoo-standard-library`
- Owner: Sengoo runtime/stdlib
- Status: `open (local regression green)`
- Consumer requirement: add strict JSON without changing existing callers
- RED: `stdlib_json_permissive_unicode_escape_behavior_remains_compatible`
- Failure: Unicode/surrogate decoding was added to the shared parser and
  changed permissive `json_parse*` behavior.
- Fix: strict and permissive escape behavior are explicitly separated.
- GREEN: `cargo test -p sgc stdlib_json_`
- Remaining gate: malformed corpus and fuzz evidence

## SGDOG-2026-003: JSON Strings and Handles Were Unsafe for a Long-Lived Worker

- Classification: `sengoo-runtime-ownership`
- Owner: Sengoo runtime
- Status: `open (local regression green)`
- Consumer requirement: strict Unicode plus repeated worker evaluations
- RED: `stdlib_json_strict_preserves_escaped_null_as_string_data` and
  `stdlib_json_document_handles_reject_forged_and_reused_stale_values`
- Failure: C-string-backed values rejected legal `U+0000`; raw pointer
  `JsonDoc` handles leaked one allocation per close and forged handles could
  crash.
- Fix: parsed keys/strings carry explicit byte lengths; exact key lookup has a
  pointer+length API; JsonDoc uses generation-checked reusable slots.
- GREEN: `cargo test -p sgc stdlib_json_`
- Remaining gate: sanitizer/soak evidence and explicit allowlist decoder

## SGDOG-2026-004: Installed Native Builds Could Fall Back or Trust Stale State

- Classification: `sengoo-package-toolchain`
- Owner: Sengoo toolchain/distribution
- Status: `open (Windows local smoke green; immutable cross-target artifacts absent)`
- Consumer requirement: build the Senline worker outside the Sengoo checkout
- RED:
  - `installed_native_build_rejects_missing_manifest_runtime_without_cargo_or_checkout_fallback`
  - `relocated_sgc_without_manifest_rejects_implicit_source_checkout_fallback`
  - `installed_native_build_rejects_runtime_hash_mismatch_before_link_or_cargo`
  - `installed_check_rejects_tampered_runtime_bridge_payload`
  - `installed_commands_reject_external_runtime_overrides`
- Failure: `sgc` implicitly ran Cargo from a compiled-in checkout; native
  cache hits bypassed installed runtime verification; packages omitted the
  static runtime and per-file hashes.
- Fix: schema-2 manifest resolution verifies target, ABI, SHA-256, link
  contract, bridge completeness, and cache identity before reuse. Packaging
  includes the target runtime and installers verify the complete payload set.
  Cargo runtime construction now requires explicit
  `--runtime-mode source-development`, is source-workspace guarded, rejects
  daemon use, and records non-release provenance in build/run metadata. Normal
  installed commands reject `SENGOO_ROOT`, `SENGOO_STDLIB`, and
  `SENGOO_RUNTIME` before frontend or engine dispatch. Source-development cache
  identity covers the Rust runtime source tree and Cargo inputs, so runtime-only
  changes cannot reuse an older linked executable.
- Additional RED: Windows PowerShell wrote a UTF-8 BOM that `serde_json`
  rejected. Packaging now writes manifest JSON as UTF-8 without BOM.
- GREEN:
  - `cargo test -p sgc --test runtime_distribution`
    (`15 passed`, including fresh installed check/build/run/test with fake
    Cargo, command/cache/manifest path audits, bridge tamper rejection, and
    override rejection across check/build/Cranelift run/test)
  - Windows archive install followed by Cargo-free strict-JSON native build in
    `D:/senline/logs/sengoo-installed-smoke`
- Remaining gate: clean Windows/Linux rebuilds, installed worker/HTTP smokes,
  reproducibility, SBOM/provenance, and Senline pin advancement

## SGDOG-2026-005: Sgpm Could Not Select Explicit Runtime Provenance

- Classification: `sengoo-package-toolchain`
- Owner: Sengoo package manager/toolchain
- Status: `open (local regression green; immutable artifact and pin absent)`
- Consumer requirement: run the locked `senline-domain-worker` package loop
  without giving installed mode an implicit source-checkout fallback
- RED: `realworld_locked_loop_uses_real_toolchain_binaries`
  - first failed because delegated `sgc` remained in default installed mode;
  - then rejected `sgpm --runtime-mode source-development` because `sgpm` had
    no corresponding explicit option.
- Failure: `sgpm` constructed child `sgc` commands without carrying the
  runtime-mode decision, so package check/test/build could not intentionally
  dogfood a source runtime while retaining non-release provenance.
- Fix: `sgpm` now exposes global
  `--runtime-mode installed|source-development`, defaults to installed, and
  prepends the selected mode to every delegated `sgc` command. Formatting does
  not consult the compiler runtime.
- GREEN: `cargo test -p sgpm --test realworld_e2e realworld_locked_loop_uses_real_toolchain_binaries -- --exact --nocapture`
  (`1 passed`, including locked update/check/test/fmt/doc/build for the new
  root worker and `senline_facts_to_plan` path package)
- Remaining gate: clean installed-toolchain package loop, immutable Windows
  and Linux artifacts, fixing commit, and reviewed Senline pin

## SGDOG-2026-006: Empty Buffer Destruction Polluted FFI Error State

- Classification: `sengoo-runtime-ownership`
- Owner: Sengoo runtime/stdlib
- Status: `open (local regression green)`
- Consumer requirement: represent clean framed EOF without allocating a
  payload buffer or changing unrelated diagnostics
- RED:
  - `stdlib_buffer_zero_handle_drop_is_noop`
  - `stdlib_buffer_zero_handle_free_is_noop`
  - consumer clean-EOF path: `sgframing/tests/frame_pipe.sg`
- Failure: `Buffer::drop` and `Buffer::free` called the runtime with handle
  zero. A normal `FrameRead { eof: true, payload: Buffer { handle: 0 } }`
  therefore changed `ffi_last_error_code()` to `STATUS_INVALID_HANDLE`, and
  explicit cleanup did the same.
- Fix: treat a zero Buffer handle as an already-empty resource; explicit
  free succeeds and implicit drop performs no runtime call.
- GREEN:
  - `cargo test -p sgc stdlib_buffer_zero_handle_ -- --nocapture`
    (`2 passed`)
  - `cargo test -p sgc --test realworld sgframing_binary_pipe_covers_boundaries_and_exact_output -- --exact --nocapture`
    (`1 passed`, including clean EOF with unchanged FFI error state)
- Remaining gate: full stdlib/package regression, installed Windows/Linux
  worker EOF evidence, fixing commit, immutable artifacts, and Senline pin

## SGDOG-2026-007: Nested JSON Corrupted Containers After Node Growth

- Classification: `sengoo-runtime-json`
- Owner: Sengoo runtime
- Status: `open (local regression green)`
- Consumer requirement: validate realistic nested V1 objects and arrays whose
  parsed document contains more than 16 nodes
- RED:
  - consumer: `sgjson_contract/tests/array_rejections.sg`
  - minimized: `stdlib_json_nested_containers_survive_node_storage_growth`
- Failure: recursive object and array parsers retained pointers into the JSON
  document node array. Adding the seventeenth node could reallocate that array,
  after which the parser wrote members/items through stale pointers. Small
  fixtures stayed below the initial capacity and hid the defect.
- Fix: reacquire the current object or array node by stable node ID after every
  recursive child parse before reserving or appending container data.
- GREEN:
  - `cargo test -p sgc stdlib_json_nested_containers_survive_node_storage_growth -- --nocapture`
    (`1 passed`)
  - locked source-development worker package loop (`sgjson_contract`: `6 passed`)
- Remaining gate: malformed/fuzz/sanitizer coverage, Linux and installed
  runtime evidence, fixing commit, immutable artifacts, and Senline pin

## SGDOG-2026-008: Generation Exhaustion Could Produce Negative Runtime Handles

- Classification: `sengoo-runtime-ownership`
- Owner: Sengoo runtime
- Status: `open (local C boundary regression green)`
- Consumer requirement: long-lived workers must retain positive, non-aliasing
  runtime handles as reusable slots advance through their generations
- RED: `generation_handle_encoding_stays_positive_and_signals_exhaustion` in
  `tools/sgc/tests/runtime_handles.rs`
- Failure: Buffer, JSON document, opaque, String, and Process slot allocators
  encoded a generation with a signed `long long` left shift by 32 bits. At
  generation `0x80000000` the mathematical result no longer fit in signed
  `long long`, invoking undefined behavior and commonly producing a negative
  handle that the runtime then rejected. Eventual unsigned wrap also permitted
  an old generation value to be reused.
- Fix: shared runtime handle helpers cap generations at
  `SENGOO_RUNTIME_HANDLE_GENERATION_MAX=0x7fffffff`, compose the positive handle
  through `uint64_t`, signal exhaustion with generation zero, and make all five
  allocator families retire an exhausted slot instead of wrapping it.
- GREEN:
  `cargo test -p sgc --test runtime_handles generation_handle_encoding_stays_positive_and_signals_exhaustion -- --exact --nocapture`
  (`1 passed` on the current local host)
- Remaining gate: full runtime/stdlib regression and sanitizer coverage,
  installed Windows/Linux evidence, fixing commit, immutable artifacts, and
  Senline pin

## SGDOG-2026-009: Strict JSON Diagnostics Had No Stable Machine Kind

- Classification: `sengoo-standard-library`
- Owner: Sengoo runtime/stdlib
- Status: `open (local regression green)`
- Consumer requirement: map parser failures to frozen worker error codes
  without parsing diagnostic text
- RED: `stdlib_json_strict_reports_stable_error_kinds`; worker duplicate,
  invalid-Unicode, and trailing-byte fixtures all returned `malformed_json`
- Failure: strict parsing exposed only a status, offset, and mutable human
  message, so protocol code could not distinguish stable rejection classes.
- Fix: the runtime and `std::json` expose stable kinds `NONE=0`,
  `UNCLASSIFIED=1`, `DUPLICATE_FIELD=2`, `INVALID_UNICODE=3`, and
  `TRAILING_BYTES=4`. The worker snapshots the kind before any later JSON
  operation and never parses the message.
- GREEN:
  - `cargo test -p sgc stdlib_json_ -- --nocapture` (`19 passed`)
  - `cargo test -p sgc --test realworld -- --nocapture` (`13 passed`)
- Remaining gate: the last-error slot retains its existing immediate,
  process-global lifecycle; malformed fuzzing plus installed Windows/Linux
  runtime evidence remain required.

## SGDOG-2026-010: JSON Builder Truncated Owned Strings at Embedded NUL

- Classification: `sengoo-runtime-json`
- Owner: Sengoo runtime/stdlib
- Status: `open (local regression green)`
- Consumer requirement: exactly echo legal bounded ASCII identifiers,
  including decoded `U+0000` and bytes after it
- RED:
  - `stdlib_json_length_aware_builder_preserves_embedded_nul`
  - `stdlib_json_length_aware_builder_rejects_invalid_utf8`
  - `stdlib_json_owned_string_builder_preserves_invalid_handle_status`
  - consumer: `senline_worker_preserves_embedded_nul_in_owned_plan_strings`
- Failure: the only builder path used C-string length and truncated the suffix
  after NUL. An initial pointer-plus-length wrapper also risked treating a
  negative String pointer status as an address and rewrote invalid handles as
  invalid lengths.
- Fix: retain the legacy C-string helper, add a bounded pointer-plus-length
  ABI, and expose a checked owned-String builder that validates the document,
  String handle, stored byte length, pointer result, and UTF-8 in that order.
- GREEN:
  - `cargo test -p sgc stdlib_json_ -- --nocapture` (`19 passed`)
  - real worker NUL echo and all realworld tests (`13 passed`)
- Remaining gate: raw pointer callers remain responsible for pointer lifetime;
  sanitizer/fuzz coverage and installed Windows/Linux evidence are pending.

## SGDOG-2026-011: Early Return Moves Poisoned Reachable Fallthrough

- Classification: `sengoo-compiler-borrow-checker`
- Owner: Sengoo compiler
- Status: `open (local regression green)`
- Consumer requirement: an unsupported-operation return may consume one field
  without making the supported fallthrough request appear partially moved
- RED: `early_return_field_move_does_not_poison_the_fallthrough_path`; the
  checker reported `request` as partially moved after the moving branch had
  already returned.
- Fix: direct unconditional-return branches no longer merge their move state
  into the reachable fallthrough; non-terminating branches still do.
- GREEN: compiler library `1064 passed`, borrow `16 passed`, ownership
  `35 passed`, Clippy with warnings denied, and rustfmt check.
- Remaining gate: this is a bounded reachability fix, not a full control-flow
  lattice; complex nested or all-terminating branch expressions need a
  separate compiler design and regression set.

## SGDOG-2026-012: Nested Field References Produced Invalid LLVM

- Classification: `sengoo-compiler-codegen`
- Owner: Sengoo compiler
- Status: `open (local regression green)`
- Consumer requirement: pass nested owned plan fields by immutable reference
  to the length-aware JSON builder
- RED: `nested_owned_string_parameter_field_reference_generates_valid_ir`;
  Clang rejected a `select` that supplied an SSA `%String` where `%String*`
  was required.
- Failure: primary LLVM `AddrOf` lowering assumed every source local already
  had an address, but nested field extraction produces an SSA temporary.
- Fix: parameter and temporary SSA values are spilled to a same-typed stack
  slot before taking their address; existing stack-local references retain
  their original path. The worker's temporary handle-rewrapping workaround
  was removed and the real nested borrow now compiles and runs.
- GREEN: Clang reference tests `3 passed`, struct codegen `8 passed`, compiler
  library `1064 passed`, integration `26 passed`, Clippy, rustfmt, diff-check,
  and real worker tests `13 passed`.
- Remaining gate: the legacy JIT emitter is separate, and nested mutable
  references still require an addressable-place/MIR design before claiming
  original-place mutation semantics.

## SGDOG-2026-013: Ready HTTP Future Drop Leaked an Unpublished Request

- Classification: `sengoo-async-concurrency`
- Owner: Sengoo runtime
- Status: `open (local native regression green)`
- Consumer requirement: the loopback dogfood harness must release an accepted
  request when `next_request_async` is dropped or canceled before `result`
  publishes its handle.
- RED:
  - `http_server_next_request_async_drop_ready_releases_unpublished_request`
  - `http_server_next_request_async_cancel_ready_releases_unpublished_request`
- Failure: polling could accept, parse, and store a request handle in the
  future's ready outcome. Drop/cancel only unregistered listener interest, so
  the request table and client connection remained live forever when no caller
  consumed `result`.
- Fix: ready abandonment now atomically takes any successful unpublished
  request handle, writes the existing deterministic `504` fallback, closes the
  connection, and preserves reuse of the server. Pending and error outcomes
  retain their previous cleanup behavior.
- GREEN: focused ready-abandonment regressions (`2 passed`) and the full native
  net suite (`40 passed`).
- Remaining gate: product-level `senline-http-dogfood` future-drop equivalence
  and Windows/Linux installed package loops.

## SGDOG-2026-014: Package Module Maps Omitted Transitive Source Imports

- Classification: `sengoo-package-runner`
- Owner: Sengoo `sgpm`
- Status: `open (local regression green)`
- Consumer requirement: a package importing `senline_domain_worker` must also
  resolve the worker source's imports without redeclaring its internal package
  graph as direct product dependencies.
- RED: `sgpm_check_exposes_transitive_dependency_library_module_map`; the root
  package received only `direct=<lib.sg>`, and real locked HTTP compilation
  failed on unresolved `senline_build_identity` from the worker source.
- Failure: `module_map_value` filtered edges to `edge.from == node.id`, while
  `sgc` expands imported source recursively using one flat module map. The
  lockfile already contained the complete dependency graph, but `sgpm`
  discarded every transitive edge when constructing `SENGOO_MODULE_MAP`.
- Fix: collect the selected package's reachable dependency closure, encode
  aliases in deterministic order, and fail closed when one reachable alias
  names different library sources.
- GREEN: `cargo test -p sgpm --test integration transitive -- --nocapture`
  (`2 passed`) and the HTTP locked source-development test loop.
- Remaining gate: full `sgpm` integration, installed-toolchain, and Linux
  package loops; the flat module map still intentionally cannot represent a
  graph with conflicting aliases.

## SGDOG-2026-015: HTTP Request Copies Lost Buffer Length Metadata

- Classification: `sengoo-stdlib-net-buffer-contract`
- Owner: Sengoo runtime and `std::net`
- Status: `open (local regression green)`
- Consumer requirement: request string accessors and `body_copy` must return
  owned data that remains valid for strict JSON parsing and response policy.
- RED: `real_sgc_http_request_owned_string_accessors_are_safe` first ended the
  child with Windows status `3221225477` (`0xC0000005`) and socket error 10054;
  after minimizing the accessor, the body-copy assertion returned HTTP 500
  because `used_len` remained zero.
- Failure: the native request-copy ABI writes through a raw pointer and cannot
  update the owning Buffer handle's `used_len`. Owned string wrappers therefore
  passed a nonzero copied length to `string_from_buffer` with `used_len == 0`,
  received an invalid-argument result, and eager Sengoo boolean evaluation let
  consumer code dereference the placeholder String handle. Body bytes reached
  strict JSON with the same stale metadata and normalized to `malformed_json`.
- Fix: owned request-string accessors use the existing length-aware native byte
  copy constructor, and successful request copy wrappers commit the exact
  copied length through a checked private runtime primitive. The HTTP harness
  also uses sequential Result guards so error paths never read placeholder
  values under eager `and`/`or` semantics.
- GREEN: `cargo test -p sgc --test http_request_strings -- --nocapture`
  (`1 passed`), locked HTTP package tests (`2 passed`), and a local real worker
  versus HTTP synthetic fixture comparison (`647` equal bytes, SHA-256
  `8b790b24c0f6306287caaef34544ecbab5d6ccd638af4294265eb2446fba545d`).
- Remaining gate: full request-copy compatibility, malformed transport, larger
  or segmented request, installed Windows, and Linux localhost matrices.

## Pin State

No defect is pinned or closed. The local Windows dogfood manifest records
`source_dirty=true`, `artifact_provenance=prebuilt-unverified`, and
`release_eligible=false`; it must not be copied into Senline's immutable
bundle manifest. Even a manifest that claims `release_eligible=true` cannot
self-authenticate as Senline pin evidence: `senline_pin_evidence` remains false
until Senline independently verifies a clean revision and immutable complete
bundle hashes. There are no active Senline workarounds for these defects.

## Resource Observation (task 8.3, not a closed defect)

- Classification: `sengoo-runtime-resource`
- Owner: Sengoo runtime / worker long-session
- Status: `open (investigation required)`
- Observation: a **single-process** 100,000-case differential run reached the
  3600-second watchdog near case **44,086** while private working set and
  throughput continued to degrade.
- Related success: task 5.11 used **8 fresh workers × 12,500** cases and
  recorded transcript digest
  `16aebd9ec476d602c9c0d0082ee9e25a87c520c333d6dd3afeb314f8c39ea128` with zero
  mismatch/crash/hang/malformed/nondeterminism. That sharded result **does not**
  satisfy task 8.3 soak or stable post-warm-up memory requirements.
- Directive: investigate under task 8.3 with checked-in sampler methodology;
  do not re-label shard success as resource green.
- **2026-07-15 re-measurement (Windows x64, pre-fix)**  
  `senline_worker_resource` `investigate-45k` stopped at case **29,014 / 45,000**
  on a 900 s watchdog; PWS ~5.2→96 MiB (~**3.27 KiB/case**); cps ~1740→7.3;
  handles flat at 66. Evidence:
  `target/senline-resource/soak-investigate-45k-windows-x86_64-1784109344.summary.json`.
- **Root cause (fixed):** by-value `String` parameters to lowered lambdas
  (field-allowlist callbacks in `sgjson_exact_object_fields`) never ran Drop
  glue. Each evaluation leaked ~dozens of key strings → linear PWS growth and
  slot-table scan slowdown. Compiler fix: force-record owned lambda param drops
  + `insert_drop_glue` for lambda MIR
  (`compiler/src/mir/lowering/lambda_expr_helpers.rs`,
  `drop_glue_helpers.rs::force_record_owned_param_drop`).
- **2026-07-15 post-fix (Windows x64):** same investigate-45k **completes**
  45,000/45,000 in ~56 s; PWS ~1.1→5.1 MiB (~**92 B/case** post-warm-up, under
  1 KiB/case); handles 66; `plan_ok=45000`. Evidence:
  `target/senline-resource/soak-investigate-45k-windows-x86_64-1784124464.summary.json`.
- **Residual root cause (fixed 2026-07-16):** worker helper
  `worker_validate_execution_mode(value: String)` took a by-value legacy handle
  and returned without Drop (ordinary function params skip auto-Drop for
  `String`). Combined with a second extract of `execution_mode`, each request
  leaked one owned mode string (~90 B/case). Fix: validate via `&str`, reuse the
  single extracted string; also return the validated literal from
  `worker_required_literal` instead of re-extracting; borrow
  `evaluation_id` for the unsupported-version encoder.
- **2026-07-16 residual post-fix (Windows x64):** investigate-45k **completes**
  45,000/45,000 in ~9 s; PWS ~1.0→1.14 MiB (~**3.4 B/case** post-warm-up, noise
  floor); handles 67; `plan_ok=45000`; p50/p95/p99 ≈ 154/332/500 µs. Evidence:
  `target/senline-resource/soak-investigate-45k-windows-x86_64-1784198111.summary.json`.
- **2026-07-17 ownership hardening:**
  1. Worker unsupported-version path: both branches move `WorkerRequestV1` into
     owning helpers so path-insensitive moved tracking cannot skip nested
     String Drop (`worker_reject_unsupported_operation_v1` /
     `worker_accept_decoded_request_v1`). Nested String fields Drop via
     aggregate param field bindings (not bare `String` params).
  2. Resource harness kills the worker before joining stdout/stderr readers so
     a hung worker cannot freeze the watchdog.
  3. Always-on regression:
     `resource_unsupported_operation_version_path_does_not_grow_memory`.
  4. **Not** auto-Dropping ordinary by-value `String` params: method receivers
     are still lowered as handle copies (`len`/`as_str`); enabling param Drop
     free'd live objects under the caller's handle. Full language ownership
     remains an open compiler task (review P1).
- **2026-07-17 1M soak (Windows x64):** `resource_single_worker_soak_1m`
  completed 1,000,000/1,000,000 in ~238 s; PWS growth **~0.066 B/case**;
  handles 68; `plan_ok=1000000`; p50/p95/p99 = 179/350/450 µs. Evidence:
  `target/senline-resource/soak-soak-1m-windows-x86_64-1784280826.summary.json`.
  Task 8.3 resource stability gate is satisfied on the recorded Windows host.
