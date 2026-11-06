# Process and terminal implementation owner

- [x] Implement bounded independent process pipes, real nonblocking status and timeout state repair.
- [x] Verify Unicode argv/executable/cwd/environment paths, stdin backpressure, dual-stream flood, termination and descendant cleanup on Windows x64.
- [x] Implement terminal raw ownership, bounded UTF-8/paste/key/resize/cancel/EOF events and restoration.
- [x] Verify native parser/non-TTY paths and real TTY mode restoration on Windows x64.
- [x] Compile and run the public Sengoo ownership/API integration and retain host-specific evidence in docs/process-terminal-runtime.md.
- [x] Replace legacy command/output tombstone allocations with domain-checked ownership, verified by 10,000 allocation/close cycles, ASan and existing idempotent-release/process regressions.
- [ ] Execute POSIX process/PTY integration and sanitizer gates on actual Linux/macOS hosts.
- [x] Add TTY-only `Terminal.discard_input()` and verify Windows OS/internal
  queue reset, retained cancellation and mode preservation in the real hidden
  console probe (`process_terminal`: 2/2); POSIX execution remains open above.
- [x] Add TTY-only `Terminal.clear_after_cursor()` with Windows off-viewport
  clearing and preserved prefix/cursor/modes; native/public `process_terminal`
  tests pass 2/2. POSIX implementation remains separately unverified.

Native I/O owns OS handles only; CLI line editing/rendering and orchestration
remain Sengoo. Targeted verification: `cargo test --locked -p sgc --test process_terminal`.
