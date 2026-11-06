- [x] Reproduce borrowed String method failure in compiler and terminal screen.
- [x] Reproduce direct destructor forwarding's wrong aggregate call in LLVM.
- [x] Accept borrowed String intrinsic and retain local-view escape rejection.
- [x] Bind destructor body self to local storage address and preserve explicit borrow.
- [x] Verify compiler IR/unit regressions and native O0/O2 resource behavior.
- [x] Rebuild compiler, run affected existing tests and strict OpenSpec validation.

Windows x64 evidence, 2026-11-04: `target/cli-receiver-compiler-suite-configured.log`
1155/1155, no ignored tests; `target/borrowed-receivers-native.log` O0/O2 passed,
field String count returned to baseline. Rebuilt sgc SHA256
`b79895635aabd0acfef17a81f9f7f0060fcc473bc56b41da47ebb7ff6912169c`.
The compiler suite first ran with no clang PATH and an outdated manual hash
module fixture dependency; that failed report is retained. The configured run
includes String in the fixture's explicit standard-library dependency list.
