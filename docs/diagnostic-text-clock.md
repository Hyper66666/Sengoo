# Diagnostic text and monotonic time

`string_from_utf8_lossy(buffer, used_len) -> Result<String, i64>` is an explicit
diagnostic boundary. It preserves valid spans and embedded NUL, replacing maximal
ill-formed UTF-8 subsequences with U+FFFD. It does not detect GBK or other legacy
encodings. Negative or beyond-initialized lengths return status 2; input remains
unchanged and reusable. The decoder is pure Sengoo. `string_from_utf8` and
`string_from_buffer` remain strict and return status 20 for malformed UTF-8.

`time_monotonic_ms() -> i64` returns milliseconds with an unspecified origin, or
-1 if unavailable. It uses GetTickCount64 on Windows and CLOCK_MONOTONIC on POSIX,
with no calendar fallback. Compare values only within a live process; do not save
them as dates or restore them after restart. Calendar time and existing elapsed
helpers keep their behavior. Runtime ABI remains 1.

```sengoo
let started = time_monotonic_ms();
// Perform work.
let finished = time_monotonic_ms();
let elapsed = if started >= 0 and finished >= started { finished - started } else { -1 };
```

Regression: `cargo test -p sgc --test text_clock -- --nocapture` builds and runs
the real Sengoo decoder. It compares 69,889 binary inputs with Rust's independent
UTF-8 oracle, verifies strict compatibility and String resource baselines, plus
real monotonic reads and sleep. Local evidence is Windows x64 source-development;
other platforms and installed artifacts require their own validation.

Local verification on DESKTOP-FUAQMFB, 2026-11-05: `text_clock` passed 1/1 in
27.29s. Baseline SHA `d46b3f6cd8d6e770734f98edc57aa67cb931e26a` plus the
recorded dirty source changes; compiler SHA256
`cf52f7a8ea6bdad280844a327beb9452980ee2f723324a3dd6155626f08f093b`.
Both `openspec validate add-diagnostic-text-clock --strict` and
`openspec validate --all --strict` passed (69 items). This is source-development
evidence, not a published release or an unrun platform claim.
