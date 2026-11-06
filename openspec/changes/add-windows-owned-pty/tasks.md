# Tasks

- [x] Implement checked owning PTY builder and dynamically discovered ConPTY launch.
- [x] Implement independent bounded overlapped I/O, poll, resize, direct-child/tree status and explicit EOF.
- [x] Add job-owned tree cancellation, timeout retention and idempotent cleanup, retaining ownership until the job is quiescent.
- [x] Run independent hidden ConPTY probes for Unicode/argv/cwd/environment, interactive echo, resize, EOF, 12 full-pipe/flood cycles, 8 failed launches, descendant cleanup and stale handles (`owned_pty` native regression, Windows x64).
- [x] Compile/run the public Sengoo PowerShell API/Drop and four Send/Sync negative cases (`cargo test --locked -p sgc --test owned_pty`, 3/3); final native probe passes clang 19.1.7 AddressSanitizer, with independent live/OS-handle cleanup checks.
- [x] Register source/installed runtime payloads and LSP/module surface; LSP regression 1/1 and fresh external installed consumer calling std::pty/cmd.exe 1/1 pass (`runtime_distribution::fresh_installed_build_and_run_avoid_cargo_and_absolute_runtime_identity`, 99.85s, no Cargo/source runtime path).
- [x] Document Windows x64 working-tree host evidence and unsupported boundaries; this change strict and all specs strict pass (57/57 items on 2026-11-04).
