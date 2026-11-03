"""PEP 517 backend for [tool.sengoo] native platform wheels."""
import base64
import csv
import hashlib
import io
import json
import keyword
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import sysconfig
import tarfile
import tempfile
import tomllib
import zipfile

from . import __version__


def _configuration():
    root = Path.cwd().resolve()
    document = tomllib.loads((root / "pyproject.toml").read_text(encoding="utf-8"))
    project = document.get("project", {})
    name, version = project.get("name"), project.get("version")
    if not isinstance(name, str) or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*", name):
        raise ValueError("[project].name must be a distribution name")
    if not isinstance(version, str) or not re.fullmatch(r"[0-9]+(?:\.[0-9]+)*(?:(?:a|b|rc)[0-9]+)?", version):
        raise ValueError("[project].version must be a static Python version")
    config = document.get("tool", {}).get("sengoo", {})
    module, exports = config.get("module"), config.get("exports")
    if not isinstance(module, str) or any(not part.isidentifier() or not part.isascii() or keyword.iskeyword(part) for part in module.split(".")):
        raise ValueError("[tool.sengoo].module must be a Python module name")
    if not isinstance(exports, list) or not exports or any(not isinstance(item, str) or not item.isidentifier() or not item.isascii() or keyword.iskeyword(item) or item == "SengooError" or item.startswith("__") for item in exports):
        raise ValueError("[tool.sengoo].exports must contain existing function names")
    if len(set(exports)) != len(exports):
        raise ValueError("duplicate [tool.sengoo].exports")
    manifest = (root / config.get("manifest", "Sengoo.toml")).resolve()
    if not manifest.is_relative_to(root) or not manifest.is_file():
        raise ValueError("manifest must name an existing file inside the Python project")
    return root, name, version, module, exports, manifest


def _dist_info(name, version):
    return re.sub(r"[-_.]+", "_", name) + "-" + version + ".dist-info"


def _metadata(name, version):
    return f"Metadata-Version: 2.1\nName: {name}\nVersion: {version}\nRequires-Python: >=3.11,<3.15\n\n"


def _platform_tag(native=None):
    # Linux wheels remain linux_* until auditwheel repairs them in a real
    # manylinux build image. Never claim manylinux compatibility by retagging.
    if sys.platform == "darwin" and native is not None:
        result = subprocess.run(["otool", "-l", str(native)], check=True, capture_output=True, text=True)
        versions = re.findall(r"\bminos\s+([0-9.]+)", result.stdout)
        versions += re.findall(r"cmd LC_VERSION_MIN_MACOSX\s+cmdsize \d+\s+version ([0-9.]+)", result.stdout)
        if not versions:
            raise RuntimeError("cannot determine the shared library's actual macOS deployment target")
        deployment = max(tuple(map(int, value.split("."))) for value in versions)
        architecture = platform.machine()
        if architecture not in ("arm64", "x86_64"):
            raise RuntimeError(f"unsupported macOS wheel architecture: {architecture}")
        # Modern macOS wheel tags use minor zero for deployment >=11.
        major, minor = deployment[0], deployment[1] if len(deployment) > 1 else 0
        return f"macosx_{major}_{0 if major >= 11 else minor}_{architecture}"
    return sysconfig.get_platform().replace("-", "_").replace(".", "_")


def get_requires_for_build_wheel(config_settings=None):
    return []


def get_requires_for_build_sdist(config_settings=None):
    return []


def prepare_metadata_for_build_wheel(metadata_directory, config_settings=None):
    _, name, version, *_ = _configuration()
    directory = Path(metadata_directory) / _dist_info(name, version)
    directory.mkdir(parents=True, exist_ok=True)
    (directory / "METADATA").write_text(_metadata(name, version), encoding="utf-8")
    return directory.name


def _annotation(ty, output=False):
    return {"i64": "int", "f64": "float", "bool": "bool", "str": "str", "string": "str",
            "unit": "None", "vec_i64": "list[int]" if output else "Sequence[int]",
            "vec_f64": "list[float]" if output else "Sequence[float]"}[ty]


def _stub(descriptor):
    text = "from typing import Sequence\n\nclass SengooError(Exception):\n    code: int\n\n"
    for function in descriptor["functions"]:
        params = ", ".join(f"{param['name']}: {_annotation(param['ty'])}" for param in function["params"])
        text += f"def {function['name']}({params}) -> {_annotation(function['returns'], True)}: ...\n"
    return text


def _tool(name, environment):
    configured = os.environ.get(environment)
    path = configured or shutil.which(name)
    if not path:
        raise RuntimeError(f"{name} is required for source builds; set {environment} or install the matching Sengoo toolchain")
    return path


def _ensure_toolchain(sgpm):
    sgc = _tool("sgc", "SGPM_SGC")
    identities = []
    for name, executable in (("sgpm", sgpm), ("sgc", sgc)):
        result = subprocess.run([executable, "--version"], check=True, capture_output=True, text=True)
        identity = result.stdout.strip().removeprefix(name + " ")
        version = re.match(r"(\d+)\.(\d+)\.(\d+)", identity)
        if not version or tuple(map(int, version.groups())) < (0, 2, 1):
            raise RuntimeError(f"{name} >=0.2.1-dev.0 is required for typed bindings; found {result.stdout.strip()}")
        identities.append(identity)
    if identities[0] != identities[1]:
        raise RuntimeError(f"sgpm and sgc must come from the same toolchain build: {identities}")


def build_wheel(wheel_directory, config_settings=None, metadata_directory=None):
    if sys.implementation.name != "cpython" or not (3, 11) <= sys.version_info[:2] <= (3, 14):
        raise RuntimeError("sengoo-python 0.1 supports CPython 3.11 through 3.14")
    root, name, version, module, exports, manifest = _configuration()
    if not (manifest.parent / "Sengoo.lock").is_file():
        raise RuntimeError("source builds require Sengoo.lock; run sgpm update first")
    sgpm = _tool("sgpm", "SENGOO_SGPM")
    _ensure_toolchain(sgpm)
    extension = ".dll" if sys.platform == "win32" else ".dylib" if sys.platform == "darwin" else ".so"
    with tempfile.TemporaryDirectory(prefix="sengoo-wheel-") as temporary:
        native = Path(temporary) / ("_native" + extension)
        command = [sgpm, "build", "--shared", "--locked", "--release", "--manifest-path", str(manifest), "--output", str(native)]
        runtime_mode = os.environ.get("SENGOO_PYTHON_RUNTIME_MODE")
        if runtime_mode:
            command += ["--runtime-mode", runtime_mode]
        for export in exports:
            command += ["--python-export", export]
        subprocess.run(command, cwd=root, check=True)
        descriptor_bytes = Path(str(native) + ".sgpy.json").read_bytes()
        descriptor = json.loads(descriptor_bytes)
        if descriptor.get("schema_version") != 1 or descriptor.get("binding_abi_version") != 1:
            raise RuntimeError("compiler generated an unsupported binding ABI")
        if [function["name"] for function in descriptor["functions"]] != exports:
            raise RuntimeError("compiler binding exports do not match pyproject.toml")
        package = module.replace(".", "/")
        files = {
            f"{package}/{native.name}": native.read_bytes(),
            f"{package}/_bindings.json": descriptor_bytes,
            f"{package}/_runtime.py": Path(__file__).with_name("runtime.py").read_bytes(),
            f"{package}/__init__.py": (f"from ._runtime import install as _install\n_install(globals(), __file__, {native.name!r})\ndel _install\n").encode(),
            f"{package}/__init__.pyi": _stub(descriptor).encode(),
            f"{package}/py.typed": b"",
        }
        parents = module.split(".")[:-1]
        for index in range(len(parents)):
            files["/".join(parents[:index + 1]) + "/__init__.py"] = b""
        tag = "py3-none-" + _platform_tag(native)
        info = _dist_info(name, version)
        files[f"{info}/METADATA"] = _metadata(name, version).encode()
        files[f"{info}/WHEEL"] = f"Wheel-Version: 1.0\nGenerator: sengoo-python {__version__}\nRoot-Is-Purelib: false\nTag: {tag}\n".encode()
        records = io.StringIO(newline="")
        writer = csv.writer(records, lineterminator="\n")
        for path, content in sorted(files.items()):
            digest = base64.urlsafe_b64encode(hashlib.sha256(content).digest()).rstrip(b"=").decode()
            writer.writerow([path, "sha256=" + digest, str(len(content))])
        writer.writerow([f"{info}/RECORD", "", ""])
        files[f"{info}/RECORD"] = records.getvalue().encode()
        filename = f"{re.sub(r'[-_.]+', '_', name)}-{version}-{tag}.whl"
        wheel_path = Path(wheel_directory).resolve() / filename
        wheel_path.parent.mkdir(parents=True, exist_ok=True)
        with zipfile.ZipFile(wheel_path, "w", compression=zipfile.ZIP_DEFLATED) as wheel:
            for path, content in sorted(files.items()):
                entry = zipfile.ZipInfo(path, date_time=(2020, 1, 1, 0, 0, 0))
                entry.compress_type = zipfile.ZIP_DEFLATED
                entry.external_attr = 0o644 << 16
                wheel.writestr(entry, content)
        return filename


def _validate_sdist_inputs(root, manifest):
    """A source archive must not silently omit local locked build inputs."""
    if not (manifest.parent / "Sengoo.lock").is_file():
        raise RuntimeError("source distributions require Sengoo.lock; run sgpm update first")
    pending, visited = [manifest], set()
    while pending:
        current = pending.pop().resolve()
        if current in visited:
            continue
        visited.add(current)
        if not current.is_relative_to(root) or not current.is_file():
            raise ValueError("sdist local dependencies must be inside the Python project; copy them into the project or use a versioned registry dependency")
        document = tomllib.loads(current.read_text(encoding="utf-8"))
        for kind in ("lib", "bin"):
            if kind not in document:
                continue
            entry = (current.parent / document[kind].get("path", f"src/{'lib' if kind == 'lib' else 'main'}.sg")).resolve()
            if not entry.is_relative_to(root) or not entry.is_file():
                raise ValueError(f"sdist {kind} entry must be an existing source inside the Python project")
            relative = entry.relative_to(root)
            if any(part in (".git", ".venv", "__pycache__", "target") for part in relative.parts) or relative.parts[0] in ("build", "dist"):
                raise ValueError("sdist source entries cannot be located in generated output directories")
        for dependency in document.get("dependencies", {}).values():
            if isinstance(dependency, dict) and "path" in dependency:
                pending.append(current.parent / dependency["path"] / "Sengoo.toml")
        for registry in document.get("registries", {}).values():
            if "path" in registry:
                directory = (current.parent / registry["path"]).resolve()
                if not directory.is_relative_to(root) or not directory.is_dir():
                    raise ValueError("sdist local registry bundles must be inside the Python project")


def build_sdist(sdist_directory, config_settings=None):
    root, name, version, _, _, manifest = _configuration()
    _validate_sdist_inputs(root, manifest)
    prefix = re.sub(r"[-_.]+", "_", name) + "-" + version
    destination = Path(sdist_directory).resolve()
    destination.mkdir(parents=True, exist_ok=True)
    filename = prefix + ".tar.gz"
    excluded = {".git", ".venv", "__pycache__", "target"}
    with tarfile.open(destination / filename, "w:gz") as archive:
        metadata = _metadata(name, version).encode()
        entry = tarfile.TarInfo(prefix + "/PKG-INFO")
        entry.size = len(metadata)
        entry.mode = 0o644
        archive.addfile(entry, io.BytesIO(metadata))
        for path in sorted(root.rglob("*")):
            relative = path.relative_to(root)
            if any(part in excluded for part in relative.parts) or relative.parts[0] in ("build", "dist") or path.is_relative_to(destination):
                continue
            if path.is_symlink():
                raise ValueError(f"source distribution does not accept symlinks: {relative}")
            if path.is_file():
                archive.add(path, arcname=prefix + "/" + relative.as_posix(), recursive=False)
    return filename
