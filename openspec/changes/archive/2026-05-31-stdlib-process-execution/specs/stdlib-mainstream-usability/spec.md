## ADDED Requirements

### Requirement: Process utilities SHALL support synchronous shell-free child execution
The standard library SHALL provide `std::process` helpers for running a child
executable directly with zero through three explicit string arguments.

#### Scenario: A program runs a child executable and reads its exit code
- **WHEN** a Sengoo program calls `process_run(executable)` or a fixed-arity
  `process_run_1` through `process_run_3` helper
- **AND** the host starts and waits for the executable successfully
- **AND** the child exits normally
- **THEN** the helper returns an ok-shaped `Result<i64, i64>` containing the
  child exit code
- **AND** a nonzero child exit code remains a successful process-run result

#### Scenario: Arguments remain literal child argv entries
- **WHEN** a program passes an argument containing spaces or shell
  metacharacters to a fixed-arity process helper
- **THEN** the runtime passes that value as one literal child argument
- **AND** the runtime does not interpret the value as shell syntax

#### Scenario: Process execution inherits standard streams
- **WHEN** a program runs a child executable
- **THEN** the child inherits the current process stdin, stdout, and stderr
- **AND** the helper blocks until the child exits

#### Scenario: Invalid or failed execution returns an error-shaped result
- **WHEN** the executable path is empty, a used raw argument pointer is
  missing, the argument count is outside zero through three, startup fails,
  waiting fails, or the child does not exit normally
- **THEN** the helper returns an error-shaped result

#### Scenario: Advanced process management remains explicitly deferred
- **WHEN** a future implementation needs arbitrary-length argv, implicit shell
  commands, stream capture, pipes, cwd or environment overrides, background
  handles, timeouts, signals, cancellation, or async execution
- **THEN** it first updates OpenSpec with API shape, portability constraints,
  security constraints, lifecycle semantics, and tests
