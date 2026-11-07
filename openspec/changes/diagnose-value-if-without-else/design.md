An omitted else continues to unify with unit. Only a resulting TypeMismatch is
wrapped in the existing structured Diagnostic variant. The label covers the
last statement of the then branch; function checking adds the function name.
Existing if/else and condition mismatch errors keep their variants. Regression
tests assert the exact source range and successful explicit-unit/early-return
compilation. The CLI implementation is verified against the same compiler.
