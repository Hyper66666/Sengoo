## ADDED Requirements

### Requirement: Bounded owned byte streams SHALL keep application protocols in Sengoo

`std::net` SHALL offer an owned, non-Send/non-Sync `NetByteStream` with
`net_stream_start`, `read_into`, `cancel`, `close`, and automatic Drop cleanup.
The native worker SHALL perform only DNS, TCP, verified TLS, copied initial
request writes, and raw byte reads. HTTP framing, SSE, provider and agent logic
SHALL remain application Sengoo code. Existing network interfaces SHALL remain
compatible.

#### Scenario: Incremental read from synchronous Sengoo

- **WHEN** a synchronous Sengoo function starts multiple independent streams
- **THEN** start and read return without waiting for DNS, TLS or network data
- **AND** read reports pending, positive byte count, EOF, or stable STATUS error
- **AND** `Buffer.used_len()` matches the positive byte count and is zero otherwise
- **AND** received bytes retain HTTP headers and transfer framing verbatim

### Requirement: Managed buffers SHALL support checked raw-byte framing

`Buffer.get_u8`, `consume` and `index_of_bytes` SHALL operate on initialized
bytes without requiring UTF-8. Invalid ranges or handles SHALL return stable
errors without mutation. Managed append SHALL check source bounds and support
self-overlap. `append_copy` SHALL borrow its source for repeated copying.

#### Scenario: A network fragment splits Unicode and includes NUL

- **WHEN** framing code searches and consumes arbitrary fragments in a Buffer
- **THEN** incomplete UTF-8 and embedded NUL remain unchanged until decoding
- **AND** byte access never reads uninitialized capacity or a stale handle
- **AND** search returns an absolute offset or -1, with empty needles matching start

### Requirement: Byte stream queues and worker counts SHALL be bounded

At most eight live handles and eight active workers SHALL be admitted. Initial
requests SHALL be copied before start returns and limited to 1 MiB. Each queue
SHALL accept a capacity from 4096 through 1048576 bytes; a full queue SHALL
backpressure only its own worker. Connect deadlines SHALL accept 1..60000 ms
and idle deadlines 1..600000 ms. Invalid bounds SHALL return invalid-argument;
capacity exhaustion SHALL return resource-exhausted.

#### Scenario: One full queue does not block three other TLS streams

- **WHEN** four verified TLS streams are active and one queue fills
- **THEN** the other three independently deliver their fragmented response bytes
- **AND** the blocked queue never exceeds its configured capacity

### Requirement: Cancellation and deadlines SHALL have explicit resource semantics

Cancel SHALL discard queued bytes, expose STATUS_CANCELED (19), wake queue waits
and shut down without joining. Close SHALL be idempotent; Drop SHALL close.
Connect/TLS deadlines SHALL expose STATUS_TIMEOUT (11). Buffered bytes SHALL
precede network errors; backpressure SHALL not count as idleness. Uninterruptible
OS DNS SHALL retain its bounded worker slot until resolution returns while
cancel/deadline results remain observable.

#### Scenario: Cancel handshake or queue backpressure

- **WHEN** a stream is canceled during TLS handshake or a full-queue wait
- **THEN** a subsequent read reports 19 without waiting for peer shutdown
- **AND** normal local-network workers and handles return to zero after cleanup

#### Scenario: Stalled TLS handshake reaches its deadline

- **WHEN** a peer accepts TCP but never completes TLS
- **THEN** the stream exposes 11 before the peer closes its connection

#### Scenario: Resolver remains blocked after cancellation

- **WHEN** the OS resolver has not returned when cancel or deadline occurs
- **THEN** the read result is observable immediately
- **AND** the worker slot remains reserved until resolution returns
- **AND** documentation does not claim portable immediate resolver termination

### Requirement: TLS trust SHALL remain verified and connection-local

TLS SHALL validate the certificate chain and requested hostname using system
trust. `net_stream_start_ca` MAY add one copied DER root for that connection;
it SHALL neither alter system trust nor disable verification. Requests and
authorization data SHALL not be logged by transport code.

#### Scenario: Local TLS fixture uses an explicit root

- **WHEN** the caller supplies the fixture CA and the matching hostname
- **THEN** verified TLS transmits the exact authorized Unicode JSON request
- **AND** an untrusted certificate reports 15 and a wrong hostname reports 16
