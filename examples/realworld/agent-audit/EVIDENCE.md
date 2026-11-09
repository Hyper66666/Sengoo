# Verification scope

Current Windows development acceptance passes with compiler
`9b1055c5a5cb7f09c016228a44f7427b7519b6a92e69cad5993004831c3aa1e5`:
`target/agent-completion-v2/foundations/evidence.json` covers all thirteen
scenarios and exact String/Buffer/JSON/network resources;
`target/agent-completion-v2/examples/evidence.json` verifies the exact-version
offline registry, external locked build and execution after source removal.
The earlier failures below are retained historical evidence, superseded by
these reports. No resource expectation or protocol assertion was weakened.

Implementation uses Sengoo, `sgllm`, `sgschema` and existing generic file
snapshots. Live model quality/cost, fresh-VM installation and non-Windows hosts
remain outside this local development acceptance.

On Windows x64, `target/agent-audit/preflight-protocol.json` records eleven
successful synthetic cases before the stricter whole-program resource gate was
added: two concurrent targets, three protocols, immutable reads, rejected
unselected/traversal/NUL tools, Responses continuation, exact-key echo redaction,
malformed HTTP200, HTTP500, cancellation and conflicting tool identities.
The native compiler was the earlier formatted-source `6b40efa1…` compiler.
This is functional preflight, not current-source final acceptance.

`scripts/verify_agent_foundations.py` subsequently built the current example
with compiler SHA256
`69a42135586f5fa56debe10f35e51e4423a926463e88c3f3bfa5f5e1f596ddce`.
The first complete audit succeeded semantically but failed its new whole-program
resource check: +6 String and +94 Buffer handles, with JsonDoc/opaque counts at
baseline. The failed report and native output remain in
`target/agent-foundations-20261109/`. No counters or expectations were relaxed.
Minimal generic argument/stream reproductions are being fixed in the compiler;
the final candidate must pass all thirteen current cases and exact counters.

The current native test waits for workers to exit and checks String, Buffer,
JsonDoc, opaque, network handles/workers and request reservations after both
success and failure. Tests never read real credentials or call paid providers.

After that gate passes, `scripts/verify_agent_examples.py --example audit`
must also verify checksummed offline-registry dependency resolution, external
locked check/build and copied executable operation after source removal.
Live model quality/cost, fresh-VM installation and non-Windows behavior remain
separate checks; no such success is implied by loopback fixtures.
