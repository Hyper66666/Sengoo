## ADDED Requirements

### Requirement: Byte streams SHALL expose conservative application write evidence
`NetByteStream.request_started()` SHALL return `Result<bool,i64>`. The value
becomes true before the first application write attempt and remains true until
close. DNS, TCP and TLS handshake activity SHALL NOT set it. Invalid or closed
handles SHALL report STATUS_INVALID_HANDLE. Retry policy remains application code.

#### Scenario: A connection fails before sending application bytes
- **WHEN** a stream reaches terminal connect or TLS handshake failure
- **THEN** request_started returns false while the handle remains open
- **AND** a successful or attempted application write reports true even after
  cancellation or an ambiguous disconnect
- **AND** false on an active stream is not a promise about future writes.
