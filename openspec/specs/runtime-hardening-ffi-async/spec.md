# runtime-hardening-ffi-async Specification

## Purpose
TBD - created by archiving change runtime-hardening-ffi-async. Update Purpose after archive.
## Requirements
### Requirement: Runtime async and concurrency semantics SHALL be stable and testable

The runtime SHALL define and test scheduling, cancellation, timeout, task
status, error propagation, and resource cleanup semantics for accepted async
and concurrency operations.

#### Scenario: A task times out

- **WHEN** an accepted async runtime operation is configured with a timeout and exceeds it
- **THEN** the task reports a timeout status through the documented result/status path
- **AND** owned runtime resources associated with the task are released or remain reachable through a documented handle state

#### Scenario: Async behavior is unsupported

- **WHEN** a source-level or stdlib async operation is not supported by the current runtime target
- **THEN** the operation returns `STATUS_UNSUPPORTED` or a more specific stable status
- **AND** native linking does not fail because of missing optional runtime symbols

### Requirement: Runtime FFI SHALL be real or explicitly unsupported

Dynamic FFI SHALL either execute through tested host support or report explicit
unsupported/status failures before native link or runtime crashes occur.

#### Scenario: A supported host loads a dynamic symbol

- **WHEN** a supported host loads a dynamic library and calls a symbol with an accepted signature
- **THEN** the runtime validates the call shape before execution
- **AND** success and failure are reported through documented status/result paths

#### Scenario: A host does not support dynamic FFI

- **WHEN** dynamic FFI is requested on an unsupported host or unsupported call shape
- **THEN** the runtime returns `STATUS_UNSUPPORTED` or `STATUS_INVALID_ARGUMENT`
- **AND** it does not emit an unresolved native link dependency

### Requirement: Runtime platform behavior SHALL be documented and covered by tests

Runtime behavior that differs across Windows/POSIX or host environments SHALL be
documented, status-mapped where practical, and covered by tests or accepted
platform skips.

#### Scenario: Process termination differs by platform

- **WHEN** a process, timeout, signal, or termination helper behaves differently on Windows and POSIX
- **THEN** docs describe the difference
- **AND** tests verify the supported behavior or assert the documented unsupported status

#### Scenario: Path and permission behavior differs by platform

- **WHEN** path encoding, symlink, permission, or filesystem metadata behavior cannot be made portable
- **THEN** the runtime returns a stable status category where possible
- **AND** docs identify host-specific behavior

### Requirement: Runtime failures SHALL include panic, backtrace, and debug context

Runtime and stdlib failures SHALL provide enough diagnostic context for users to
identify the failing operation, source location when available, and runtime
state relevant to the failure.

#### Scenario: A runtime panic occurs

- **WHEN** runtime code panics or detects an invariant violation
- **THEN** diagnostics include the failing runtime operation and best available source/call context
- **AND** backtrace or debug context is included when enabled and supported

#### Scenario: A stdlib helper reports an error

- **WHEN** a stdlib helper fails through a runtime bridge
- **THEN** the error can be mapped to a stable status category or `STATUS_UNKNOWN`
- **AND** copyable diagnostics are available when the helper supports them

### Requirement: Runtime handle lifecycle SHALL be validated

Runtime-owned handles SHALL validate handle type, generation, closed state, and
resource ownership where practical before accessing underlying storage.

#### Scenario: A closed handle is reused

- **WHEN** a program closes a runtime-owned handle and then uses it again
- **THEN** the helper returns an invalid-handle or closed-handle status
- **AND** it does not read freed storage

#### Scenario: A wrong handle type is used

- **WHEN** a helper receives a handle from a different runtime domain
- **THEN** the helper returns a stable invalid-handle status
- **AND** no cross-domain storage is accessed

### Requirement: Runtime resource and security boundaries SHALL be explicit

Runtime APIs that interact with commands, paths, network, config parsers, regex, JSON, compression, FFI, or large inputs SHALL document and enforce resource and security boundaries.

#### Scenario: A command contains shell metacharacters

- **WHEN** a process helper receives an argument containing spaces or shell metacharacters
- **THEN** the runtime passes it as a literal argv entry
- **AND** no shell expansion is performed unless a future OpenSpec explicitly adds shell execution

#### Scenario: An input exceeds resource limits

- **WHEN** config, regex, JSON, compression, FFI, network, command-output, Buffer, or String input exceeds documented limits
- **THEN** the runtime returns a stable resource or invalid-argument status
- **AND** tests cover the limit behavior

