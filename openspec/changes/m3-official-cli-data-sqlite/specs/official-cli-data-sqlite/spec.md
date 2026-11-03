## ADDED Requirements

### Requirement: The toolchain SHALL ship owning cli, data and sqlite packages

The official packages SHALL provide the following versioned APIs with verified resource ownership.

#### Scenario: Stage deliverables are reviewed

- **WHEN** this stage is considered for completion
- **THEN** the following implementation and evidence obligations are satisfied:

- Implement sgcli 0.1.0 with declarative options, one subcommand and deterministic errors/help.
- Implement sgdata 0.1.0 bounded UTF-8/JSON/CSV and an exact i64 JSON constructor.
- Vendor verified SQLite 3.54.1 and implement sgsqlite owning statements, transactions and diagnostics.
- Verify lifecycle, negative, compatibility and outside-checkout package consumption tests.
- Add the file/CSV index consumer without application manual resource-release calls.

#### Scenario: A SQLite transaction scope exits without committing

- **WHEN** a SQLite transaction scope exits without committing
- **THEN** its changes are rolled back and its owned resources are released exactly once.
- **AND** existing compatible v0.2 projects SHALL retain their documented behavior.
