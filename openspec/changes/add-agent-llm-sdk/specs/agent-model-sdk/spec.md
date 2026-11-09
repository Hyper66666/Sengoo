## ADDED Requirements

### Requirement: Standalone shared provider implementation
The sgllm package SHALL provide three native streaming model protocols using
Sengoo HTTP/SSE parsing and shall depend only on general standard-library APIs.

#### Scenario: Independent consumer
- **WHEN** an external project locks sgllm and supplies its own endpoint and credentials
- **THEN** it can compile and stream all three protocols without any Sencode state, UI or source module
- **AND** Sencode can consume the same implementation through thin adapters

### Requirement: Complete tool and usage results
The SDK SHALL expose canonical tool calls only after a complete provider turn
and HTTP body have passed validation, while preserving usage and target scope.

#### Scenario: Fragmented output
- **WHEN** UTF-8, SSE records and tool JSON arrive split across arbitrary chunks
- **THEN** text, call identity, arguments and usage are reconstructed consistently
- **AND** incomplete, invalid or conflicting output cannot authorize tools

#### Scenario: Premature result
- **WHEN** an application requests a final result before network framing ends
- **THEN** the result is rejected without cancelling the still-running request

### Requirement: Explicit limits and recovery ownership
The SDK SHALL validate bounded I/O configuration and expose classified failures
without replaying requests or tools, changing providers or printing diagnostics.

#### Scenario: HTTP success with invalid stream
- **WHEN** HTTP200 contains malformed or incomplete provider output
- **THEN** failure reports HTTP200 separately from the parser stage and safe reason
- **AND** the failed job becomes terminal rather than returning success

#### Scenario: Error and cancellation
- **WHEN** HTTP500, idle timeout or explicit cancellation occurs
- **THEN** the original status is preserved and no implicit retry occurs
- **AND** releasing jobs reclaims workers, handles and request byte reservations

#### Scenario: Configuration denial
- **WHEN** a request exceeds its configured serialized byte bound or has invalid queue, timeout or credential framing
- **THEN** it fails before any worker sends its request
- **AND** images require an explicit capability declaration from the caller
