## ADDED Requirements

### Requirement: Terminal BackTab bytes and native Shift Tab SHALL share a normalized event

The common bounded terminal byte decoder SHALL map the standard three-byte
BackTab sequence `ESC [ Z` to a key event with code 9 and Shift modifier bit 1,
matching native Windows Shift+Tab. Ordinary Tab SHALL remain key code 9 with no
modifier. This mapping SHALL NOT change the terminal ABI or implement an
application permission policy.

#### Scenario: Standard BackTab arrives complete or split
- **WHEN** `ESC [ Z` arrives in one read or across either internal byte boundary
- **THEN** it produces exactly one key event with code 9 and modifier 1
- **AND** incomplete prefixes remain pending within the existing escape deadline
- **AND** the following ordinary Tab and text byte retain their own events

#### Scenario: BackTab bytes occur inside a bracketed paste
- **WHEN** a valid bounded bracketed paste contains `ESC [ Z`
- **THEN** the sequence remains literal paste content and does not become a key event

#### Scenario: Windows native Shift Tab is compared with the byte decoder
- **WHEN** the dedicated probe injects Shift+Tab and ordinary Tab into its owned hidden Windows console
- **THEN** the normalized codes and modifiers agree with the common byte decoder
- **AND** the public Sengoo consumer observes the same byte-stream mapping without enabling raw mode
