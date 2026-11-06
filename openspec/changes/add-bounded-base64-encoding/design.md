# Design

Validate maximum, initialized input extent and distinct source/output handles
before conversion. Require enough output capacity for `4 * ceil(length / 3)`.
Invalid maximum, negative/uninitialized length or aliasing returns status 2;
input above its explicit cap or insufficient output capacity returns status 13.

Clear output after validation. Empty input succeeds with zero output without
dereferencing a native input pointer. Encode at most 65535 bytes per call into
one 87380-byte owned scratch Buffer. This chunk size is divisible by three, so
only the final chunk can contain padding. Append exact encoded lengths with
the checked borrowed Buffer API. Scope Drop releases the scratch on success
and early error. Output may be partial after a conversion failure; callers
must discard it and may not send partial data.

This is ordinary RFC4648 Base64 of the complete binary range, including NUL;
it is not independent padded chunks. Native code and runtime ABI do not change.
The old 65536-byte maximum and error remain independently tested. The CLI
keeps references and hashes in durable history and generates Base64 only in
the ephemeral, explicitly bounded request conversion.

The dedicated native SG regression checks small and empty vectors, legacy
65536/65537 boundaries, 65537 zero bytes with exact final padding, initialized
input, invalid maxima, input/output bounds and embedded NUL. A separate CLI
loopback fixture compares a real >1-MiB PNG with Python's Base64 reference in
Chat Completions, Responses and Anthropic Messages requests. These are local
Windows development checks, distinct from real-provider or release evidence.
