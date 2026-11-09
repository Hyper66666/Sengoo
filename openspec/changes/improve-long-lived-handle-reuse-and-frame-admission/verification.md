# Windows native verification, 2026-11-08

Host: DESKTOP-FUAQMFB, Windows x64. Baseline Sengoo
`0039f59e04e68acc386c9f49497b32178bc879b9`; adjacent Sencode CLI
`390ddc2c6999e7eba801b9d636d4c4038226236b`. Changes are uncommitted additions
on these baselines. Compiler SHA256:
`6507e0e2eff15fd375054334f4e4ff6bccf540431f5f81bd56625f7f0d616252`.
Native source-development runtime, clang 19.1.7; not release qualification.

Evidence: `D:/Sencode-CLI/target/viewport-20261108/foundation-evidence.json`.
All eight recorded executable runs passed, with SHA256 checksums:

- String/Buffer/opaque churn: 20000 retained values and 100000 temporary
  allocations per domain; retained values, stale generations and live-count
  cleanup checked. Raw Buffer/opaque owners are explicitly released to isolate
  runtime-slot behavior from generic container Drop behavior.
- String iterator probe: domains/generations, repeated release, independent
  snapshots and 50000 cycles.
- Rich JSON-event arena admission and exact cleanup, plus named owned Result
  propagation: independent compatibility regressions passed.
- Real owned Windows console/process C probe: geometry mismatch rejects writes,
  same-size resize notification is delivered, queued input/modes remain valid;
  existing dual-pipe, timeout, Unicode and descendant cleanup checks passed.

The same 12000-row viewport computation measured 485 ms with the prior handle
allocator and 16 ms with freelists for 60 scroll/paint computations. The isolated
String allocation loop changed from 1468 ms to a coarse 0 ms; Buffer/opaque loops
reported 15/16 ms. Zero means below this coarse clock's observed resolution,
not zero execution cost. These are local native computation samples; terminal
wheel delivery is measured independently by the CLI's real-console fixture.

`terminal.write_frame` and its false-on-geometry-race contract are documented in
`docs/terminal-frame-and-handle-reuse.md`. Existing strict text/ordinary terminal
write behavior and runtime ABI remain unchanged.

OpenSpec strict validation passed for this change and all 75 Sengoo items.
Detailed logs are under the CLI candidate's `candidate-3/openspec-sengoo-*.log`.
This evidence covers the named runtime paths, not the whole compiler ownership
matrix, sanitizers, non-Windows execution or a fresh machine installation.
