# workspace-doc-loop

`workspace-doc-loop` is a dual-target fixture. Its importable package name is
`workspace_doc_loop`, because Sengoo import identifiers use underscores.
`sgpm doc` documents `src/lib.sg`
while `sgpm build` builds `src/main.sg`; package tests import the package
library through the module map.

```powershell
sgpm update
sgpm check --locked
sgpm test --locked
sgpm fmt --check --locked
sgpm doc --locked
sgpm build --locked
```

The source uses `std::process` to run a foreground command with captured output.
