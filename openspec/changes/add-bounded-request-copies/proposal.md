# Explicit bounded streaming request copies

## Capabilities

Add TCP/TLS byte-stream entry points with an explicit request limit up to 16 MiB,
and a process-wide 64 MiB admission budget for request copies. This provides a
generic prerequisite for larger multimodal requests without moving HTTP or
provider parsing into native threads.

## Non-Goals

No unbounded upload, file streaming, provider integration or media qualification.
Existing entry points keep the 1 MiB request limit and all ABI signatures.
