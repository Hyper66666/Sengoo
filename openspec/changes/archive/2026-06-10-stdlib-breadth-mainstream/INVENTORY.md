# stdlib-breadth-mainstream inventory

## Landed modules (source)

| Module | File | Runtime bridge |
|---|---|---|
| `std::assert` | `tools/stdlib/assert.sg` | `runtime.c` (panic helpers) |
| `std::fmt` | `tools/stdlib/fmt.sg` | uses `strconv` + `status` |
| `std::regex` | `tools/stdlib/regex.sg` | `runtime_breadth.c` |
| `std::log` | `tools/stdlib/log.sg` | `runtime_breadth.c` |
| `std::config` | `tools/stdlib/config.sg` | `runtime_breadth.c` (INI/TOML subset) |
| `std::hash` | `tools/stdlib/hash.sg` | `runtime_breadth.c` (SHA-256 hex) |
| `std::encoding` | `tools/stdlib/encoding.sg` | `runtime_breadth.c` |
| `std::compress` | `tools/stdlib/compress.sg` | `runtime_breadth.c` (gzip deferred: `STATUS_UNSUPPORTED`) |
| `std::fs` | `tools/stdlib/fs.sg` | `runtime_breadth.c` + `runtime.c` file/dir |
| `std::http` | `tools/stdlib/http.sg` | `runtime/src/net.rs` via existing HTTP C ABI |
| `std::time` (extended) | `tools/stdlib/time.sg` | `runtime_breadth.c` + `runtime.c` |

## `std::net` / HTTP classification

| API family | Classification |
|---|---|
| `net_tcp_*`, `net_udp_*`, `TcpStream`, `UdpSocket` | Stable public |
| `http_get`, `http_post`, `HttpClient` on `std::net` | Compatibility (prefer `std::http`) |
| `http_client_get`, `http_client_post`, `HttpResponse` on `std::http` | Stable public |
| `net_bench_*` | Internal / bench-only |
| TLS / async IO | Unsupported (`STATUS_UNSUPPORTED`) |

## `owned-string-text`

Landed: `tools/stdlib/string.sg` + `runtime_string.c`. Breadth helpers still accept `Buffer` outputs where noted in module docs.
