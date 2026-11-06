# Design

Only the Windows console branch changes. One native input attempt inspects up
to 64 ready INPUT_RECORD values, skipping records that produce no public event.
It returns immediately for a decoded key/mouse event or bytes ready for the
existing parser. UTF-16 state survives an empty poll. The outer loop still owns
the deadline and cancellation; exhausting the batch yields on a zero timeout
and revisits those controls before any later positive-timeout batch. Repeated
key state and redirected/POSIX byte parsing keep their existing contracts.

Use a hidden real console to inject ignored records, emoji halves, Chinese and
ASCII, then test both direct native calls and a separately compiled Sengoo
Terminal consumer. A 2048-record ignored queue must yield before its end,
honor cancellation, and eventually deliver its trailing key without loss.
The existing input-discard regression retains stale-suffix flushing and fresh
approval-input checks instead of asserting the old polling defect.
