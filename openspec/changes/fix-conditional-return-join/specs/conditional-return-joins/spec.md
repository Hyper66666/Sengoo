## ADDED Requirements

### Requirement: Conditional expressions join continuing types
An omitted else SHALL contribute Unit, and a diverging branch SHALL coerce to
the type of the other branch. Only continuing predecessors SHALL contribute
values to a MIR join.

#### Scenario: Return on the true branch
- **WHEN** an if returns from its true branch and yields an integer otherwise
- **THEN** both native paths return their expected integers without invalid Phi values.

#### Scenario: Conditional early return followed by Unit
- **WHEN** an enclosing conditional combines bool with a false path that can continue with Unit
- **THEN** compilation fails during type checking rather than emitting invalid LLVM.
