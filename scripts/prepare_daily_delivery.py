#!/usr/bin/env python3
"""Assemble reviewable local delivery artifacts; never publish a release."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import shutil
import subprocess
import tarfile
import zipfile

from candidate_evidence import validate as validate_candidate
from package_ecosystem import sha256, verify


def toolchain_manifest(archive: Path) -> dict:
    if archive.suffix == ".zip":
        with zipfile.ZipFile(archive) as package:
            manifests = [name for name in package.namelist() if name.endswith("/manifest.json")]
            if len(manifests) != 1:
                raise ValueError("one toolchain build manifest is required")
            return json.loads(package.read(manifests[0]))
    with tarfile.open(archive, "r:gz") as package:
        manifests = [member for member in package.getmembers() if member.name.endswith("/manifest.json")]
        if len(manifests) != 1:
            raise ValueError("one toolchain build manifest is required")
        return json.load(package.extractfile(manifests[0]))


def prepare(toolchain: Path, official: Path, backend: Path, wheels: Path, output: Path,
            python_evidence: Path, candidate_evidence: Path | None = None) -> None:
    repo = Path(__file__).resolve().parents[1]
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip()
    dirty = bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=repo, text=True).strip())
    if dirty:
        raise ValueError("commit the reviewed candidate before assembling its delivery")
    verify(official)
    package_identity = json.loads((official / "ecosystem-manifest.json").read_text(encoding="utf-8"))
    compiler_identity = toolchain_manifest(toolchain)
    if (package_identity.get("revision") != revision or package_identity.get("dirty") is not False
            or compiler_identity.get("source_revision") != revision or compiler_identity.get("source_dirty") is not False):
        raise ValueError("toolchain, package bundle and clean checkout must identify the same committed candidate")
    expected = Path(str(toolchain) + ".sha256").read_text(encoding="utf-8-sig").split()[0].lower()
    if sha256(toolchain) != expected:
        raise ValueError("toolchain archive checksum mismatch")
    backend_wheels = list(backend.glob("sengoo_python-0.1.0-*.whl"))
    example_wheels = sorted(wheels.glob("sengoo_batch_*-0.1.0-*.whl"))
    if len(backend_wheels) != 1 or len(example_wheels) != 2:
        raise ValueError("one backend wheel and exactly two independently built consumer wheels are required")
    consumer = json.loads((python_evidence / "consumer.json").read_text(encoding="utf-8"))
    if (consumer.get("candidate_sha") != revision or consumer.get("offline_install") is not True
            or consumer.get("toolchain_on_path") is not False or consumer.get("source_project_available") is not False):
        raise ValueError("clean offline Python evidence must identify this candidate")
    observed_wheels = {record["filename"]: record["sha256"] for record in consumer.get("wheels", [])}
    if observed_wheels != {path.name: sha256(path) for path in example_wheels}:
        raise ValueError("delivered wheels differ from the wheels actually consumed")
    resources = json.loads((python_evidence / "resources.json").read_text(encoding="utf-8"))
    checkpoints = resources.get("checkpoints", [])
    baseline = resources.get("native_baseline", {})
    if (set(baseline) != {"sengoo_batch_a", "sengoo_batch_b"} or not checkpoints
            or checkpoints[-1].get("iteration") != resources.get("iterations_per_module")
            or any(sample.get("native_handles") != baseline for sample in checkpoints)):
        raise ValueError("both consumed modules must restore native resource counts at every checkpoint")
    costs = json.loads((python_evidence / "call-cost.json").read_text(encoding="utf-8"))
    sizes = {record.get("elements") for record in costs.get("records", [])
             if record.get("operation") == "sengoo_affine_with_input_output_copies"}
    if not {0, 1, 32, 1024, 65536}.issubset(sizes):
        raise ValueError("real batch cost observations are missing required scales")
    output.mkdir(parents=True, exist_ok=False)
    shutil.copyfile(toolchain, output / toolchain.name)
    shutil.copyfile(Path(str(toolchain) + ".sha256"), output / (toolchain.name + ".sha256"))
    for source in [*official.iterdir(), *backend_wheels, *example_wheels]:
        if source.is_file() and source.name != "SHA256SUMS":
            if (output / source.name).exists():
                raise ValueError(f"delivery artifact collision: {source.name}")
            shutil.copyfile(source, output / source.name)
    docs = output / "docs"
    docs.mkdir()
    shutil.copyfile(repo / "安装指南.md", output / "安装指南.md")
    for source in [repo / "docs" / name for name in (
            "language-reference.md", "doctor.md", "editor-setup.md", "official-package-catalog.md",
            "development-evidence.md", "daily-development-pilot.md")]:
        shutil.copyfile(source, docs / source.name)
    shutil.copyfile(repo / "docs/candidate-validation.md", docs / "candidate-validation.md")
    python_docs = output / "tools/sengoo-python"
    shutil.copytree(repo / "tools/sengoo-python", python_docs,
        ignore=shutil.ignore_patterns("target", "build", "dist", "__pycache__", "*.pyc", "*.egg-info", ".venv"))
    examples = output / "examples/realworld"
    examples.mkdir(parents=True)
    shutil.copyfile(repo / "examples/realworld/SUPPORT_MATRIX.md", examples / "SUPPORT_MATRIX.md")
    for name in ("file-csv-index", "python-batch"):
        shutil.copytree(repo / "examples/realworld" / name, examples / name,
            ignore=shutil.ignore_patterns("target", "build", "registry", "__pycache__", "*.pyc", ".venv"))
    scripts = output / "scripts"
    scripts.mkdir()
    for name in ("install.ps1", "install.sh", "package_ecosystem.py", "candidate_evidence.py"):
        shutil.copyfile(repo / "scripts" / name, scripts / name)
    evidence = output / "evidence"
    evidence.mkdir()
    for name in ("daily-installed-evidence.json", "index-incremental-evidence.json",
            "package-native-tests/evidence.json", "ownership-sanitizer-gate/evidence.json"):
        source = repo / "target" / name
        if not source.is_file():
            raise ValueError(f"local acceptance evidence is missing: {source}")
        result = json.loads(source.read_text(encoding="utf-8"))
        if result.get("revision") != revision:
            raise ValueError(f"acceptance evidence belongs to another candidate: {source}")
        passed = result.get("ok") is True
        if name == "package-native-tests/evidence.json":
            tests = result.get("tests", [])
            passed = ({test.get("package") for test in tests} == {"sgcli", "sgdata", "sgsqlite"}
                and len(tests) == 3 and all(test.get("exit_code") == 0 for test in tests))
        if not passed:
            raise ValueError(f"passing local acceptance evidence is required: {source}")
        shutil.copyfile(source, evidence / name.replace("/", "-"))
    python_reports = evidence / "python-wheel"
    python_reports.mkdir()
    for name in ("consumer.json", "resources.json", "call-cost.json"):
        shutil.copyfile(python_evidence / name, python_reports / name)
    remote_passed = False
    if candidate_evidence is not None:
        remote = json.loads(candidate_evidence.read_text(encoding="utf-8"))
        if remote.get("source_revision") != revision:
            raise ValueError("remote evidence belongs to another candidate")
        remote_passed = not remote.get("collection_errors") and not validate_candidate(remote)
        shutil.copyfile(candidate_evidence, evidence / "candidate-evidence.json")
    for name in ("sgc-final-tests.log", "installed-python-wheel-evidence.log"):
        source = repo / "target" / name
        if not source.read_text(encoding="utf-8-sig").startswith(f"Candidate {revision}\n"):
            raise ValueError(f"test transcript must identify this committed candidate: {source}")
        shutil.copyfile(source, evidence / name)
    (output / "verify-wheels.py").write_text(
        '"""Verify the delivered prebuilt modules after offline installation."""\n'
        'from pathlib import Path\nimport runpy\nimport sys\n'
        'import sengoo_batch_a as first\nimport sengoo_batch_b as second\n'
        'sys.modules["sengoo_batch"] = first\n'
        'checks = runpy.run_path(str(Path(__file__).parent / "examples/realworld/python-batch/verify.py"))\n'
        'checks["verify"]("sengoo_batch_a")\nchecks["verify"]("sengoo_batch_b")\n'
        'assert first.module_identity() == 1 and second.module_identity() == 2\n'
        'assert second.affine([2.0], 3.0, 1.0) == [7.0]\n'
        'assert second.owned_text("second module") == "second module"\n'
        'print("Both delivered prebuilt modules passed")\n', encoding="utf-8")
    (output / "README.md").write_text(
        "# Sengoo local development delivery\n\n"
        "These are Windows x64 development artifacts, not a qualified release. "
        "See DELIVERY.json, docs/development-evidence.md and evidence/.\n\n"
        "Verify SHA256SUMS before using the artifacts. SOURCE-SHA256SUMS identifies "
        "the committed source tree; it is not a release signature.\n\n"
        "Install the toolchain with scripts/install.ps1 -Archive <zip> -InstallDir <directory>. "
        "The matching .sha256 sidecar is included. See 安装指南.md for native prerequisites.\n\n"
        "Extract sengoo-official-registry-0.1.0.tar.gz and copy registry/ next to "
        "examples/realworld/file-csv-index/Sengoo.toml. Follow that project's README.\n\n"
        "Python consumers can install the two sengoo_batch wheel files using "
        "python -m pip install --no-index --no-deps <wheel paths>. "
        "Run python verify-wheels.py to check the prebuilt sengoo_batch_a/b modules. "
        "Only source builders need sengoo_python and the native toolchain. "
        "See tools/sengoo-python/README.md.\n", encoding="utf-8")
    source_names = subprocess.check_output(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"], cwd=repo
    ).decode("utf-8").split("\0")
    source_inventory = "".join(f"{sha256(repo / name)}  {name}\n" for name in
        sorted(set(source_names)) if name and (repo / name).is_file())
    (output / "SOURCE-SHA256SUMS").write_text(source_inventory, encoding="utf-8")
    manifest = {"schema_version": 1, "status": "local development artifacts; not a qualified release",
        "source_revision": revision, "uncommitted_changes": dirty,
        "remote_candidate_checks": "passed" if remote_passed else "incomplete or not collected",
        "source_inventory_sha256": sha256(output / "SOURCE-SHA256SUMS"),
        "toolchain_archive": toolchain.name, "backend": backend_wheels[0].name,
        "example_wheels": [path.name for path in example_wheels],
        "remaining_external_gates": ([] if remote_passed else ["four-host committed-candidate checks"]) + [
            "same-candidate six main-push release gates and published upgrade/rollback",
            "3-5 independent users over two weeks (deferred by owner)"]}
    (output / "DELIVERY.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    files = sorted(path for path in output.rglob("*") if path.is_file())
    (output / "SHA256SUMS").write_text("".join(
        f"{sha256(path)}  {path.relative_to(output).as_posix()}\n" for path in files), encoding="utf-8")
    print(f"Local review artifacts prepared: {output.resolve()}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--toolchain-archive", type=Path, required=True)
    parser.add_argument("--official", type=Path, required=True)
    parser.add_argument("--backend-wheels", type=Path, required=True)
    parser.add_argument("--example-wheels", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--python-evidence", type=Path, required=True)
    parser.add_argument("--candidate-evidence", type=Path)
    args = parser.parse_args()
    prepare(args.toolchain_archive, args.official, args.backend_wheels, args.example_wheels,
            args.output, args.python_evidence, args.candidate_evidence)


if __name__ == "__main__":
    main()
