"""Resolve the real compiler import closure, including native support packages.

These helpers describe consumed inputs; they do not qualify their behavior.
Callers retain and hash their stdlib, runtime, tools and C header closure as well.
"""
from __future__ import annotations

import hashlib
from pathlib import Path
import re


ALIASES = {
    "selfhost_factory_path": "selfhost/factory_support/factory_path.sg",
    "selfhost_compiler_source_snapshot": "selfhost/compiler_support/source_snapshot.sg",
    "selfhost_compiler_snapshot_sha": "selfhost/runtime/sha256.sg",
    "selfhost_compiler_snapshot_utf8": "selfhost/runtime/utf8.sg",
}
IMPORT = re.compile(r"(?m)^\s*import\s+(selfhost_[A-Za-z0-9_]+)\s*;")


def resolve_module(root: Path, name: str) -> Path:
    relative = ALIASES.get(name)
    if relative is None:
        if not re.fullmatch(r"selfhost_[A-Za-z0-9_]+", name):
            raise ValueError("unsupported compiler module alias: " + name)
        relative = "selfhost/src/" + name[len("selfhost_"):] + ".sg"
    path = root / relative
    if not path.is_file():
        raise ValueError("missing compiler module: " + name + " at " + str(path))
    return path


def compiler_closure(root: Path, entries) -> dict[Path, str]:
    root = Path(root).resolve()
    pending = [Path(path).resolve() for path in entries]
    found = {}
    packages = {root / "selfhost/compiler_support", root / "selfhost/factory_support"}
    while pending:
        path = pending.pop()
        if path in found:
            continue
        if not path.is_relative_to(root):
            raise ValueError("compiler input outside root: " + str(path))
        raw = path.read_bytes()
        found[path] = hashlib.sha256(raw).hexdigest()
        if path.suffix != ".sg":
            continue
        pending.extend(resolve_module(root, name) for name in IMPORT.findall(raw.decode("utf8")))
        if path.parent in packages:
            package = path.parent
            # The seed discovers this native directory from the package manifest.
            # Retain its actual sources rather than recording only the SG alias.
            pending.append(package / "Sengoo.toml")
            native = package / "native"
            if not native.is_dir():
                raise ValueError("missing compiler support native directory")
            pending.extend(p for p in native.rglob("*") if p.is_file()
                           and p.suffix in {".c", ".h"})
    helper = root / "selfhost/compiler_inputs.py"
    found[helper] = hashlib.sha256(helper.read_bytes()).hexdigest()
    return dict(sorted(found.items()))


def module_map(root: Path, paths, destination: Path | None = None) -> str:
    root = Path(root).resolve()
    destination = root if destination is None else Path(destination).resolve()
    reverse = {Path(value): key for key, value in ALIASES.items()}
    modules = {}
    for value in paths:
        path = Path(value).resolve()
        relative = path.relative_to(root)
        name = reverse.get(relative)
        if name is None and relative.parent == Path("selfhost/src") and path.suffix == ".sg":
            name = "selfhost_" + path.stem
        if name is None:
            continue
        mapped = destination / relative
        if not mapped.is_file():
            raise ValueError("missing mapped compiler input: " + str(mapped))
        if name in modules and modules[name] != mapped:
            raise ValueError("ambiguous compiler alias: " + name)
        modules[name] = mapped
    return ";".join(name + "=" + str(path) for name, path in sorted(modules.items()))
