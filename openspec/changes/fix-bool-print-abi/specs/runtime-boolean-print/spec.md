## ADDED Requirements

### Requirement: Boolean print calls SHALL preserve runtime ABI 1
The compiler SHALL zero-extend MIR boolean values to i64 before invoking
runtime stdout or stderr boolean print functions. Runtime signatures remain
unchanged and printed values SHALL agree with boolean control flow.

#### Scenario: False struct fields follow negative status arithmetic
- **WHEN** a program prints a status, true and false boolean fields to both sinks
- **THEN** native execution prints the corresponding true/false words at O0 and O2
- **AND** emitted runtime print calls use i64, never i1.
