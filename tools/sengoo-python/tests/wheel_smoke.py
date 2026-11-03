"""Build outside the checkout, or consume prebuilt wheels without build tools.

Both phases run on each platform in python-wheels.yml. The verification phase
also runs on all supported CPython versions and can run on a clean machine.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


def build(directory):
    from sengoo_python import backend
    repository = Path(__file__).resolve().parents[3]
    example = repository / "examples/realworld/python-batch"
    directory = directory.resolve()
    directory.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="sengoo-external-projects-") as temporary:
        for suffix in ("a", "b"):
            project = Path(temporary) / ("batch-" + suffix)
            shutil.copytree(example, project, ignore=shutil.ignore_patterns("target", "build", "__pycache__", "dist"))
            pyproject = project / "pyproject.toml"
            source = pyproject.read_text(encoding="utf-8").replace('name = "sengoo-batch-example"', f'name = "sengoo-batch-{suffix}"').replace('module = "sengoo_batch"', f'module = "sengoo_batch_{suffix}"')
            pyproject.write_text(source, encoding="utf-8")
            native_source = project / "src/lib.sg"
            native_source.write_text("import batch_helper;\n" + native_source.read_text(encoding="utf-8") + f"\npub def module_identity() -> i64 {{ {1 if suffix == 'a' else 2} }}\npub def dependency_value(value: i64) -> i64 {{ helper_increment(value) }}\n", encoding="utf-8")
            pyproject.write_text(pyproject.read_text(encoding="utf-8").replace('exports = [', 'exports = ["module_identity", "dependency_value", '), encoding="utf-8")
            manifest = project / "Sengoo.toml"
            manifest.write_text(manifest.read_text(encoding="utf-8") + '\n[dependencies]\nbatch_helper = { path = "deps/helper", package = "batch-helper" }\n', encoding="utf-8")
            helper = project / "deps/helper"
            (helper / "src").mkdir(parents=True)
            (helper / "native").mkdir()
            (helper / "Sengoo.toml").write_text('[package]\nname = "batch-helper"\nversion = "0.1.0"\nedition = "2026"\n[lib]\npath = "src/lib.sg"\n', encoding="utf-8")
            (helper / "src/lib.sg").write_text('extern "C" { fn batch_native_increment(value: i64) -> i64; }\npub def helper_increment(value: i64) -> i64 { batch_native_increment(value) }\n', encoding="utf-8")
            (helper / "native/helper.c").write_text('long long batch_native_increment(long long value) { return value + 1; }\n', encoding="utf-8")
            subprocess.run([os.environ.get("SENGOO_SGPM", "sgpm"), "update", "--manifest-path", str(project / "Sengoo.toml")], check=True)
            previous = Path.cwd()
            try:
                os.chdir(project)
                subprocess.run([sys.executable, "-m", "pip", "wheel", "--no-build-isolation", "--no-deps", "--wheel-dir", str(directory), "."], check=True)
                # A source archive must retain manifest/lock/source so an
                # offline frontend can reconstruct exactly the same project.
                sdist = backend.build_sdist(str(directory))
                rebuilt = Path(temporary) / ("rebuilt-" + suffix)
                rebuilt.mkdir()
                subprocess.run([sys.executable, "-m", "pip", "wheel", "--no-build-isolation", "--no-deps", "--wheel-dir", str(rebuilt), str(directory / sdist)], check=True)
            finally:
                os.chdir(previous)


def clean_environment(executable):
    environment = dict(os.environ)
    for variable in list(environment):
        if variable.startswith(("SENGOO_", "SGPM_", "DYLD_", "LLVM_")) or variable in (
            "PYTHONPATH", "PYTHONHOME", "CARGO_HOME", "RUSTUP_HOME", "LD_LIBRARY_PATH",
            "LIB", "LIBPATH", "INCLUDE", "CC", "CXX", "CL", "CFLAGS", "LDFLAGS",
        ):
            environment.pop(variable)
    environment["PATH"] = str(executable.parent)
    environment["PIP_NO_INDEX"] = "1"
    environment["PIP_DISABLE_PIP_VERSION_CHECK"] = "1"
    environment["PYTHONUTF8"] = "1"
    if os.name == "nt":
        environment["PATH"] += os.pathsep + str(Path(os.environ["SystemRoot"]) / "System32")
    return environment


def verify(directory, report_directory, iterations=20000):
    wheels = sorted(directory.resolve().glob("*.whl"))
    if len(wheels) != 2:
        raise RuntimeError(f"expected two independently built module wheels in {directory}")
    with tempfile.TemporaryDirectory(prefix="sengoo-wheel-consumer-") as temporary:
        root = Path(temporary)
        environment = root / "venv"
        subprocess.run([sys.executable, "-m", "venv", str(environment)], cwd=root,
                       env=clean_environment(Path(sys.executable)), check=True)
        executable = environment / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
        environment_vars = clean_environment(executable)
        subprocess.run([str(executable), "-I", "-m", "pip", "install", "--no-index", "--no-deps", "--only-binary=:all:", *map(str, wheels)],
                       cwd=root, env=environment_vars, check=True)
        example = Path(__file__).resolve().parents[3] / "examples/realworld/python-batch"
        for name in ("verify.py", "batch_cli.py", "observe.py", "benchmark.py"):
            shutil.copyfile(example / name, root / name)
        checks = '''
import sengoo_batch_a as batch
import sengoo_batch_b as second
import ctypes
from pathlib import Path
import importlib
import json
import runpy
import shutil
import sys
for tool in ("sgc", "sgpm", "cargo", "rustc", "clang", "clang-cl", "gcc", "cl"):
    assert shutil.which(tool) is None, f"build tool still on consumer PATH: {tool}"
assert not any("sengoo_python" in name for name in sys.modules)
for module in (batch, second):
    runpy.run_path("verify.py")["verify"](module.__name__)
assert second.owned_text("second runtime") == "second runtime"
assert batch.echo_text("first runtime") == "first runtime"
assert batch.module_identity() == 1
assert second.module_identity() == 2
assert batch.dependency_value(41) == 42
assert second.dependency_value(-1) == 0
for _ in range(1000):
    assert batch.affine([2.0], 3.0, 1.0) == [7.0]
    assert second.affine([4.0], 2.0, 0.0) == [8.0]
for module in (batch, second):
    directory = Path(module.__file__).parent
    native = next(path for path in directory.glob("_native.*") if path.suffix in (".dll", ".so", ".dylib"))
    library = ctypes.PyDLL(str(native))
    try:
        getattr(library, "sengoo_string_free_status")
    except AttributeError:
        pass
    else:
        raise AssertionError("runtime symbol leaked from the module export surface")
descriptor_path = Path(second.__file__).parent / "_bindings.json"
original = descriptor_path.read_text()
changed = json.loads(original)
changed["identity"] = "0" * 64
descriptor_path.write_text(json.dumps(changed))
try:
    importlib.reload(second)
except ImportError:
    pass
else:
    raise AssertionError("mismatched ABI identity was accepted")
finally:
    descriptor_path.write_text(original)
print("Two independent Python modules passed without build tools")
'''
        checks += f'''
observation = runpy.run_path("observe.py")["observe"](["sengoo_batch_a", "sengoo_batch_b"], iterations={iterations}, checkpoint=max(1, {iterations} // 10))
Path("resources.json").write_text(json.dumps(observation, ensure_ascii=False, indent=2) + "\\n", encoding="utf-8")
costs = runpy.run_path("benchmark.py")["benchmark"]("sengoo_batch_a", samples=5)
Path("call-cost.json").write_text(json.dumps(costs, ensure_ascii=False, indent=2) + "\\n", encoding="utf-8")
'''
        consumer_script = root / "consumer.py"
        consumer_script.write_text(checks, encoding="utf-8")
        subprocess.run([str(executable), "-I", str(consumer_script)], cwd=root, env=environment_vars, check=True)
        report_directory.mkdir(parents=True, exist_ok=True)
        for name in ("resources.json", "call-cost.json"):
            shutil.copyfile(root / name, report_directory / name)
        evidence = {"schema_version": 1, "candidate_sha": os.environ.get("CANDIDATE_SHA", os.environ.get("GITHUB_SHA")),
                    "python": sys.version, "offline_install": True, "toolchain_on_path": False,
                    "source_project_available": False,
                    "wheels": [{"filename": wheel.name, "sha256": hashlib.sha256(wheel.read_bytes()).hexdigest()} for wheel in wheels]}
        (report_directory / "consumer.json").write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    selection = parser.add_mutually_exclusive_group(required=True)
    selection.add_argument("--build-dir", type=Path)
    selection.add_argument("--verify-dir", type=Path)
    parser.add_argument("--report-dir", type=Path, default=Path("python-wheel-evidence"))
    parser.add_argument("--iterations", type=int, default=20000)
    args = parser.parse_args()
    if args.build_dir:
        build(args.build_dir)
    else:
        verify(args.verify_dir, args.report_dir.resolve(), args.iterations)
