## ADDED Requirements

### Requirement: Clipboard text SHALL be a generic owning UTF-8 boundary
`terminal_copy_text` SHALL borrow String and use its full owned byte length.
Windows SHALL copy strictly validated UTF-8 to CF_UNICODETEXT with a temporary
invisible owner window, transferring memory only on success. Embedded NUL and
input above 64KiB SHALL be rejected before touching the clipboard. Other targets
SHALL report unsupported. No TUI selection or agent state SHALL enter the bridge.

#### Scenario: Supplementary Unicode and a failed copy
- **WHEN** Chinese and supplementary characters are copied
- **THEN** the bridge supplies their exact UTF-16 clipboard representation
- **AND** every failed conversion/open/set path releases memory and helper state

#### Scenario: Automated ownership verification
- **WHEN** tests cannot preserve all interactive clipboard formats
- **THEN** their clipboard/window calls use an isolated fixture
- **AND** fixture results are not represented as a real OS clipboard integration
