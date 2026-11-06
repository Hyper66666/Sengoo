## ADDED Requirements

### Requirement: Clipboard reads SHALL return bounded owned snapshots

`std::terminal` SHALL expose image availability, owned UTF-8 text, and an owned
JSON array of original CF_HDROP paths. Reading SHALL NOT clear or replace
clipboard contents. Text SHALL be limited to 64KiB, file count to 32 and JSON
output to 1MiB. Other platforms SHALL report unsupported. Callers SHALL invoke
clipboard reads only for an explicit paste action.

#### Scenario: Unicode text and original image paths
- **WHEN** an explicit paste reads Unicode text or several dropped file paths
- **THEN** returned Strings own exact UTF-8 values independent of future clipboard changes
- **AND** file paths come from a single snapshot without creating project copies

### Requirement: Clipboard image export SHALL be bounded and exclusive

PNG, CF_DIBV5, CF_DIB and CF_BITMAP SHALL convert to PNG using Windows system
components. Inputs/output SHALL not exceed 64MiB, either dimension SHALL not
exceed 8192 and pixel count SHALL not exceed 16Mi. Caller output limits SHALL
be enforced before creating a file. Existing paths SHALL never be overwritten.
NUL or malformed UTF-8 destination paths SHALL fail before clipboard access.

#### Scenario: Source lifetime and repeated exports
- **WHEN** valid DIB, PNG or bitmap data is exported repeatedly
- **THEN** PNG output has the original dimensions and all owned resources are released
- **AND** a clipboard-owned bitmap remains protected until the GDI+ image is disposed

#### Scenario: Rejected image or destination
- **WHEN** data exceeds limits, a destination exists, or a write fails
- **THEN** the operation returns an explicit status and existing files remain unchanged
- **AND** no partial newly created output or clipboard lock is retained

### Requirement: Automated clipboard evidence SHALL preserve user state

Deterministic tests SHALL use owned clipboard fixtures and real image conversion.
Real clipboard tests SHALL create a private window station and desktop first.
Isolation denial SHALL report unavailable and SHALL NOT fall back to the
interactive clipboard or count as a successful OS clipboard integration run.

#### Scenario: Isolation unavailable
- **WHEN** private station creation is denied
- **THEN** the test exits 77 without inspecting or replacing interactive clipboard data
