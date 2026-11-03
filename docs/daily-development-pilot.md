# Independent daily-development pilot

Status: **deferred by the owner, not started** (2026-11-04). Internal tests do not satisfy this gate. Invite 3–5
independent users only when the owner triggers the pilot; retain observations
for two weeks. No invitations have been sent by this implementation.

Give each participant the same candidate archive/wheel, checksum manifest,
installation guide and project instructions. Record its full source revision,
platform and Python version. Ask the participant to work from the public
documentation without unpublished setup assistance.

The native task is to install, run doctor, create a project, add the official
packages, import a CSV into persistent SQLite storage, restart and query it,
add a test, generate docs and deliver the executable to a clean directory.
The Python task is to install a precompiled wheel in a new environment without
Sengoo/Rust/clang, process numeric arrays and Unicode strings, and handle a
documented error. Source wheel construction is a separate developer task.

Record an observation for every blocker, workaround and missing instruction:

| Date | Participant alias | Candidate SHA | Host | Task | Expected/observed behavior | Severity | Workaround | Issue/fix evidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |

Use anonymous aliases; do not publish personal details without consent. A
blocker prevents finishing a task using the documented flow. Close it only
after the participant or an independently reproducible rerun verifies the fix.
The final gate requires zero open blockers and explicit documentation for
remaining limitations. Record start/end dates and actual outcomes; do not
pre-fill success or replace the observation period with CI results.
