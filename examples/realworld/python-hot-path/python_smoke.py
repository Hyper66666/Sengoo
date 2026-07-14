from __future__ import annotations

import argparse
import ctypes
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Build the python-hot-path Sengoo fixture with reflection metadata and invoke it through ctypes."
    )
    parser.add_argument("--sgc", help="Path to the sgc binary. Defaults to SGPM_SGC or sgc on PATH.")
    parser.add_argument(
        "--package-dir",
        default=str(Path(__file__).resolve().parent),
        help="Path to the python-hot-path package directory.",
    )
    parser.add_argument(
        "--work-dir",
        help="Optional directory for emitted LLVM IR, reflection metadata, and shared library output.",
    )
    return parser.parse_args()


def find_program(explicit: str | None, env_key: str | None = None, fallback: str | None = None) -> str:
    if explicit:
        return str(Path(explicit).resolve())
    if env_key:
        value = os.environ.get(env_key)
        if value:
            return str(Path(value).resolve())
    if fallback:
        resolved = shutil.which(fallback)
        if resolved:
            return resolved
    raise FileNotFoundError(f"unable to resolve required program: explicit={explicit!r} env={env_key!r} fallback={fallback!r}")


def run_checked(command: list[str], cwd: Path | None = None) -> None:
    completed = subprocess.run(
        command,
        cwd=str(cwd) if cwd else None,
        text=True,
        capture_output=True,
        check=False,
    )
    if completed.returncode != 0:
        raise RuntimeError(
            "command failed:\n"
            f"cwd={cwd}\n"
            f"cmd={' '.join(command)}\n"
            f"stdout:\n{completed.stdout}\n"
            f"stderr:\n{completed.stderr}"
        )


def reflection_sidecar_for(artifact: Path) -> Path:
    return Path(f"{artifact}.sgreflect.json")


def shared_library_path_for(work_dir: Path) -> Path:
    if sys.platform == "win32":
        return work_dir / "python_hot_path.dll"
    if sys.platform == "darwin":
        return work_dir / "libpython_hot_path.dylib"
    return work_dir / "libpython_hot_path.so"


def object_path_for(work_dir: Path) -> Path:
    if sys.platform == "win32":
        return work_dir / "python_hot_path.obj"
    return work_dir / "python_hot_path.o"


def discover_native_symbol(sidecar_path: Path) -> str:
    metadata = json.loads(sidecar_path.read_text(encoding="utf-8"))
    for module in metadata.get("modules", []):
        for symbol in module.get("symbols", []):
            short_name = symbol["symbol"].rsplit("::", 1)[-1]
            if short_name == "hot_path_mix":
                native_symbol = symbol.get("native_symbol") or short_name
                if not native_symbol:
                    raise RuntimeError("reflection metadata entry for hot_path_mix did not expose a native symbol")
                return native_symbol
    raise RuntimeError(f"reflection metadata missing hot_path_mix entry in {sidecar_path}")


def compile_shared_library(clang: str, llvm_ir_path: Path, native_symbol: str, shared_library_path: Path) -> None:
    object_path = object_path_for(shared_library_path.parent)
    run_checked([clang, "-Wno-override-module", "-c", str(llvm_ir_path), "-o", str(object_path)])

    link_commands: list[list[str]]
    if sys.platform == "win32":
        link_commands = [
            [clang, "-shared", str(object_path), f"-Wl,/EXPORT:{native_symbol}", "-o", str(shared_library_path)],
            [clang, "-shared", str(object_path), "-Wl,--export-all-symbols", "-o", str(shared_library_path)],
        ]
    else:
        link_commands = [[clang, "-shared", str(object_path), "-o", str(shared_library_path)]]

    last_error: RuntimeError | None = None
    for command in link_commands:
        try:
            run_checked(command)
            return
        except RuntimeError as err:
            last_error = err
    assert last_error is not None
    raise last_error


def invoke_hot_path(shared_library_path: Path, native_symbol: str) -> int:
    library = ctypes.CDLL(str(shared_library_path))
    function = getattr(library, native_symbol)
    function.argtypes = [ctypes.c_longlong, ctypes.c_longlong]
    function.restype = ctypes.c_longlong
    lhs = 6
    rhs = 7
    actual = int(function(lhs, rhs))
    expected = lhs * 7 + rhs * 11 + 5
    if actual != expected:
        raise RuntimeError(
            f"ctypes invocation returned {actual}, expected {expected} from symbol {native_symbol}"
        )
    return actual


def main() -> int:
    args = parse_args()
    package_dir = Path(args.package_dir).resolve()
    source_path = package_dir / "src" / "lib.sg"
    if not source_path.is_file():
        raise FileNotFoundError(f"missing Sengoo source file: {source_path}")

    sgc = find_program(args.sgc, env_key="SGPM_SGC", fallback="sgc")
    clang = find_program(None, fallback="clang")

    if args.work_dir:
        work_dir = Path(args.work_dir).resolve()
        work_dir.mkdir(parents=True, exist_ok=True)
    else:
        work_dir = Path(tempfile.mkdtemp(prefix="sengoo-python-hot-path-")).resolve()

    llvm_ir_path = work_dir / "python_hot_path.ll"
    run_checked(
        [
            sgc,
            "build",
            str(source_path),
            "--output",
            str(llvm_ir_path),
            "-O",
            "2",
            "--emit-llvm",
            "--reflect=on",
            "--reflect-symbol",
            "hot_path_mix",
        ],
        cwd=package_dir,
    )

    sidecar_path = reflection_sidecar_for(llvm_ir_path)
    if not sidecar_path.is_file():
        raise FileNotFoundError(f"missing reflection metadata sidecar: {sidecar_path}")
    native_symbol = discover_native_symbol(sidecar_path)

    shared_library_path = shared_library_path_for(work_dir)
    compile_shared_library(clang, llvm_ir_path, native_symbol, shared_library_path)
    actual = invoke_hot_path(shared_library_path, native_symbol)
    print(
        f"python smoke ok: sgc={sgc} clang={clang} sidecar={sidecar_path.name} symbol={native_symbol} result={actual}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
