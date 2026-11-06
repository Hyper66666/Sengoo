# Explicit detached process launch evidence

## Capabilities
Add a generic Windows detached launch receipt to std::process so an explicitly
requested application host can survive its launcher exit. Return PID plus OS
creation identity; identity queries preserve access errors. Existing owned
spawn and spawn_piped retain their kill-on-close behavior.

## Non-Goals
No agent scheduler, automatic background task, PID-only takeover, arbitrary
process termination or bypass of external supervisor Jobs is introduced.
Non-Windows hosts initially return unsupported. Runtime ABI stays 1.

## Boundaries
Native code owns only Windows creation, quoting, environment application and
identity queries. Sengoo exposes a plain receipt without process ownership.
The application owns approval, authenticated IPC handshake, bootstrap deadline,
durable state and its own tool-process tree. The source owner also owns isolated
native and Sengoo regression tests; application integration is a sibling gate.
