# Local development evidence

Windows x64, DESKTOP-FUAQMFB, 2026-11-09. Baseline Sengoo
`ea803f1913f9d08f6393d412b0eb4b1de3055b3e` plus working changes.
Runtime mode: source-development; native optimization level 0.

- Compiler `target/agent-unused-compiler/debug/sgc.exe`, SHA256
  `f7ea03a89c27ccedf8e6f2f8c81fe7ab6008887dd028fd8848102774e2f5dde5`.
- Formatter `target/agent-formatter/debug/sgfmt.exe`, SHA256
  `d075a1a0fd51a8e2434650ea542f5f252f526198ff1bf0ba27a7fce3ea7ba814`.
- Public executable `target/sgeval-final.exe`, SHA256
  `308d12387d99915561afb3457ce84fd14ff19c71a26debc69aaeeb3b786bec8f`.
- Native regression `target/sgeval-native-final.exe`, SHA256
  `a2d956cc132b975f9eb3ef62822e9e96e7b52b643026157a55e3044fe5a80806`.
- `target/sgeval-final-evidence.json` records exact source hashes, compiler,
  binaries and command outputs. `target/sgeval-final-offline.json` records the
  seven public offline check groups. Earlier pre-NUL evidence is retained as
  `target/sgeval-before-nul-evidence.json`; it is not the final candidate.

The final driver copied the manifest, relative lockfile and declared sources to
a private temporary directory outside the checkout. A real `sgpm build --locked`
succeeded with the compiler above and left the lockfile unchanged. It then ran
the built program with only Windows System32 on PATH and with all `SENGOO_` and
`SGPM_` overrides removed. No compiler or source tree was required at runtime.
This is a repository-external source-build/runtime check, not an installer VM.

The native test repeats successful and failing paths 20 times. Bounded JSON
live documents and reserved bytes return to their initial baseline each time.
The public CLI passes all seven check groups:

1. Recursive credential and literal redaction, Unicode, CRLF and exact i64.
2. Byte-identical deterministic reports and an absent recorded-command sentinel.
3. Failed assertions return exit 1 with a failed report.
4. Invalid JSON/UTF-8/version/sequence return exit 2 without a success report.
5. Unknown assertions, partial input and preexisting output fail safely.
6. Line capacity, missing paths and report overwrite are rejected.
7. Raw and escaped NUL in nested values, keys, literal secrets, assertion keys,
   paths and expected values is rejected before narrowing strings. Secrets
   before/after NUL cannot be accidentally retained or compared as a prefix.

All three SG files pass the formatter check. Dedicated strict OpenSpec
validation passes. The independent discarded-owning-Try native regression
also passes with the same compiler; see the ownership change evidence.

Reproduce the public check after selecting the pinned native toolchain:

```text
sgpm build --manifest-path tools/sgeval/Sengoo.toml --locked
python tools/sgeval/tests/offline.py tools/sgeval/target/debug/sgeval.exe --report target/sgeval-offline.json
```

Direct compiler builds additionally require
`SENGOO_MODULE_MAP=sgeval=<absolute tools/sgeval/src/lib.sg>` and
`--runtime-mode source-development`. Pin clang and the source runtime according
to current development evidence. The local final driver is
`target/agent-unused-compiler/sgeval_final_verification.py`.

Only synthetic local fixtures were used. No live API, fresh VM, other-platform,
release, installer or full Agent SDK acceptance is claimed. Temporary ownership
bugs found during implementation have independent compiler fixes and native
regressions; naming owned locals in this tool alone was never their proof.
