## ADDED Requirements

### Requirement: Standard I/O utilities SHALL support synchronous pipeline-style programs
The standard library SHALL provide `std::io` helpers for bounded stdin reads,
stdout writes, stderr writes, and stream flushing.

#### Scenario: A program reads stdin into a managed Buffer
- **WHEN** a Sengoo program imports `std::io`
- **THEN** it can call `io_stdin_read(buffer)` to read up to the buffer capacity
- **AND** it can call `io_stdin_read_line(buffer)` to read up to the buffer
  capacity or through one newline
- **AND** successful reads return `Result<i64, i64>` with the byte count
- **AND** EOF without bytes is a successful read count of `0`

#### Scenario: A program writes exact bytes to stdout and stderr
- **WHEN** a Sengoo program imports `std::io`
- **THEN** it can call `io_stdout_write(data)` and `io_stderr_write(data)`
- **AND** the helpers write exactly the provided string bytes without adding a newline
- **AND** successful writes return `Result<i64, i64>` with the byte count

#### Scenario: A program flushes standard output streams
- **WHEN** a Sengoo program imports `std::io`
- **THEN** it can call `io_stdout_flush()` and `io_stderr_flush()`
- **AND** successful flushes return `Result<bool, i64>` with `true`

#### Scenario: Standard I/O helpers are wired through the stdlib toolchain
- **WHEN** a Sengoo program imports `std::io`
- **THEN** `sgc check`, `sgc build`, and `sgc run` preload the module and its
  declared source dependencies
- **AND** `sglsp` exposes the module's public symbols and signatures
- **AND** `examples/stdlib` contains a runnable synchronous I/O example

#### Scenario: Advanced I/O features remain explicitly deferred
- **WHEN** a future implementation needs async I/O, terminal control, file
  descriptor APIs, or owned-string stdin helpers
- **THEN** it first updates OpenSpec with API shape, portability constraints,
  ownership rules, and tests
