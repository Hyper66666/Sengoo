## ADDED Requirements

### Requirement: Method names cannot consume custom borrowed owners
A custom method taking a borrowed receiver SHALL preserve caller ownership
irrespective of whether its name is close or free.

#### Scenario: Owning client close only stops work
- **WHEN** an owning client calls `close(&mut self)` to stop and reap a process
- **THEN** scope exit still releases its process handle and owned fields once
- **AND** the method name alone cannot suppress caller Drop

#### Scenario: Explicit runtime release
- **WHEN** an established native handle calls its real close/free release method or an owner invokes its explicit destructor
- **THEN** the compatible explicit-release behavior remains and automatic cleanup does not duplicate the release
