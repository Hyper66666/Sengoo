# Verification

Windows x64, CPython 3.12.14, 2026-11-09; baseline
`ea803f1913f9d08f6393d412b0eb4b1de3055b3e` plus working changes.
`examples/realworld/python-agent-tools/EVIDENCE.md` links exact source/tool/wheel
hashes and retained command evidence. The 69a421 compiler and matching c3d589
sgpm build the real external locked PEP517 wheel with the unchanged backend.
The copied project and schema sources are removed before the fresh offline
consumer install. The wheel SHA256 is
`7d105849146b16aa53059a8b88cf0e2f47bc5bbc39b71f529433a970b1a60e74`.

`target/python-agent-tools/evidence.json` records that full build and consumer.
The old pip console's Unicode logging issue is retained rather than hidden.
After the driver adds explicit `-X utf8`, the exact same wheel is installed into
another fresh offline venv without logging errors; see
`target/python-agent-tools/clean-reinstall/evidence.json`. This second mode is
correctly labelled prebuilt reinstall and makes no new build claim.

Unicode, exact i64, real schema decisions, typed failures, NUL and byte bounds
pass. Two hundred repeated success/failure rounds return all three native
resource counters to [0,0,0]. No compiler is on runtime PATH, no backend is
imported and no network index is available. Formatting and dedicated strict
validation pass. This closes the local small-ABI example only; whole-Agent ABI,
all supported CPython versions/platforms, sdist/registry/public release and an
installer VM remain outside this evidence.
