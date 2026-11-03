# Design

## Ownership of work

- Compiler agent: generated ownership cases, compiler cleanup fixes and sanitizer integration.
- Package agent: incremental index, package API diagnostics and standalone index acceptance.
- Python agent: real batch consumer, resource/cost measurements and Python wheel workflow.
- Parent agent: candidate revision, other workflows, evidence collection, pilot kit and integration.

## Candidate identity

A candidate is a committed revision. Local evidence includes the revision,
working-tree state and tool identity. Remote evidence requires the exact candidate
head, completed successful jobs and required artifacts. Dispatch/PR evidence is
candidate evidence; it does not satisfy the existing stable main-push release
contract. Missing authentication or unexecuted hosts stay explicitly incomplete.

## Bounded application behavior

The index compares bounded validated UTF-8 source content for identical imports.
Changed rows and source metadata update in one transaction; parse or database
failure preserves the previous committed rows. Existing databases migrate by
adding metadata rather than rewriting old entries. Input caps are explicit.

Python measurements separate native computation, conversion and per-call costs.
Explicit live-resource counters verify exercised paths. RSS is observational;
it is not accepted as a stand-alone proof of leak freedom or a portable budget.

## Independent pilot

Prepare checksum-verified candidate assets, participant instructions, environment
capture and observations before invitations. An actual human must supply the
participants and delivery channel. A configured workflow, simulated participant,
or elapsed internal run cannot complete the two-week observation gate.
