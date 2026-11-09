#!/usr/bin/env python3
"""Build and verify offline official-package artifacts, without a network registry.

Requires Python 3.11+. Uses sgpm's existing deterministic package format. It does
not publish anything or download dependencies.
"""
from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import re
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import tarfile
import tempfile
import tomllib

PACKAGES = ("sgcli", "sgdata", "sgsqlite")
AGENT_PACKAGES = ("sgschema", "sgllm", "sgmcp", "sgsqlite", "sgagent")


def profile_packages(profile: str) -> tuple[str, ...]:
    if profile == "foundation":
        return PACKAGES
    if profile == "agents":
        return AGENT_PACKAGES
    raise ValueError("unknown ecosystem profile")


def bundle_name(profile: str) -> str:
    return ("sengoo-official-registry-0.1.0.tar.gz" if profile == "foundation"
            else "sengoo-agent-preview-registry-0.1.0.tar.gz")


def normalize_agent_dependencies(root: Path) -> bool:
    """Translate known sibling dev paths in the *archive*, never the checkout.

    Consumers resolve exact versions from their configured official registry.
    Reject unknown dependencies instead of shipping checkout-relative paths.
    """
    path = root / "Sengoo.toml"
    text = path.read_text(encoding="utf-8")
    manifest = tomllib.loads(text)
    dependencies = manifest.get("dependencies", {})
    if not dependencies:
        return False
    normalized = []
    for alias, dependency in dependencies.items():
        if not isinstance(dependency, dict):
            raise ValueError("agent preview dependencies require explicit source tables")
        name = dependency.get("package", alias)
        if name not in AGENT_PACKAGES or set(dependency) - {"path", "package"}:
            raise ValueError(f"unsupported agent preview dependency: {alias}")
        if dependency.get("path") != f"../{name}" or not re.fullmatch(r"[a-z][a-z0-9_]*", alias):
            raise ValueError(f"unexpected development dependency path: {alias}")
        package = f', package = "{name}"' if name != alias else ""
        normalized.append(f'{alias} = {{ version = "=0.1.0", registry = "official"{package} }}\n')
    section = re.compile(r"(?ms)^\[dependencies\]\s*\n.*?(?=^\[|\Z)")
    text, count = section.subn("[dependencies]\n" + "".join(normalized) + "\n", text)
    if count != 1:
        raise ValueError("expected one explicit dependency section")
    path.write_text(text, encoding="utf-8", newline="\n")
    return True


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def unpack(archive: Path, destination: Path) -> None:
    """Extract bounded regular files into a fresh directory, rejecting links."""
    destination.mkdir(parents=True, exist_ok=False)
    seen: set[str] = set()
    with tarfile.open(archive, "r:gz") as tar:
        total = 0
        for index, member in enumerate(tar):
            parts = PurePosixPath(member.name).parts
            if (index >= 10000 or not parts or member.name in seen
                    or "\\" in member.name or PurePosixPath(member.name).is_absolute()
                    or any(part in ("..", ".") or ":" in part for part in parts)
                    or not (member.isfile() or member.isdir())):
                raise ValueError(f"unsafe archive entry: {member.name!r}")
            seen.add(member.name)
            total += member.size
            if member.size < 0 or total > 256 * 1024 * 1024:
                raise ValueError("archive exceeds the package size limit")
            target = destination.joinpath(*parts)
            if member.isdir():
                target.mkdir(parents=True, exist_ok=True)
            else:
                target.parent.mkdir(parents=True, exist_ok=True)
                source = tar.extractfile(member)
                if source is None:
                    raise ValueError(f"missing archive file: {member.name}")
                with source, target.open("xb") as output:
                    shutil.copyfileobj(source, output)


def deterministic_archive(root: Path, output: Path) -> None:
    with output.open("wb") as raw:
        with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as gz:
            with tarfile.open(fileobj=gz, mode="w", format=tarfile.PAX_FORMAT) as tar:
                for source in sorted(root.rglob("*")):
                    if not source.is_file() or source.is_symlink():
                        continue
                    info = tarfile.TarInfo(source.relative_to(root).as_posix())
                    info.size = source.stat().st_size
                    info.mode = 0o644
                    info.mtime = info.uid = info.gid = 0
                    with source.open("rb") as stream:
                        tar.addfile(info, stream)


def build(repo: Path, output: Path, sgpm: Path, profile: str = "foundation") -> dict:
    names = profile_packages(profile)
    repo, output, sgpm = repo.resolve(), output.resolve(), sgpm.resolve()
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="sengoo-ecosystem-") as temporary:
        stage = Path(temporary)
        archives = stage / "archives"
        archives.mkdir()
        bundle = stage / "bundle"
        (bundle / "registry").mkdir(parents=True)
        records = []
        for name in names:
            manifest = repo / "packages" / name / "Sengoo.toml"
            package = tomllib.loads(manifest.read_text(encoding="utf-8"))["package"]
            if package["name"] != name or package["version"] != "0.1.0":
                raise ValueError(f"unexpected official package identity: {manifest}")
            result = subprocess.run([str(sgpm), "publish", "--dry-run", "--format", "json",
                "--manifest-path", str(manifest), "--output", str(archives)],
                check=True, capture_output=True, text=True, encoding="utf-8")
            metadata = json.loads(result.stdout)
            archive = Path(metadata["archive_path"])
            if sha256(archive) != metadata["sha256"]:
                raise ValueError(f"sgpm artifact checksum mismatch: {name}")
            published = bundle / "registry" / name / package["version"]
            unpack(archive, published)
            source_archive_sha = metadata["sha256"]
            normalized = profile == "agents" and normalize_agent_dependencies(published)
            if normalized:
                deterministic_archive(published, archive)
            records.append({"name": name, "version": package["version"],
                "archive": archive.name, "sha256": sha256(archive),
                "source_archive_sha256": source_archive_sha,
                "normalized_development_dependencies": bool(normalized)})
        revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip()
        dirty = bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=repo, text=True).strip())
        toolchain = subprocess.check_output([str(sgpm), "--version"], text=True).strip()
        inventory = {"schema_version": 1, "revision": revision, "dirty": dirty,
            "toolchain": toolchain, "profile": profile, "packages": records}
        (bundle / "ecosystem-manifest.json").write_text(json.dumps(inventory, indent=2) + "\n", encoding="utf-8")
        (bundle / "README.txt").write_text(
            'Configure [registries.official] path = "registry" beside your Sengoo.toml.\n'
            'Select dependencies with version = "=0.1.0", registry = "official".\n'
            'Run sgpm update once; commit Sengoo.lock; use --locked afterwards.\n'
            'Check SHA256SUMS before extraction; see ecosystem-manifest.json for the build identity.\n', encoding="utf-8")
        bundle_archive = stage / bundle_name(profile)
        deterministic_archive(bundle, bundle_archive)
        products = [*sorted(archives.glob("*.tar.gz")), bundle_archive]
        for product in products:
            shutil.copyfile(product, output / product.name)
        (output / "ecosystem-manifest.json").write_text(json.dumps(inventory, indent=2) + "\n", encoding="utf-8")
        products.append(output / "ecosystem-manifest.json")
        (output / "SHA256SUMS").write_text("".join(
            f"{sha256(product)}  {product.name}\n" for product in sorted(products, key=lambda p: p.name)), encoding="utf-8")
    verify(output, profile)
    return inventory


def verify(directory: Path, profile: str = "foundation") -> None:
    entries = {}
    for line in (directory / "SHA256SUMS").read_text(encoding="utf-8").splitlines():
        digest, name = line.split("  ", 1)
        if len(digest) != 64 or any(c not in "0123456789abcdef" for c in digest):
            raise ValueError("invalid SHA-256 digest")
        if Path(name).name != name or name in entries or "\\" in name or ":" in name:
            raise ValueError("invalid checksum filename")
        entries[name] = digest
        if sha256(directory / name) != digest:
            raise ValueError(f"checksum mismatch: {name}")
    required = {f"{name}-0.1.0.tar.gz" for name in profile_packages(profile)} | {
        "ecosystem-manifest.json", bundle_name(profile)}
    if entries.keys() != required:
        raise ValueError("checksum manifest must contain the exact official artifact set")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    build_parser = sub.add_parser("build")
    build_parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
    build_parser.add_argument("--output", type=Path, default=Path("target/ecosystem-dist"))
    build_parser.add_argument("--sgpm", type=Path, required=True)
    build_parser.add_argument("--profile", choices=("foundation", "agents"), default="foundation")
    verify_parser = sub.add_parser("verify")
    verify_parser.add_argument("directory", type=Path)
    verify_parser.add_argument("--profile", choices=("foundation", "agents"), default="foundation")
    args = parser.parse_args()
    if args.command == "build":
        print(json.dumps(build(args.repo, args.output, args.sgpm, args.profile), indent=2))
    else:
        verify(args.directory, args.profile)
        print("official artifact checksums verified")


if __name__ == "__main__":
    main()
