# Sengoo Runtime Platform Behavior

> Documents portable vs host-specific behavior for the C/Rust runtime bridges.
> When portable behavior cannot be guaranteed, APIs return stable status codes
> from `std::status`.

## Path encoding

- Paths are **UTF-8 byte strings** end-to-end. No wide-character conversion at
  the runtime boundary.
- `std::path` helpers normalize lexically; they do not resolve symlinks.
- Windows accepts both `/` and `\\` in many APIs; prefer `path_join` for
  composition.

## Permissions and symlinks

- Metadata helpers report `kind`, `size`, and `mtime` for regular files and
  directories when the host stat call succeeds.
- Permission denied maps to `STATUS_PERMISSION_DENIED` where errno/`GetLastError`
  can be classified.
- Directory walk defaults to **no symlink follow**; unsupported follow modes
  return `STATUS_UNSUPPORTED`.

## Process execution (`std::process`)

- **Shell-free:** all `process_run*` and command-builder paths use `exec`/`CreateProcess`
  style argv vectors. Arguments containing spaces or shell metacharacters are
  passed literally.
- **Timeout:** child is terminated on timeout; exit status is
  `STATUS_TIMEOUT` (11). Partial stdout/stderr remain readable when captured.
- **Env clear:** `env_clear` removes inherited variables including `PATH` unless
  re-added; documented in examples.
- **Signals:** POSIX-only signal delivery is not exposed; requesting unsupported
  signal behavior returns `STATUS_UNSUPPORTED`.

## Stdio capture

- Inherited, captured, and null stream modes are supported on Windows and POSIX
  for the command-builder API.
- Capture buffers enforce size limits; overflow returns
  `STATUS_OVERFLOW` or truncates per API contract in `runtime_process.c`.

## Network / HTTP

- TLS is **not** guaranteed; HTTPS URLs may return `STATUS_UNSUPPORTED` when the
  host bridge lacks TLS.
- Header/body size limits are configurable on server helpers; defaults documented
  in `runtime_breadth.c`.
- Invalid handles return `STATUS_INVALID_HANDLE` (3).

## Dynamic FFI

| Host | Dynamic load |
|------|--------------|
| Windows x64 | Supported via `LoadLibraryW` / `GetProcAddress` (Rust bridge) |
| POSIX (Linux, macOS) | Supported via `dlopen` / `dlsym` (Rust bridge) |
| C-only stdlib bundle | `STATUS_UNSUPPORTED` (`-2007` FFI code → category 8) |

Missing symbols return `SYMBOL_NOT_FOUND` (-2003), not link failures.

## Status mapping for host failures

| Condition | Status |
|-----------|--------|
| ENOENT / file not found | `NOT_FOUND` (5) or `IO` (9) |
| EACCES / permission denied | `PERMISSION_DENIED` (7) |
| ETIMEDOUT / wait timeout | `TIMEOUT` (11) |
| Unsupported API on host | `UNSUPPORTED` (8) |
| Invalid handle / use-after-close | `INVALID_HANDLE` (3) |
| Input exceeds documented limit | `INVALID_ARGUMENT` (2) or `OVERFLOW` (13) |

## Accepted platform skips in CI

- HTTP server bind tests may skip on hosts without loopback server support.
- Lua 5.4 dynamic library tests skip when `liblua` is not installed.
- Native link tests skip when `clang` or the runtime C bundle is unavailable.
