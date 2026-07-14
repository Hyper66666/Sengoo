# python-hot-path

`python-hot-path` is the reviewed first-party Python interop fixture for the
installed release lane. It keeps the normal `sgpm` package loop green and adds a
host smoke that:

- compiles `src/lib.sg` with an installed `sgc` into emitted LLVM IR;
- reads the generated `.sgreflect.json` sidecar to discover the exported native
  symbol;
- compiles the emitted `.ll` into a shared library with `clang`;
- loads the shared library from Python `ctypes` outside the checkout and
  invokes the reflected scalar hot path.

Run from this directory:

```powershell
sgpm update
sgpm check --locked
sgpm test --locked
sgpm fmt --check --locked
sgpm doc --locked
sgpm build --locked
sgpm run --locked
python python_smoke.py --sgc <path-to-sgc>
```
