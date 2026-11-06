# Design

An `if` without else has Unit on its false path even when its true path returns.
Never coerces to the other branch type. Type checking reports incompatible
continuing values before code generation. MIR chooses a result type from a
branch that actually reaches the join and excludes terminated predecessors.

The minimal invalid example is retained in the native compiler regression;
positive cases execute both outcomes of a returning then, returning else and
conditional guard. No user state or language runtime data is migrated.
