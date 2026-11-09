# Design

Use the actual consumes-receiver result for user methods. Explicit `.drop()`
continues to dispatch a destructor. Names free/close are release operations
only on the established legacy handle types whose borrowed-looking ABI already
means explicit native release. A custom `close(&mut self)` or `free(&mut self)`
does not mark the owner moved. Apply the same predicate in both method-expression
and already-lowered method-call paths so one layer cannot silently restore the
old name-only heuristic.

The MCP client's bounded process/JSON/Buffer counts must return to baseline
after scope exit, while genuine legacy handle release cannot double-destroy.
The standalone borrowed-close fixture is owned by the MCP SDK change and used
as an independent compiler regression.

The old ownership-composition test combines explicit destructor calls and
custom borrowed free/close methods that only print a marker. Its assumption
that borrowed method names suppress field Drop conflicts with this signature-
based contract. Retain its raw results separately rather than changing expected
output merely to claim a green suite. The older explicit nested destructor
duplication is tracked independently and not silently declared fixed.
