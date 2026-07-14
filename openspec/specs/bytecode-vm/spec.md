# bytecode-vm Specification

## Purpose
Record that a production Sengoo bytecode VM was evaluated and cancelled after
the maturity-program value review. The experimental scalar `SGB1` prototype is
not a compatibility or support promise.

## Requirements
### Requirement: Production bytecode VM work SHALL remain cancelled without a new go decision

The project SHALL NOT claim production bytecode-VM support until a replacement
OpenSpec re-opens the value review with new evidence.

#### Scenario: Tooling exposes experimental bytecode target
- **WHEN** `sgc build/run --target bytecode` is used
- **THEN** documentation identifies the path as an experimental prototype
- **AND** the portable capability matrix does not list it as production Supported

### Requirement: Hostile bytecode SHALL NOT be treated as a trusted format
Experimental `SGB1` artifacts have no compatibility promise and MUST NOT be
accepted as a stable interchange format.

#### Scenario: A future production format is proposed
- **WHEN** a new bytecode format is introduced
- **THEN** it receives its own versioned magic/ABI and verifier design
- **AND** it does not inherit compatibility from prototype `SGB1` version 1
