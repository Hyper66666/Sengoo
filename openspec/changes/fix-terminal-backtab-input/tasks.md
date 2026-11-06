# Tasks

- [x] Preserve a failing native standard-BackTab reproduction on the original runtime source.
- [x] Add bounded standard BackTab normalization without changing ordinary Tab or paste parsing.
- [x] Pass dedicated native complete/split/EOF/paste and owned Windows Shift+Tab/Tab parity checks.
- [x] Pass a compiled public Sengoo consumer with redirected input and retain exact source/compiler/binary evidence.
- [x] Run affected existing terminal regressions and strict OpenSpec validation; record unverified platforms separately.

Windows x64 evidence: `target/terminal-backtab/evidence.json`.
The new `terminal_backtab` Rust integration passes 1/1. Existing
`process_terminal` (2/2), `terminal_polling` (1/1), and `terminal_screen` (3/3)
pass against the same runtime source. The change validates strictly, and all
68 OpenSpec changes/specs pass. This is common decoder plus Windows native
evidence; a live POSIX terminal is not qualified.
