## Why

Ship owning CLI, data and SQLite packages for native CLI and Python developers while preserving v0.2 compatibility.

## What Changes

- Implement sgcli 0.1.0 with declarative options, one subcommand and deterministic errors/help.
- Implement sgdata 0.1.0 bounded UTF-8/JSON/CSV and an exact i64 JSON constructor.
- Vendor verified SQLite 3.54.1 and implement sgsqlite owning statements, transactions and diagnostics.
- Verify lifecycle, negative, compatibility and outside-checkout package consumption tests.
- Add the file/CSV index consumer without application manual resource-release calls.

## Capabilities

### New Capabilities

- `official-cli-data-sqlite`: Ship owning CLI, data and SQLite packages.

## Impact

The approved native daily-development program; existing experimental backend scope is unchanged.
