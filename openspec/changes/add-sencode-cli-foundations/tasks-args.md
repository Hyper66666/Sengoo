# OS argument implementation owner

- [x] Preserve strict UTF-8 OS arguments on Windows, including quote, empty, trailing-slash and supplementary Unicode values.
- [x] Verify managed Buffer initialized lengths, public argument/current-directory access, Unicode directory existence/creation and extreme-index diagnostics through `cargo test --locked -p sgc --test args_unicode` on Windows x64.
- [x] Repair managed environment copy initialized lengths and verify OS-wide Unicode/empty environment values through the same public native integration.
- [ ] Execute the same argument integration on actual Linux/macOS hosts.

Owner: process/system/package implementation. This fixes a standard runtime
boundary; the CLI command parser and diagnostic `probe` remain CLI-owned.
