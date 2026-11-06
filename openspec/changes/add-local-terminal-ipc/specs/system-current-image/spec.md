## ADDED Requirements

### Requirement: Current executable SHALL use the OS current-image path
`std::system::current_executable_string()` SHALL return an owned UTF-8 path to
the current process image. Resolution SHALL use the operating system process
image API and SHALL not depend on the working directory, PATH or argv0. The
root integrator owns implementation and its independent regression test.

#### Scenario: Unicode image path and unrelated working directory
- **WHEN** an executable is started from a Unicode path with an unrelated cwd
- **THEN** the API returns the executable image path with valid Unicode
- **AND** existing system path APIs remain compatible
