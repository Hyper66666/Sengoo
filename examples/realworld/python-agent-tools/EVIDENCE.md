# Local Python consumer evidence

## Frozen compiler rerun

The final local rerun uses compiler SHA256
`7f6343885839b8dfbcd9e03509a3da73739c1ee88d87224fd01fcb1ec5fca7d7`,
matching-revision sgpm `c3d589c6d497ac7a4f61e4557c4d0f616b2414338ed3f23ed0ba3aba9ead125a`,
the unchanged formatter/backend, and the final audited native runtime. The host
is DESKTOP-FUAQMFB, Windows AMD64, CPython 3.12.14, on 2026-11-09.

`target/agent-completion/python-final/evidence.json` records an actual new
external locked PEP517 wheel build, source removal, a fresh offline venv install
and all consumer checks. Report SHA256:
`cc8bfa3b016c89e92059ade5e2cd376738b6a6c0b5ea179b7f7b84ca7a6a7a42`.
The wheel's SHA256 is
`ee5ab03663d716ff02b3373e6861fc3f835de0129b4c198d4364a5b22de08144`;
it is retained beside the report. All 200 successful/failing rounds preserve
String, Buffer and bounded JsonDoc counts at `[0, 0, 0]`. Unicode, exact i64,
typed failures and literal/escaped NUL cases pass. The driver also verifies that
runtime, schema, backend, input-source and selected-tool hashes remain unchanged
through the run. This is a rebuilt wheel, not a reuse of the earlier bytes.

CPython 3.11/3.13/3.14, Linux/macOS, public publication and a clean installer VM
remain separate unrun gates. The following section preserves earlier local
development history and its original artifact identities.

## Earlier development observations

Windows x64, DESKTOP-FUAQMFB, CPython **3.12.14**, 2026-11-09. Repository baseline
`ea803f1913f9d08f6393d412b0eb4b1de3055b3e` plus working changes. This is local
experimental acceptance, not a public release or all-platform qualification.

- Compiler `target/agent-parameter-compiler/debug/sgc.exe`, SHA256
  `69a42135586f5fa56debe10f35e51e4423a926463e88c3f3bfa5f5e1f596ddce`.
- Matching sgpm `target/agent-parameter-compiler/debug/sgpm.exe`, SHA256
  `c3d589c6d497ac7a4f61e4557c4d0f616b2414338ed3f23ed0ba3aba9ead125a`.
  Both identify as `0.2.1-dev.0 (ea803f1913f9)`; the package manager was built
  from the frozen source without changing the compiler or backend.
- Formatter SHA256
  `d075a1a0fd51a8e2434650ea542f5f252f526198ff1bf0ba27a7fce3ea7ba814`.
- Unchanged backend source SHA256
  `2b5c21f551a870bd659fcca23029f478d3b9e25eeea7021689a341e97c48d3b3`.
- Wheel `sengoo_agent_tools_example-0.1.0-py3-none-win_amd64.whl`, SHA256
  `7d105849146b16aa53059a8b88cf0e2f47bc5bbc39b71f529433a970b1a60e74`.

`target/python-agent-tools/evidence.json` records the actual repository-external
locked PEP517 build. It copied the source project and schema dependency outside
the checkout, built a real shared library and wheel using the existing backend,
kept the lockfile unchanged, and removed the entire copied source/dependency
tree before fresh offline venv installation. The installed consumer passed.

That first pip invocation used `-I`, which ignores the environment UTF-8 switch;
pip's own Rich logging printed GBK/emoji errors while still installing and
returning success. The original output is retained as
`external-build-with-pip-logging-warning.json`. The test driver now supplies
explicit `-X utf8`. A second **new venv**, using the exact same wheel bytes,
installs without those logging errors and passes the entire consumer again:
`target/python-agent-tools/clean-reinstall/evidence.json`.
The prebuilt mode does not claim another build or source-removal operation.

Both installs disable package-index access and dependency fetching, remove
SENGOO_/SGPM_/PYTHONPATH/backend/native-build environment, and put only the venv
and Windows System32 on PATH. The independent installed consumer confirms no
sgc/sgpm/clang/cargo/rustc tool lookup or backend import, then checks:

- The actual sgschema package accepts/rejects tool inputs and reports diagnostics.
- Chinese/emoji and exact `9223372036854775807` survive canonical JSON.
- Typed Result errors, unknown schema, literal/escaped NUL and byte capacity fail.
- Two hundred rounds of valid canonicalization/reporting plus invalid instance
  and schema calls leave native String, Buffer and bounded JsonDoc counts exactly
  at **[0, 0, 0]** after every round.

The SG source passes the current formatter and the dedicated OpenSpec change
passes strict validation. Temporary projects and venvs are removed by exact
inventoried paths, never recursive shell deletion. No shared backend, compiler,
native implementation, existing package or user configuration was modified by
the consumer. A mismatched older sgpm was rejected before compilation, with that
failure preserved in `toolchain-identity-mismatch.json`; identity checks were
not bypassed.

Reproduce full build and consumption:

```text
python examples/realworld/python-agent-tools/tests/verify.py --compiler path/to/sgc.exe --clang path/to/clang.exe --formatter path/to/sgfmt.exe --sgpm path/to/sgpm.exe --output target/python-agent/evidence.json
```

Add `--wheel path/to/the.whl` for a new offline installation of prebuilt bytes
without starting a compiler. Source builds use relative path dependencies;
registry/sdist publication, fresh installer VM, CPython 3.11/3.13/3.14 and other
platforms remain independent gates. The ABI covers only the documented small
JSON-text/status functions, not whole Agent objects or callbacks.
# Current conditional-lifetime compiler repeat

`target/agent-completion-v2/python/evidence.json` passes an external locked
wheel build, copied-source removal, fresh offline CPython 3.12 installation,
Unicode/exact-i64/errors/NUL and two hundred resource rounds. Compiler SHA256
is `9b1055c5a5cb7f09c016228a44f7427b7519b6a92e69cad5993004831c3aa1e5`;
the wheel SHA256 is
`0f3d21bf9eb6fd69827e07dda330c6cb2f5ac43d4c36fadcaa3189bf1e7aba11`.
All source/tool identity checks pass. Earlier runs below retain their original
compiler and wheel identities rather than certifying these new bytes.

