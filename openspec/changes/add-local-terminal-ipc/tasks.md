# Tasks

- [x] Implement additive fullscreen ownership and mouse event APIs.
- [x] Run the independent native terminal probe and Sengoo API compilation (`cargo test -p sgc --test terminal_screen`, Windows x64, 3/3 including the isolated clipboard fixture).
- [x] Add `std::system::current_executable_string()` using the OS current-image path, independent of cwd/PATH/argv0 (owner: root; `cargo test -p sgc --test current_executable`, Windows x64, 1/1).
- [x] Implement same-user, same-logon Windows byte IPC and owning wrappers.
- [x] Run real listener/client, bounded I/O, peer identity and cleanup probes (`cargo test -p sgc --test local_ipc`, Windows x64, 2/2).
- [x] Add generic Windows clipboard output using owned String length, strict UTF-8 and CF_UNICODETEXT; test Unicode, NUL/invalid input and failure ownership using an isolated Win32 fixture without invoking the interactive clipboard (`terminal_screen`, 3/3).
- [x] Add pure Sengoo Unicode17 extended-grapheme state, cursor boundaries and a documented terminal-width policy; pass all 766 official GraphemeBreakTest cases plus editing/width regressions (`cargo test -p sgc --test unicode_grapheme`, Windows x64, 1/1).
- [x] Add bounded streaming `hash_sha256_file` and owning output; verify 129MiB Unicode artifact, exact SHA256, short/error reads and size/regular-file limits (`cargo test -p sgc --test hash_file`, Windows x64, 2/2).
- [x] Reproduce early-return retention in all three String snapshot iterators, add automatic Drop and opaque generation validation, and preserve explicit repeated free (`cargo test -p sgc --test string_iter_ownership`, Windows x64, 2/2; 10,000 Sengoo and 50,000 native cycles).
- [x] Add explicit `try_write_stdin` alias to the existing piped-process requirement and verify immediate full-pipe backpressure, deadlines, descendant cancellation and input reuse (`cargo test -p sgc --test process_stdin`, Windows x64, 1/1; requirement remains owned by add-sencode-cli-foundations).
- [x] Run Windows clang 19.1.7 AddressSanitizer probes for String iterator ownership, IPC, terminal screen and process stdin without memory-access errors; handle/live-allocation checks independently cover cleanup (Windows LeakSanitizer is not claimed).
- [x] Register packaged sources/modules and verify source and installed paths (`runtime_distribution`, 15/15, including a fresh installed consumer using terminal, IPC, Unicode and file hashing without Cargo/source-root identity; LSP symbols 1/1; iterator Item compatibility 1/1; Unicode generator offline `--check`).
- [x] Package the Unicode data/license/provenance and verify their manifest SHA256 entries (`package-toolchain.ps1 -NoBuild`, separate local payload-smoke stage, seven required payloads; explicitly prebuilt-unverified and not release evidence).
- [x] Document observed Windows x64 working-tree evidence and limits; `openspec validate add-local-terminal-ipc --strict` and `openspec validate --all --strict` pass (55/55 items on 2026-11-04).
