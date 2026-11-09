# Pollable Agent MCP SDK

## Why
Sencode's MCP implementation is coupled to its terminal and cannot be consumed
by an independent Sengoo Agent or used to write a tool server.

## Capabilities
Extract its bounded framing, pipe draining and process ownership into `sgmcp`
0.1.0; add an application-driven stdio server, explicit protocol negotiation,
tool paging, structured output, progress, schema checks, deadlines and cancel.

## Non-Goals
Remote MCP, OAuth, resources, prompts, sampling, elicitation, MCP task extensions,
public registry hosting, permissions/sandbox policy and side-effect replay.

One MCP owner edits `packages/sgmcp` and this change. The integration owner owns
CLI adapters, package catalog/distribution and shared `sgschema`.
