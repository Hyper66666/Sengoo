# Explicit detached process launch

The Windows `std::process` API can launch an explicitly requested host without a
launcher-owned process handle or kill Job. The result is plain evidence:

```sg
let command = process_command("C:\\path\\host.exe")?;
command.arg("--background-host")?;
command.env_clear()?;
// Copy only the OS discovery variables and user values that this host needs.
let receipt = command.spawn_detached()?;
let alive = receipt.is_running()?;
let same = process_identity_matches(receipt.pid, receipt.creation_identity)?;
```

`ProcessLaunchReceipt { pid: i64, creation_identity: i64 }` owns no OS resource.
Dropping it never kills the host. Existing `spawn` and `spawn_piped` still own
their child process trees and kill those trees on Drop.
`ProcessHandle.tree_quiescent()` separately reports whether its Windows Job has
zero active processes. Direct-child `reap()` alone is not that evidence; callers
that release workspace or execution locks should verify the whole Job first.
The new tree query returns unsupported on non-Windows in this first version.

The runtime launches suspended, captures the Windows creation FILETIME, then
resumes hidden. Standard input/output/error use a fresh NUL device. An explicit
handle whitelist excludes the caller's redirected streams and arbitrary
inheritable file, pipe or event handles. The builder's environment semantics
are unchanged: applications should clear it and copy an explicit allowlist
before launching a host that must not inherit provider credentials.

If the launcher is in a Windows Job, explicit breakaway from its immediate Job
must succeed. A Job that forbids breakaway returns a permission error. Windows
can retain outer system or supervisor Jobs in a nested hierarchy; the API does
not claim to bypass them. It creates no additional launcher-owned kill Job.

Identity queries compare PID and birth FILETIME before checking exit. A missing
PID, mismatched identity or confirmed exit returns false. Access denied and
other query failures return errors, so callers cannot mistake uncertainty for
death. There is no PID-based termination API here. Non-Windows returns status
8 (unsupported) for the initial implementation.

Applications still own explicit user authorization, durable execution state,
their host's tool-process tree and authenticated IPC. Verify both the IPC peer
PID and the receipt's birth identity before accepting readiness. A timeout or
stale heartbeat does not prove that a host exited, and a receipt is not a
takeover capability. Stop a host through its authenticated application protocol.

Windows x64 evidence: real native probe and Windows AddressSanitizer exercise
Unicode/spaces/quoted argv, cwd/environment copying, caller-handle isolation,
birth mismatch, normal exit, survival after owned launcher exit, refusal inside
a nonbreakaway Job and 24 failed launches with stable OS/native owner counts.
The final focused native/public Sengoo Rust test passed 2/2 in 17.10 seconds on
the same host. Existing piped stdin and native/public terminal-process cases
passed, and the final runtime distribution suite passed 15/15 in 95.57 seconds,
including fresh installed consumers. No other platform or Windows
leak-sanitizer result is implied.
