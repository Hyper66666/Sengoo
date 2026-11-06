# Tasks

- [x] Add plain launch receipt and birth-matching API without changing runtime ABI or existing owned spawn semantics.
- [x] Implement hidden Unicode Windows launch with NUL-only handle whitelist, explicit immediate-job breakaway and failure cleanup.
- [x] Verify real parent exit, mismatched birth, Unicode argv/cwd/env, arbitrary event noninheritance, forbidden breakaway and 24 failed launches (native probe, Windows x64).
- [x] Run native Windows AddressSanitizer probe; no sanitizer failure (Windows LSan is not claimed).
- [x] Verify native/public Sengoo wrapper through process_detached.rs (2/2, Windows x64, final run 17.10s); existing process_stdin (1/1) and process_terminal native/public wrapper cases also passed.
- [x] Re-run runtime distribution against the final public process source (`runtime_distribution`: 15/15, Windows x64, 95.57s), including fresh installed consumers without Cargo or absolute source runtime paths.
- [x] Document the public contract and validate this OpenSpec change (`openspec validate add-detached-process-launch --strict`).
