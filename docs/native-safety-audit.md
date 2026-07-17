# Native safety audit

Status: release gate for runtime ABI v1.

## Boundaries

| Boundary | Validation before dereference/call | Negative and lifetime evidence |
| --- | --- | --- |
| Managed Buffer and owned String | generation-tagged handles, length/capacity checks, null rejection | Rust FFI double-free/use-after-free tests; ASan/UBSan native probe; live-handle baselines |
| Generic collections and `Arc<T>` | ABI-v1 descriptor size/alignment/callback validation | descriptor negatives, stale vector handle, exact Drop count, sanitizer probe |
| JSON/config decoders | byte ceilings, null/negative-length checks, parse status channel | retained/generated fuzz, native parser probe, sanitizer probe |
| Registry metadata and archives | compressed/uncompressed/entry ceilings, normalized paths, no links | bounded decoder fuzz, traversal/link/duplicate/checksum tests |
| Async scheduler/reactor | generation/task ownership, bounded queues, panic containment | cancellation/close/deadlock stress, task-panic containment, AsyncFile stale-handle tests |
| Dynamic C FFI | non-null symbols/argv/out pointers, arity 0-8, handle tables | null argv/out/symbol tests, stale/double-close tests, unsupported arity tests |
| TLS/network handles | owned Rust state and stable invalid-handle/status mapping | close/cancel and certificate/hostname failure suites on supported hosts |

## Unwind policy

Rust panics must not cross scheduler or callback ownership boundaries. The
executor wraps poll, cancel, and cleanup callbacks with `catch_unwind`, marks a
failed task, releases capacity, and keeps other workers progressing. Raw native
C calls remain explicitly unsafe: Sengoo validates call shape and ownership,
but it cannot recover from a foreign library that unwinds or violates its
declared ABI.

## Release gate

`.github/workflows/native-safety.yml` runs the split C runtime under Clang
ASan/UBSan with leak detection and runs the Rust runtime tests under ASan. A
missing sanitizer, skipped probe, sanitizer report, leak, panic, or non-zero
probe status fails the job.
