## ADDED Requirements

### Requirement: Small typed Python tool-validation boundary
The consumer SHALL use actual Sengoo schema validation through supported typed
Python bindings without claiming full Agent object ABI or executing a tool.

#### Scenario: Valid and invalid JSON tool parameters
- **WHEN** Python supplies schema and argument text
- **THEN** the Sengoo package returns precise validation diagnostics or a typed status error
- **AND** Unicode and exact i64 survive successful canonicalization

#### Scenario: NUL, capacity and repeated cleanup
- **WHEN** literal/escaped NUL, oversized inputs or repeated successful/failing calls occur
- **THEN** invalid inputs fail explicitly before string narrowing
- **AND** native String, Buffer and bounded document counts remain at baseline

### Requirement: Offline wheel consumer independence
The complete example SHALL build through the existing locked Python backend
and install as a platform wheel without runtime compiler or source access.

#### Scenario: Fresh isolated installation
- **WHEN** a wheel built outside the checkout is copied away and all copied SG sources/dependencies are deleted
- **THEN** a new venv installs it offline and imports/runs it with no Sengoo/backend/compiler environment or native tool on PATH
