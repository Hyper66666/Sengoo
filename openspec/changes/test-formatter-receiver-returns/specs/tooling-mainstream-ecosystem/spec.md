## ADDED Requirements

### Requirement: Formatting preserves method receiver returns
The formatter SHALL preserve a return statement whose operand starts with self
as one return expression, including the method call operand.

#### Scenario: Forwarding method
- **WHEN** a method returns self.read()
- **THEN** formatted and reparsed source retains a single return with that operand
- **AND** does not create a bare return followed by an unreachable call
