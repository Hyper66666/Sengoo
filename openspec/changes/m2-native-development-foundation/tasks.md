# Implementation and verification

- [x] 1. Run owning aggregate/generic/borrow/control-flow/async cleanup composition tests and correct discovered defects (1132 compiler tests; ownership 15 and ordered collections 12; generated IR ASan with 19/19 resource cleanup).
- [x] 2. Add explicit shared-library builds whose selected exports are reachability roots (native shared-library ABI tests).
- [x] 3. Link runtime and package sources with correct PIC and explicit export boundaries (common native linker; isolated Python modules).
- [x] 4. Invalidate native object caches on source/transitive header/compiler/target/options/output-mode changes (20 native-toolchain tests and top-level build/run header-change regression).
- [x] 5. Keep existing source/ABI/diagnostic compatibility and document rejected boundaries (edition 2026, runtime ABI 1; stable soundness diagnostics).
- [x] Validate this change and repository OpenSpec strictly (51/51).

See `docs/development-evidence.md` for local scope and sanitizer limitations.
Owning async cancellation, unsupported closure environments and Drop/tagged
partial moves are explicitly bounded; this does not implement Rust lifetimes
or prove cancellation through FFI/indirect callbacks. Four-host candidate
execution remains an M5 gate.
