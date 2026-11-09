# Generational freelists and frame admission

String/Buffer/opaque slot entries retain their current generation and alive checks.
Freed indices form an intrusive freelist; failed allocation leaves the list
intact, stale generations cannot retrieve/release a replacement, and duplicate
free/take never enqueues the same slot twice. Internal slot layout is private.
Documented thread ownership stays unchanged. Retained 20000-string/buffer
workloads verify allocation cost, stale values and exact live-count cleanup.

`write_frame` validates handle/arguments/TTY, checks actual dimensions before
writing and again afterwards. False means geometry mismatch and caller cache
invalidation; errors remain negative native status / SG Result. No input is
flushed or consumed. Windows buffer-resize records invalidate a same-size frame
when an away-and-back transition was missed by a dimension poll. Fullscreen
entry/restoration and non-frame `write` retain their behavior.
