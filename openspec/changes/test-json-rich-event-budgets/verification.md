# Windows x64 native evidence, 2026-11-07

Host: `DESKTOP-FUAQMFB`. Baseline revision:
`86c6f87975646b91c9cf2d7d069ac06ee4243b9b`; existing local changes preserved.
Compiler SHA256:
`6507e0e2eff15fd375054334f4e4ff6bccf540431f5f81bd56625f7f0d616252`.

Built `tools/stdlib/tests/json_event_metadata.sg` with the pinned compiler,
`--runtime-mode source-development`, force rebuild and optimization zero.
The executable SHA256 is
`f6d3ccb6699ec4f59976aed782ca8ec86d63547705fcde1c707dbc6521c8f105`.
All assertions passed: 600 rich objects; legacy error 10; explicit budget
round trip; insufficient explicit budget error 13; malformed JSON error 10;
twenty iterations returning live-document and reservation counts to baseline.

Machine-readable executable hashes and output are recorded in the adjacent
CLI's `target/provider-event-20261107/independent-json-evidence.json`. The same
evidence runs old and repaired Sengoo provider probes on a synthetic rich
completion: old fails with 10, repaired succeeds. These checks are independent
of a provider account or real user data.

The language reference explains the explicit/legacy boundary. OpenSpec strict
validation passed for this change and all 74 items. No compiler, runtime ABI,
legacy limit or numeric error-code behavior changed. This focused JSON result
does not supersede the earlier known explicit-field-destructor failure or
claim complete language-wide/platform validation.
