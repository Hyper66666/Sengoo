# Admission and ownership

Validate lengths and limits before reading input memory. Reserve aggregate
request-copy capacity before copying, behind the existing handle/worker admission
gate. Rejected calls allocate no request copy, worker or handle. The copy uses
fallible allocation and a native RAII owner, released after its worker exits,
including connect failure, cancellation, panic and thread-creation failure.

The deliberately conservative 64 MiB budget remains held for the whole worker
lifetime, even after the request is sent. Four requests of at most 16 MiB can
coexist; worker/handle and response-queue limits remain independent. Cancellation
does not report reclaimed request bytes before the worker actually drops them.

Both explicit-limit plain/system-TLS and custom-CA TLS paths preserve trust,
nonblocking polling and request_started semantics. Native code transports bytes;
Sengoo remains responsible for complete serialized-request size preflight.
