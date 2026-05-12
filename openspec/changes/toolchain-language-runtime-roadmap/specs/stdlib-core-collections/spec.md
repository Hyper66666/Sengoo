## ADDED Requirements

### Requirement: Standard library SHALL provide core collection types
The standard library SHALL provide `Vec<T>` and `HashMap<K,V>` with essential construction, mutation, and query operations.

#### Scenario: Core collections compile and execute
- **WHEN** a Sengoo program creates and uses `Vec<T>` and `HashMap<K,V>` with supported operations
- **THEN** the program type-checks and executes with expected collection behavior

### Requirement: Standard library SHALL provide iterator abstractions and adapters
The standard library SHALL provide an `Iterator` trait with common adapters for mapping, filtering, and collection workflows.

#### Scenario: Iterator adapters chain correctly
- **WHEN** iterator adapter methods are chained on a collection iterator
- **THEN** the chain type-checks and produces expected transformed outputs

### Requirement: Standard library SHALL provide complete Option and Result ergonomics
The standard library SHALL provide practical and consistent APIs for `Option<T>` and `Result<T,E>` handling.

#### Scenario: Option and Result workflows are expressive
- **WHEN** a program composes map/and_then/match-style flows on `Option` and `Result`
- **THEN** the flows type-check and preserve error/value propagation semantics
