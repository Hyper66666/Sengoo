# Design

UTF-8 validation and maximal ill-formed subsequence replacement are implemented
in Sengoo over initialized Buffer bytes. Valid spans are copied in bulk, preserving
embedded NUL and the explicit byte length. U+FFFD replaces malformed subsequences;
invalid prefixes, overlongs, surrogate encodings and values beyond U+10FFFF remain
rejected by strict constructors. The maximum output allocation is three times
input length with checked arithmetic. Caller input is neither changed nor freed.

Time uses GetTickCount64 on Windows and CLOCK_MONOTONIC on POSIX. The origin is
unspecified. Failure returns -1; no calendar clock substitutes for elapsed time.
Existing Unix time and elapsed helpers retain their ABI and behavior. Consumers
must compare values from the same live process and handle an unavailable clock.

Tests compare 69,889 binary inputs with Rust's independent UTF-8 decoder, verify
String resource baselines, strict-constructor compatibility, NUL and initialized
prefix boundaries, monotonic reads and real sleep. Local Windows evidence does
not claim unrun POSIX or distribution-platform validation.
