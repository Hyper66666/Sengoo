# Ownership and sequence

Baseline publication precedes implementation. Integration owner maintains this
program, Sencode adapters, package tooling, independent consumers and evidence.
The model SDK owner modifies packages/sgllm and add-agent-llm-sdk. The MCP owner
modifies packages/sgmcp and add-agent-mcp-sdk. The evaluation owner modifies
tools/sgeval and add-agent-evaluation-tool. No shared-source mutation during
compiler or acceptance source-hash freezes; coordinate native build capacity.

First extract existing protocol implementations into packages with complete
failure, cancellation and memory bounds, then replace CLI bodies with adapters.
Do not declare extraction complete while both copies remain maintained.
Agent extraction follows a poll/action host boundary: provider requests,
permissions and tool effects remain host-owned; durable assistant admission,
tool claiming, fenced result persistence and unknown-outcome checks precede
effects or replay. Keep actual CLI and independent consumer on the same core.

Each phase records sources, tools, host, binary identity, commands, failures and
limits. Independent consumption means manifests/lockfiles outside both source
checkouts with a relative offline registry and explicit versions. Program
completion requires the roadmap's stated proof, not a package skeleton or an
internal import. Real-service measurements and platform claims require actual
runs. Follow-up scenario-dependent extensions remain tracked individually.
