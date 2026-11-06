# Design

ProcessCommand.spawn_detached returns ProcessLaunchReceipt { pid,
creation_identity }. The receipt is plain data and has no process handle, Job
or Drop action. A supplied Buffer carries the native creation FILETIME before
ResumeThread; the public wrapper returns copied integer evidence.

Windows creation is suspended, hidden and Unicode. A fresh NUL device is the
only whitelisted inherited handle, serving as stdin/stdout/stderr; no caller
stdio, pipe, event, file or provider environment is implicitly added. Builder
environment inheritance semantics remain explicit and unchanged. In an
enclosing Job, CREATE_BREAKAWAY_FROM_JOB must succeed; a forbidden immediate Job
is an error. Nested outer supervisor Jobs may still apply under Windows rules.
The runtime creates no new launcher-owned kill Job and claims no escape from
external supervisors. Failures before ResumeThread terminate/wait the suspended
child and close all local resources.

process_identity_matches and receipt.is_running open query/synchronize rights,
compare creation FILETIME and then query exit. Missing PID, changed birth or
confirmed exit returns false. Access denied or query failures remain errors.
No API terminates a process by PID. Non-Windows returns unsupported.

Consumers must authenticate IPC and compare peer PID and birth before ready;
the application host owns its own piped engine/tool tree. Launcher timeout
alone does not prove host death. The application implements its own explicit
bootstrap/stop protocol without changing this generic runtime boundary.
