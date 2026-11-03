"""Repair ctypes wheel dependencies and record the exact delivered artifacts."""
import argparse
import base64
import csv
import hashlib
import importlib.metadata
import io
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import zipfile


def validate_archive(path, target):
    with zipfile.ZipFile(path) as archive:
        names = archive.namelist()
        if len(names) != len(set(names)):
            raise RuntimeError(f"duplicate ZIP entries in {path.name}")
        metadata = next(name for name in names if name.endswith(".dist-info/WHEEL"))
        tags = [line.removeprefix("Tag: ") for line in archive.read(metadata).decode().splitlines() if line.startswith("Tag: ")]
        if not tags or any(not tag.startswith("py3-none-") for tag in tags):
            raise RuntimeError(f"unexpected interpreter/ABI tags in {path.name}: {tags}")
        platforms = [tag.removeprefix("py3-none-") for tag in tags]
        allowed = {"windows-x64": lambda tag: tag == "win_amd64",
                   "linux-x64": lambda tag: tag == "manylinux_2_28_x86_64",
                   "macos-x64": lambda tag: tag.startswith("macosx_") and tag.endswith("_x86_64"),
                   "macos-arm64": lambda tag: tag.startswith("macosx_") and tag.endswith("_arm64")}
        if not all(allowed[target](tag) for tag in platforms):
            raise RuntimeError(f"unexpected delivered platform for {target}: {platforms}")
        records = next(name for name in names if name.endswith(".dist-info/RECORD"))
        seen = set()
        for name, digest, size in csv.reader(io.StringIO(archive.read(records).decode())):
            seen.add(name)
            if name == records:
                continue
            content = archive.read(name)
            expected = "sha256=" + base64.urlsafe_b64encode(hashlib.sha256(content).digest()).rstrip(b"=").decode()
            if digest != expected or int(size) != len(content):
                raise RuntimeError(f"invalid RECORD hash/size for {name}")
        if seen != {name for name in names if not name.endswith("/")}:
            raise RuntimeError(f"RECORD does not cover the complete wheel: {path.name}")
        return {"filename": path.name, "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                "tags": tags, "native_files": [name for name in names if name.endswith((".dll", ".so", ".dylib")) or ".so." in name]}


def repair(source, destination, target, report):
    wheels = sorted(source.resolve().glob("*.whl"))
    if len(wheels) != 2:
        raise RuntimeError("expected exactly two independent input wheels")
    destination = destination.resolve()
    if destination == source.resolve() or list(destination.glob("*.whl")):
        raise RuntimeError("repair output must be a separate directory without stale wheels")
    destination.mkdir(parents=True, exist_ok=True)
    logs = []

    def run(arguments):
        result = subprocess.run(arguments, check=True, text=True, capture_output=True)
        logs.append({"command": arguments, "stdout": result.stdout, "stderr": result.stderr})
        print(result.stdout, end="")

    if target == "windows-x64":
        tool = "delvewheel"
        # The active CPython installation ships the matching redistributable.
        # Prefer it over unrelated development DLLs on a broad build PATH.
        redist = str(Path(sys.base_prefix)) + os.pathsep + str(Path(os.environ["SystemRoot"]) / "System32")
        for wheel in wheels:
            run([sys.executable, "-m", tool, "show", "--analyze-existing", "--add-path", redist, str(wheel)])
            run([sys.executable, "-m", tool, "repair", "--analyze-existing", "--add-path", redist, "--wheel-dir", str(destination), str(wheel)])
    elif target == "linux-x64":
        tool = "auditwheel"
        for wheel in wheels:
            run([sys.executable, "-m", tool, "show", str(wheel)])
            run([sys.executable, "-m", tool, "repair", "--plat", "manylinux_2_28_x86_64", "--only-plat", "--wheel-dir", str(destination), str(wheel)])
    else:
        tool = "delocate"
        architecture = "arm64" if target == "macos-arm64" else "x86_64"
        if platform.machine() != architecture:
            raise RuntimeError(f"wheel host architecture {platform.machine()} does not match {target}")
        for wheel in wheels:
            run(["delocate-listdeps", "--all", str(wheel)])
            run(["delocate-wheel", "--require-archs", architecture, "--wheel-dir", str(destination), str(wheel)])
    delivered = sorted(destination.glob("*.whl"))
    if len(delivered) != 2:
        raise RuntimeError("repair did not produce exactly two delivered wheels")
    evidence = {"schema_version": 1, "candidate_sha": os.environ.get("CANDIDATE_SHA", os.environ.get("GITHUB_SHA")),
                "platform": target, "repair_tool": tool, "repair_tool_version": importlib.metadata.version(tool),
                "inputs": [{"filename": path.name, "sha256": hashlib.sha256(path.read_bytes()).hexdigest()} for path in wheels],
                "outputs": [validate_archive(path, target) for path in delivered], "dependency_commands": logs}
    report.parent.mkdir(parents=True, exist_ok=True)
    report.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
    return evidence


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input-dir", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--platform", choices=["windows-x64", "linux-x64", "macos-x64", "macos-arm64"], required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    repair(args.input_dir, args.output_dir, args.platform, args.report)
