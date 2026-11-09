## Design and validation

- [x] Define seed identity, explicit stage0/runtime boundaries and the SG foundation contract.
- [x] Separate M1 behavior ownership from the self-hosting umbrella and future runtime/backend work.
- [x] Run `openspec validate add-selfhost-foundations --strict` (passed on Windows x64, 2026-11-09).

## Seed and entry implementation

- [x] Add `selfhost/bootstrap.lock` with exact seed/source/target/tool/runtime identities and documented normalization.
- [x] Add the stage0 seed runner with pre-execution identity checks and explicit source-development mode.
- [x] Add an SG entry with deterministic probe/self-test and invalid-invocation handling.
- [x] Verify missing/tampered seed rejection without selecting an unchecked PATH fallback.

## SG compiler foundation

- [x] Implement owned source buffers/file IDs and validated byte spans.
- [x] Implement deterministic symbol interning and checked resolution.
- [x] Implement checked node IDs and append-only arena growth with explicit bounds.
- [x] Verify empty/UTF-8/source-error, invalid spans/IDs, growth/limit and duplicate/distinct symbol behavior.
- [x] Verify repeated lifecycles return tracked resources to baseline.

## Integration evidence

- [x] Build and run SG foundation programs using the locked stage0 inputs; record exact commands and native identities.
- [x] Record the full source revision, host, source/tool hashes, raw output and limitations in ignored evidence.
- [x] Run `openspec validate --all --strict` and leave later compiler/native self-hosting tasks open.

Windows x64 DESKTOP-FUAQMFB, 2026-11-09: `selfhost/verify.py` passed 44 steps in
`target/selfhost/m1-run2/evidence.json` (SHA256
`827fb8a74a900961c43e60223f7672b54343fdc378a37d79e48cb49dd9aa0d94`).
Full base `fc1a686a2953adcbd14731cc6f93270cfb9cf34d` plus hashed M1 sources;
547 inputs recorded, no source changes. The native core includes 4096 nodes,
256 KiB input and 100 exact-resource rounds. Full strict validation passed
94 items. Seed-rejection preflight separately replaced a deliberately stale
pass report with a failed report before executing the mismatched seed.
