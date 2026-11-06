# System runtime implementation owner

- [x] Implement domain-checked owned file locks, persistent lock files and nonblocking shared/exclusive acquisition.
- [x] Implement OS canonicalization and bounded UTF-8 Windows Generic Credential helpers; POSIX credentials explicitly unsupported.
- [x] Verify cross-process locking, Windows junction/case normalization and disposable credential lifecycle on Windows x64; native ASan probe passes.
- [x] Verify public Sengoo FileLock/String APIs and automatic Drop after native integration; FileLock/String/opaque counts return to baseline.
- [ ] Execute POSIX locking/path checks on actual Linux/macOS hosts.

Owner: process/system implementation. Targeted verification:
`cargo test --locked -p sgc --test system_runtime`. Native business logic remains
in Sengoo; this module only owns operating-system resources and bounded values.
