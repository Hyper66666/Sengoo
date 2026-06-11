# Sengoo Toolchain Archive

This archive contains the Sengoo command-line tools:

- `bin/sgc`
- `bin/sgpm`
- `bin/sgfmt`
- `bin/sglsp`

It also includes the standard library and C runtime bridge under
`share/sengoo/`, so `sgc` can resolve `std::*` imports without a source
checkout or `SENGOO_ROOT`.

## Requirements

Native builds require LLVM/Clang 15 or newer on `PATH`. `sgc build
--emit-llvm` and stdlib import expansion work without native linking.

## Quickstart

```sh
bin/sgc --version
cat > hello.sg <<'EOF'
import std::status;

def main() -> i64 {
    STATUS_OK()
}
EOF
bin/sgc build hello.sg --emit-llvm
```

For installed use, add the archive's `bin` directory to `PATH`.

PowerShell:

```powershell
.\scripts\install.ps1 -Archive .\sengoo-<version>-<target>.zip
.\scripts\install.ps1 -Version <version>
```

POSIX shell:

```sh
scripts/install.sh ./sengoo-<version>-<target>.tar.gz
scripts/install.sh --version <version>
```
