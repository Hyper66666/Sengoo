## ADDED Requirements

### Requirement: Sengoo SHALL support async and await syntax
The language SHALL support asynchronous function definitions and await expressions with type-checked async return values.

#### Scenario: Awaiting an async call is type-checked
- **WHEN** an async function result is awaited in a valid async context
- **THEN** the awaited expression type-checks as the async output type

### Requirement: Runtime SHALL support coroutine-based async execution
The runtime SHALL provide a coroutine-capable execution model to schedule and drive async tasks.

#### Scenario: Multiple async tasks can be scheduled
- **WHEN** a program spawns multiple async tasks
- **THEN** the runtime progresses each task according to scheduler policy until completion or cancellation
