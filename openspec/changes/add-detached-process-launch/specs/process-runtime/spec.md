## ADDED Requirements

### Requirement: Explicit detached launch returns immutable identity evidence
The process module SHALL expose a Windows detached launch receipt containing PID and OS creation identity without a launcher-owned process handle, kill Job or Drop termination action.

#### Scenario: Launcher exits
- **WHEN** an explicitly detached host starts and its launcher exits or closes its own owned Job
- **THEN** the host remains alive under applicable outer OS supervisor policy and its receipt owns no termination action

#### Scenario: Restricted enclosing Job
- **WHEN** the launcher's immediate Job forbids explicit breakaway
- **THEN** launch fails instead of pretending the new process is independent of that Job

#### Scenario: Handle and standard stream isolation
- **WHEN** the caller has redirected standard streams and arbitrary inheritable handles
- **THEN** the detached host receives only fresh NUL standard streams and none of those caller handles

### Requirement: Process liveness validates birth identity
The process module SHALL compare PID and OS creation identity before reporting life and SHALL preserve unknown or denied queries as errors.

#### Scenario: PID reuse or confirmed exit
- **WHEN** a PID has a different creation identity, is absent or has exited
- **THEN** the query returns false without terminating any process

#### Scenario: Query denied
- **WHEN** OS access to identity or liveness is denied
- **THEN** the query returns an error and does not claim that the host exited

#### Scenario: Unsupported host
- **WHEN** this initial API is invoked on a non-Windows host
- **THEN** detached launch and valid identity queries return unsupported

### Requirement: Owned cleanup can verify the whole Windows Job
The process module SHALL expose a fallible owned-process tree-quiescence query distinct from direct-child exit and SHALL report true only when its Windows Job has no active processes.

#### Scenario: Descendant remains after direct exit
- **WHEN** the owned direct child exited but a descendant remains in its Job
- **THEN** tree quiescence remains false until the Job has zero active processes

#### Scenario: Unknown tree state
- **WHEN** Job accounting fails or the host has no supported whole-tree query
- **THEN** the query returns an error instead of certifying resource cleanup
