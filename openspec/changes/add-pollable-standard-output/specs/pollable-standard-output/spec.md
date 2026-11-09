## ADDED Requirements

### Requirement: Standard-output writers SHALL provide bounded byte submission

The generic standard I/O module SHALL expose an owning writer for supported
stdout/stderr endpoints. Submission SHALL return promptly with the accepted byte
count or a stable error, preserve byte order and exact bytes, and distinguish
queue acceptance from confirmed native write progress and peer acknowledgement.
Queue plus in-flight storage SHALL remain within configured per-writer and
process-wide bounds. Existing synchronous I/O SHALL retain its behavior.

#### Scenario: A reader stops consuming output

- **WHEN** accepted bytes exhaust the configured writer capacity while the peer
  stops reading
- **THEN** further submission reports backpressure without blocking the caller
- **AND** queued plus in-flight storage stays within the configured capacity
- **AND** input polling and cancellation can continue independently

#### Scenario: Partial and binary writes

- **WHEN** a supported endpoint accepts only part of a submitted Buffer
- **THEN** the caller receives an exact accepted count and can submit the suffix
- **AND** the writer preserves FIFO bytes including NUL and split UTF-8 without
  decoding, newline conversion or automatic replay
- **AND** drained status does not claim peer or durable-storage acknowledgement

### Requirement: Cancellation SHALL preserve ownership until actual quiescence

Cancellation SHALL reject further writes and request termination of pending I/O.
A successful cancellation request SHALL NOT imply worker completion. Queued and
in-flight storage, duplicate handles and worker identity SHALL remain owned
until the operation finishes and the worker is joined. Drop SHALL use the same
cleanup and SHALL NOT detach or forcibly terminate a worker thread.

#### Scenario: Cancellation races with a blocking Windows write

- **WHEN** cancellation arrives before, during or immediately after an issued
  synchronous write
- **THEN** the worker observes terminal intent and issues no subsequent write
- **AND** a cancellation request that initially finds no pending I/O is not
  treated as quiescence
- **AND** quiescence is reported only after actual write completion and join

#### Scenario: Cleanup exceeds the caller's wait deadline

- **WHEN** the worker has not stopped before wait_quiescent reaches its deadline
- **THEN** the call reports timeout and retains a usable owner of all resources
- **AND** no buffers or handles are reclaimed while the worker can access them
- **AND** unknown partial output is recorded without replay or false success

### Requirement: A writer SHALL isolate inherited endpoint ownership

The Windows backend SHALL use a dedicated byte-only worker and non-inheritable
duplicated endpoint without changing or closing the original standard handle.
Workers SHALL NOT access application JSON, protocol state or callbacks. Endpoint
types without verified cancellation and cleanup behavior SHALL be unsupported.

#### Scenario: Closing a supported writer

- **WHEN** a Windows local byte-pipe writer has completed or canceled and is
  dropped
- **THEN** its worker, duplicate endpoint and wakeups are reclaimed after join
- **AND** the original standard handle remains open with unchanged modes
- **AND** no child process is created as an undocumented output implementation

#### Scenario: Unsupported endpoints and platforms

- **WHEN** the constructor receives a console, regular file, message pipe,
  unsupported device or a POSIX endpoint without an implemented verified backend
- **THEN** it returns STATUS_UNSUPPORTED before starting a worker
- **AND** it does not toggle flags on a shared inherited open-file description
- **AND** unverified platforms are not recorded as successful host evidence

### Requirement: Writer resources SHALL be bounded and auditable

Admission SHALL enforce the documented capacity and live-writer limits before
allocation/copying and roll them back on all failure paths. Test diagnostics
SHALL report actual writer, worker-thread, native-handle and byte resource counts.
Generation checks SHALL prevent stale identities from targeting reused slots.

#### Scenario: Repeated failures and cancellations

- **WHEN** native tests repeatedly construct, saturate, fail, cancel and Drop
  writers against slow and broken readers
- **THEN** live writer/thread/handle and queued/in-flight byte counts return to
  their baseline after each quiescent scope
- **AND** OS-level handle/thread counts cross-check internal accounting
- **AND** existing process live-resource counts remain at their baseline

#### Scenario: Invalid or saturated construction

- **WHEN** an invalid length/capacity, exhausted writer slot or worker creation
  failure is injected
- **THEN** construction/submission returns a stable error without accessing an
  invalid source region or leaking reservations, buffers, threads or handles

### Requirement: Protocol adapters SHALL consume generic output capabilities

The MCP stdio adapter SHALL integrate a verified generic writer without adding
MCP-aware native code. Application protocol state and cancellation decisions
SHALL stay in Sengoo, and adapter output SHALL remain bounded and FIFO.

#### Scenario: MCP peer does not read responses

- **WHEN** an independent client fills the server output pipe and then sends
  cancellation or reaches an existing request deadline
- **THEN** the Sengoo host continues polling input and observes cancellation
- **AND** it does not grow an unbounded response queue or reset deadlines
- **AND** cleanup waits for actual writer quiescence before releasing ownership
