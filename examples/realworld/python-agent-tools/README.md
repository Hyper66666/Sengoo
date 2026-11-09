# Python Agent tool validation

A small Python wheel backed by Sengoo's real `sgschema` package. It validates
JSON tool arguments and produces deterministic JSON diagnostics or canonical
argument text. It executes no tool, command, Agent loop, model or credential API.

```python
import json
from sengoo_agent_tools import canonical_arguments, require_valid_tool

schema = json.dumps({"type": "object", "properties": {"path": {"type": "string"}},
                     "required": ["path"], "additionalProperties": False})
arguments = json.dumps({"path": "中文/notes.md"}, ensure_ascii=False)
require_valid_tool(schema, arguments)
print(canonical_arguments(schema, arguments))
```

| Export | Return |
| --- | --- |
| `validation_report(schema_json, arguments_json)` | JSON text with valid/stage/pointer/keyword |
| `require_valid_tool(schema_json, arguments_json)` | True or `SengooError` |
| `canonical_arguments(schema_json, arguments_json)` | Validated JSON text preserving exact i64 |

The native diagnostic counters expose current String, Buffer and bounded JSON
handle counts for the repeated-call fixture. They grant no execution capability.
Only `&str`, `String`, `bool`, `i64` and `Result<_,i64>` cross the compiler's
typed binding bridge. This example does **not** export AgentKernel, callbacks,
SQLite handles or an entire Agent object ABI.

Inputs are bounded to 32 KiB each, 4096 nodes and depth 32. Malformed JSON and
unsupported schema fail explicitly. Invalid instance arguments produce status
2, invalid schema status 10 and explicit capacity status 13. Literal NUL is
rejected by Python argument conversion; escaped NUL in JSON keys or values is
rejected recursively in Sengoo before string narrowing. No path grants, sandbox
or permission decision is inferred from successful schema validation.

Source builds require the current matching Sengoo 0.2.1-dev toolchain, native
compiler and `sengoo-python` backend. Wheel consumers require only supported
CPython 3.11–3.14 on the matching platform. Installation of the prebuilt wheel
never fetches a compiler or runs a native build.

The source project uses a relative path dependency during development. Retain
its generated `Sengoo.lock`; the Python backend always invokes sgpm with
`--shared --locked`. The integration owner separately qualifies registry
bundles and release platforms.

The reproducible external gate is `tests/verify.py`: copy the project and schema
package outside the checkout, build a real platform wheel through PEP 517, delete
the copied sources, create a fresh venv, install offline and run only the wheel
under a PATH without native tools or source/backend overrides. Tests exercise
Unicode, exact i64, errors/NUL and 200 rounds of successful/failing resource
cleanup. Current host results belong in `EVIDENCE.md` after execution.
