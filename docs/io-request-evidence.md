# Conservative network write evidence and repeated terminal input

`NetByteStream.request_started() -> Result<bool,i64>` is an additive development
API. It returns true immediately before the worker attempts an application write.
DNS lookup, TCP connect and the TLS handshake leave it false. Query only after a
terminal transport failure when deciding whether no request write was attempted.
False while a worker is running may become true immediately afterward. True is
not delivery or side-effect confirmation; automatic retry decisions belong in
the application. Cancellation does not clear evidence. Closed/invalid handles
return error 3. Runtime ABI 1 and existing stream interfaces are unchanged.

Windows console key records may combine several repeated presses. The terminal
now retains and delivers the reported special-key or UTF-8 repeats, preserving
modifiers. Explicit input discard clears retained repeats as well as OS records.
The retained payload is bounded; it does not implement application key bindings.

Windows verification: `cargo test -p sengoo-runtime byte_stream_request_started`,
`cargo test -p sgc --test net_byte_stream --test terminal_screen`. Native fixtures
exercise actual TCP, rejected TLS handshake and a hidden Windows console. These
results do not qualify POSIX hosts or a live external model provider.
