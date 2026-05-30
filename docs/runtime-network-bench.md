# Sengoo Runtime Network Benchmark (Go/No-Go)

This document describes the runtime network benchmark used for:

- concurrent connection validation
- broadcast fanout latency validation
- p50/p95/p99 latency collection

## C ABI

- `i64 sengoo_net_bench_run(u32 connections, u32 rtt_messages_per_connection, u32 broadcast_rounds, u32 payload_bytes, u8* report_buffer, usize report_capacity)`
- `i32 sengoo_net_bench_last_error_code()`
- `i64 sengoo_net_bench_last_error_len()`
- `i64 sengoo_net_bench_last_error_copy(u8* buffer, usize capacity)`
- `i32 sengoo_net_bench_last_error_clear()`

## Sengoo stdlib wrapper

`import std::net` also preloads `std::ffi`, so callers can allocate a managed
`Buffer` and pass it to `net_bench_run` or `net_bench_last_error_copy`:

```sg
import std::net;

def main() -> i64 {
    let report = ffi_buffer_new(4096);
    if report.is_err() {
        0
    } else {
        let buffer = report.unwrap_or(Buffer { handle: 0 });
        let copied = net_bench_run(4, 6, 3, 24, buffer);
        buffer.free();
        copied.unwrap_or(0)
    }
}
```

## Report format

`sengoo_net_bench_run` writes a JSON report:

- `connections`
- `rtt_messages_per_connection`
- `broadcast_rounds`
- `payload_bytes`
- `rtt_samples`
- `rtt_p50_us`, `rtt_p95_us`, `rtt_p99_us`
- `broadcast_samples`
- `broadcast_p50_us`, `broadcast_p95_us`, `broadcast_p99_us`

## Error codes

- `0`: success
- `-2601`: invalid argument
- `-2602`: IO error
- `-2699`: internal error

## Notes

- Benchmark runs on loopback TCP (`127.0.0.1`) for reproducibility.
- RTT path: concurrent clients send messages and wait for echoed response.
- Broadcast path: server writes to all connections each round and measures client receive latencies.
