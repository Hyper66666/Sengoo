## ADDED Requirements

### Requirement: Canceled tasks SHALL stop at the next await point

Canceling a spawned task SHALL be cooperative and observable: the task runs
no user code past its next await point, reaches the canceled terminal
status, and releases its resources through normal future cleanup.

#### Scenario: A pending task is canceled before resuming

- **WHEN** a program calls `cancel_task` on a spawned task that has not
  completed
- **THEN** the task does not execute user code past its next await point
- **AND** `task_status` for that id reaches the canceled state (`3`)
- **AND** child futures owned by the canceled frame are dropped and their
  reactor interest registrations are removed

#### Scenario: Awaiting a canceled task surfaces a stable status

- **WHEN** a program awaits a spawned future whose underlying task was
  canceled
- **THEN** the await resolves to `STATUS_CANCELED() == 19` instead of
  blocking forever
- **AND** a task that already completed stays completed and is not demoted

### Requirement: A consuming select variant SHALL cancel its losers

`select_cancel` SHALL accept two to eight homogeneous future operands,
return the first ready branch's value, and deterministically cancel and
drop every losing branch before returning, while the existing non-canceling
`select` semantics remain unchanged.

#### Scenario: Losers are canceled when a winner is ready

- **WHEN** a program awaits `select_cancel` and one branch becomes ready
- **THEN** that branch's value is returned
- **AND** every losing branch is canceled and dropped before
  `select_cancel` returns
- **AND** no loser subsequently completes, runs user code, or retains a
  reactor interest registration

#### Scenario: A spawned-task loser transitions to canceled

- **WHEN** a losing operand is a spawned task's future
- **THEN** the underlying task reaches the canceled status per the task
  cancellation contract

#### Scenario: Arity and homogeneity are enforced

- **WHEN** a program uses `select_cancel` with fewer than two or more than
  eight operands, or with heterogeneous result types
- **THEN** compilation fails with the same stable diagnostic family as the
  existing `select` arity and type errors

#### Scenario: The non-canceling select is unchanged

- **WHEN** a program uses the existing `select`
- **THEN** losing branches are not canceled and are dropped through normal
  future cleanup exactly as previously specified

### Requirement: Process waits SHALL be cancellable with prompt resolution

The generation-checked `ProcessHandle` SHALL offer a cancellation-capable
wait that resolves promptly when the process is killed or the wait is
canceled, with stable status mapping and unchanged existing lifecycle
operations.

#### Scenario: A waited-on process is killed

- **WHEN** a task waits on a process through the cancellation-capable wait
  and the process is killed
- **THEN** the wait resolves within the documented 250 ms prompt bound with
  `STATUS_CANCELED() == 19` rather than blocking until the timeout expires

#### Scenario: Normal exit and timeout keep existing semantics

- **WHEN** the process exits normally or the timeout elapses
- **THEN** the wait returns the exit code or `STATUS_TIMEOUT` respectively
- **AND** stale or closed handles map to `STATUS_INVALID_HANDLE` per the
  existing lifecycle

#### Scenario: Both host families are covered

- **WHEN** the cancellable wait tests run on Windows and POSIX CI hosts
- **THEN** kill-during-wait resolves within the documented prompt bound on
  both
