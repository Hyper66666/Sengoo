# Development checks

Windows x64, DESKTOP-FUAQMFB, 2026-11-09. Baseline Sengoo
ea803f1913f9d08f6393d412b0eb4b1de3055b3e plus the program changes.

`target/agent-schema-20261109/final-evidence.json` pins the formatted source, binary
and compiler identities. The native suite passes Unicode scalar counts, exact
i64 enum/bounds, nested pointers, missing/extra fields, unsupported definitions,
NUL keys/type names, full owned-string enum comparison after embedded NUL,
invalid API limits and explicit capacity rejection. Twenty repeated
error/success calls restore String and bounded JsonDoc counts to their baseline.

Compiler SHA256: f7ea03a89c27ccedf8e6f2f8c81fe7ab6008887dd028fd8848102774e2f5dde5.
Formatter: target/agent-formatter/debug/sgfmt.exe SHA256
d075a1a0fd51a8e2434650ea542f5f252f526198ff1bf0ba27a7fce3ea7ba814.
The schema library's two SG files pass its format checks. MCP consumes the
validator for input and structured output contracts. The MCP final NUL/lifecycle
suite passes against its pinned source. `target/agent-preview-locked-consumer.json`
proves three-SDK external locked offline resolution/check/build and execution of
a copied binary after removing source/registry and development-tool PATH, with
formatted-source compiler SHA256
6b40efa12c374adbc48dd7f543ddc83820e00140fa87689b3cb3a35479039c7a.

This is an experimental parsed-value subset, not complete JSON Schema or raw
wire duplicate detection. No new runtime/native code was added. These checks
do not imply Linux/macOS, live-provider or installed-distribution qualification.

Additional resource audit found an owning String comparison path not covered by
the original integer mismatch loop: after 20 scalar enum comparisons, live
String count increased by 20. Semantic comparison (including NUL suffix bytes)
was correct, but the resource check failed. It remains recorded in
`target/agent-schema-20261109/enum-owned-resource-failure.json`; the task is not
complete until the consuming method/argument lifetime is corrected and rerun.

The regression is now closed by the generic legacy-parameter ownership fix:
compiler `69a42135586f5fa56debe10f35e51e4423a926463e88c3f3bfa5f5e1f596ddce`
builds and runs `packages/sgschema/tests/schema.sg` successfully, including the
twenty String-enum/NUL comparisons with exact String and JsonDoc baselines.
`target/agent-foundations-20261109/evidence.json` records source/tool hashes and
the passing `schema-build`/`schema-native` steps. The combined report remains
failed because the separate audit consumer still exposes networking resource
leaks; that does not invalidate its completed schema steps or close that other
requirement. The formatter checks also pass. No platform or live-provider claim
is extended by this result.

The final completion toolchain reruns this suite successfully with compiler
`7f6343885839b8dfbcd9e03509a3da73739c1ee88d87224fd01fcb1ec5fca7d7`
and runtime C
`eaa900898e670775f8019a8ddc363c714861db6d9887f6c04d04cb97575995d5`.
The combined schema/audit report now passes all seven steps, including the
complete thirteen-case audit resource gate. Its exact report SHA256 is
`025b673e26110a03e7eaac6d705f281e5468dc2fb30e8a24bcdebaf9fb0f7066`,
at `target/agent-completion/foundations/evidence.json`. Schema binary SHA256:
`9b2a883f45b680b748d479537470a6b99f2df28218a49b753325498abbd5769d`.
