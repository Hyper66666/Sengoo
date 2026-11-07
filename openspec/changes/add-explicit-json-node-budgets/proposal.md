# Explicit JSON node budgets

## Capabilities
Add opt-in byte and node/link budgets for structured documents which exceed
4096 nodes before reaching their byte limit. Owner: integration agent.

## Non-Goals
Do not change existing constructors, JSON syntax, depth limit, ABI version or
thread ownership. Do not remove aggregate resource admission.
