## ADDED Requirements

### Requirement: Explicit and compatible request size limits
The runtime SHALL preserve existing 1 MiB entry points and provide explicit-limit
byte-stream entry points with a maximum of 16 MiB per copied request.

#### Scenario: Legacy callers exceed the default
- WHEN an existing entry point receives more than 1 MiB
- THEN it returns invalid-argument status without starting I/O.

#### Scenario: Explicit caller exceeds its selected limit
- WHEN request length or the supplied limit is invalid
- THEN validation fails before input memory is copied or a worker is admitted.

### Requirement: Aggregate request copy ownership
Copied requests SHALL consume a process-wide admission budget of at most 64 MiB
until their owning worker releases the copy.

#### Scenario: Concurrent copies fill the budget
- WHEN four live workers each own a 16 MiB request copy
- THEN a further nonempty copy is refused without creating a worker or handle.

#### Scenario: Cancelled and failed workers release their copies
- WHEN a worker finishes cancellation, fails to connect, or exits through an error
- THEN its request copy and reservation are released exactly once.

### Requirement: Transport independence and unchanged trust
Explicit-limit plain and TLS streams SHALL copy inputs before returning and
retain existing TLS validation and conservative request-write evidence.

#### Scenario: Caller changes its original large request
- WHEN the caller changes or drops its input after start returns
- THEN a real loopback peer receives the copied original request bytes.
