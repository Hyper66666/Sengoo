# Fix ready Windows console input polling

Windows nonblocking terminal polling currently returns an empty event after a
single ignored KEYUP, focus or disabled mouse record, even when a key is already
queued. A queued UTF-16 high surrogate likewise delays its ready low surrogate.
Applications can accumulate frame-timer latency while typing. Drain a bounded
batch of ready records in the generic native input boundary. Preserve the
existing Terminal API, event layout, raw-mode ownership and cancellation.

Owner: cursor_tui (terminal runtime and independent polling regressions).
