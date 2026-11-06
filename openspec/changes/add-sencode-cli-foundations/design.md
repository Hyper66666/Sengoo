# Design

The Sengoo coordinator owns JSON, SQLite and application state on one thread.
Bounded transport workers own DNS/TCP/TLS and byte queues only. HTTP/SSE/provider
parsing, retry, permissions and scheduling are Sengoo source in Sencode.

Cancellation has an independent control path and owned socket/process shutdown.
Dropping a blocking future is not interpreted as stopping its host thread.
Resource handles are validated, non-Send where applicable, and cleaned once.
Old APIs are retained; newly documented APIs have explicit pending/EOF/error
semantics. No whole-response or whole-process buffering in new streaming paths.

New modules must be wired into sgc, sglsp, installed payload validation and source
distribution. Language changes must include the failing source and regression.

Tests use temporary directories, loopback fixtures and synthetic credentials;
never the user's Sencode database or API keys. Windows is the first application
target; portable runtime tests are kept where implementations exist.
