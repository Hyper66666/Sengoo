# Verification

Windows x64, DESKTOP-FUAQMFB, 2026-11-09; repository baseline
`ea803f1913f9d08f6393d412b0eb4b1de3055b3e` plus working changes.

`target/agent-temporary-compiler/native-evidence.json` records the original
independent borrowed JsonDoc chain (source SHA256
`2e1f93fdd5ab67da84a6743de1fb8ce08afae37b90a296c9390d4357e1d5a2e5`):
compiler `6507e0e2eff15fd375054334f4e4ff6bccf540431f5f81bd56625f7f0d616252`
leaks and exits 4; compiler
`deb91ea7223b5cf6c41d9fed585db4af73f5a555183470e0c42c9210ee30432d`
passes. This earlier fixture differs from the final expanded loop fixture;
its exact source hash is retained rather than claiming identical sources.

Final compiler `target/agent-unused-compiler/debug/sgc.exe` SHA256
`f7ea03a89c27ccedf8e6f2f8c81fe7ab6008887dd028fd8848102774e2f5dde5`
adds statement-boundary cleanup. Evidence
`target/agent-unused-compiler/final-language-evidence.json` contains exact final
source and executable hashes and successful command output:

- All 18 `try_operator_tests` pass, including consuming receivers, borrowed
  close/free, chain propagation and discarded owned producers.
- All 77 tests selected by `drop` pass, with the pinned clang on PATH.
- `tools/stdlib/tests/temporary_owned_chain.sg` passes. The 20 outer iterations
  contain 20 inner iterations each: 400 iterations discard owning Try, Result,
  Option and Vec values and check JSON live count immediately. Named controls,
  correct borrowed reads, later error propagation, Option unwrap and consuming
  Vec chains also pass with bounded JSON reservations and opaque handles at
  their original baselines. Native SHA256:
  `4ef52f31af51d8fe3d1560438d8ece531acf97054dbee0a8decbd837d8df787d`.
- Independent MCP `tests/unused_owned.sg` passes with the same compiler; its
  exact command is also recorded in `target/sgeval-final-evidence.json`.

The current formatter passes the native fixture. Dedicated strict validation
passes. This is scoped ownership evidence, not the full language test suite.
The old nested explicit-field destructor example still duplicates marker 831;
raw before/after results remain in the earlier native evidence. An unrelated
`Option<JsonDoc>?` expression returning `Option<i64>` still has a type-checking
problem (`target/temporary-chain-option.sg`); Option `unwrap_or` is the tested
native path here. Inactive nonzero owned Result error payloads are outside this
fix. Neither limitation was hidden by editing expectations or raising capacity.
# Conditional borrowed view correction (2026-11-09)

The unchanged Sencode `src/tests_tui.sg` failed at inconsistent frame-comparison
assertions. An independent diagnostic printed identical frames except for the
activity row: `Working` became freed heap bytes. A native counter-first probe
(`tools/stdlib/tests/conditional_borrow_owner.sg`) deterministically reports 101
with the prior 7f634388 compiler, because the branch's temporary String was
already destroyed on entry to its outer consumer.

The corrected compiler is `target/agent-conditional-compiler/bin/sgc.exe`, SHA256
`9b1055c5a5cb7f09c016228a44f7427b7519b6a92e69cad5993004831c3aa1e5`.
Its complete library suite passes 1,178/1,178 with the verified Clang on PATH
(`target/agent-completion/conditional-compiler-tests-final.log`). The original
unmodified TUI suite passes five recorded repetitions; binary identity and full
stdout are in Sencode
`target/agent-sdk-20261109/candidate-6/ui-repeated-evidence.json`. No frame
assertion was removed and no named-value workaround was added to the product.

The native reproduction alternates both branches, nested blocks, later error
propagation and a failing subsequent argument, with exact per-iteration String
baselines. It is part of `discarded_owned_string_verify.py`. New integration
evidence belongs under `target/agent-completion-v2/`; previous 7f compiler
reports retain their historical scope and do not certify this correction.

