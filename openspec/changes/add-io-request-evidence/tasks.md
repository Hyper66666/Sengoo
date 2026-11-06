# Tasks

- [x] Add transport flag and public Result API without changing ABI 1.
- [x] Rust request evidence regression: refused connect, TLS handshake failure,
  post-write disconnect, cancellation and closed handles.
- [x] Sengoo public API native regression: sent flag, cancellation persistence,
  invalidated owner; `cargo test -p sgc --test net_byte_stream`.
- [x] Preserve Windows arrow/text repeats and clear them at input barriers;
  `cargo test -p sgc --test terminal_screen` (3 tests).
- [x] Validate strict change and all specs (57/57 at this gate); document API behavior.
- [ ] Record candidate source hashes with CLI integration.
