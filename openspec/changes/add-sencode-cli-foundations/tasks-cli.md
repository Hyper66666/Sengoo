# CLI hierarchy implementation owner

- [x] Implement bounded hierarchical commands, ancestor options and sibling scope isolation without changing old single-level signatures.
- [x] Native regression covers three levels, inherited required options, sibling types, conflicts, unknown children, `--`, and stable help.
- [x] Verify native AddressSanitizer and public Sengoo automatic-ownership tests on Windows x64.
- [ ] Qualify other supported native hosts through actual execution.

Owner: process/system/package implementation. Package `tests/native.c` and
`tests/cli.sg` are the executable acceptance sources.
