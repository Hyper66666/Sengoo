## ADDED Requirements

### Requirement: Independent polling client
The SDK SHALL provide a Sengoo stdio MCP client without Sencode configuration,
terminal-control or permission dependencies and without automatic tool replay.

#### Scenario: Long request and cancellation
- **WHEN** a server applies stdin backpressure, emits progress or waits indefinitely
- **THEN** each poll remains bounded and the original monotonic deadline applies
- **AND** cancellation closes and reaps the owned process tree without replaying tools

#### Scenario: Negotiation and errors
- **WHEN** the initialization reply names an unsupported version or a call returns a JSON-RPC error
- **THEN** the client rejects the unsupported version and retains the RPC error envelope
- **AND** initialization cancellation does not emit notifications/cancelled

### Requirement: Bounded Unicode protocol transport
Framing SHALL retain split UTF-8 scalars, reject truncated or oversized input and
drain stderr independently with a bounded retained prefix.

#### Scenario: Fragmented and hostile output
- **WHEN** newline-delimited JSON crosses byte boundaries or a server emits oversized, invalid or stderr-heavy output
- **THEN** valid Unicode survives and failures have explicit numeric status
- **AND** retained stderr and notification queues do not grow without bound

### Requirement: Application-driven tool server
The SDK SHALL implement initialization, tools/list pagination and tools/call
dispatch with explicit schemas and opaque request identity.

#### Scenario: Validated dispatch and output
- **WHEN** a registered tool is called and completed
- **THEN** input validation precedes dispatch and successful structured output satisfies outputSchema
- **AND** tool failures use isError rather than transport failure

#### Scenario: Cooperative cancellation and progress
- **WHEN** a client supplies a progress token or cancels an active call
- **THEN** progress preserves that token and cancellation affects only the matching call
- **AND** completion of a cancelled call emits no response

#### Scenario: Unsupported capability
- **WHEN** a peer requests an unimplemented method or calls before initialization
- **THEN** a JSON-RPC error is returned and no application tool is dispatched

### Requirement: Independent consumption evidence
The SDK SHALL include native examples and tests that run without Sencode state.

#### Scenario: Independent peers
- **WHEN** the Sengoo server is driven by a separate client and the Sengoo client consumes a synthetic server
- **THEN** negotiation, tools, progress, error and cleanup results are recorded separately from real-service evidence

### Requirement: Exact control identity
The SDK SHALL reject NUL in control strings and object keys before narrowing
owned bytes, while preserving opaque IDs and business JSON values losslessly.

#### Scenario: Escaped NUL
- **WHEN** a peer supplies a NUL-suffixed method, version, tool name, cursor, identity or object key
- **THEN** it cannot match its visible prefix or dispatch a different operation
- **AND** opaque string IDs and payload values copied as JSON retain all bytes
