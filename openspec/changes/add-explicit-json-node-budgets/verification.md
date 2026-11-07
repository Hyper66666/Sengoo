# Local verification —2026-11-07

Host DESKTOP-FUAQMFB, Windows x64; baseline
`86c6f87975646b91c9cf2d7d069ac06ee4243b9b`, dirty source tree.
Compiler SHA256
`cc280b3bf0ec0bf17650b0745500c72e205ee2d2073af8ea7ed552045570f1b0`.
Evidence: `target/network-context-20261107/json-evidence.json` records source
hashes and independent native/public-wrapper passes.

`json_bounded_probe.c` passes5000-object parse/copy/serialize, legacy4096
rejection, explicit node13, malformed10, invalid pre-read admission, buffer
initialization, failed-copy rollback,5002 links and complete reservation release.
Existing exact16MiB, NUL, cycles, aggregate and ownership cases also pass.

`json_bounded.sg` compiles with the source-development compiler and passes
5000-value public wrapper parsing, copying refusal, borrowed buffer limits,
>1MiB Unicode, owned NUL and scope Drop. Independent normal binaries exit0.

The AddressSanitizer binary exits0 but emits the existing LLVM19 Windows
`interception_win` warning. This is not clean sanitizer qualification.
Other platforms and a published toolchain payload were not tested here.
Strict validation: this change and all71 OpenSpec items pass.

An isolated temporary-receiver probe also confirms an existing compiler
limitation: chained borrowed method calls on an unnamed owning JsonDoc can
retain the temporary. CLI context code uses named owners; rich-history tests
assert zero remaining bounded documents after scope exit. No general compiler
temporary-lifetime fix is claimed by this runtime API change.
