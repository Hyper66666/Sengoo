"""Verify the consumed linker branch of source-bound historical qualifications."""
from __future__ import annotations

import hashlib
import json
from pathlib import Path


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def load_linker(root, evidence):
    """Resolve a passed linker, preserving every report/source binding in its chain."""
    root, evidence = root.resolve(), evidence.resolve()
    if not evidence.is_relative_to(root / "target"):
        raise ValueError("linker evidence must stay inside target/")
    consumed, chain, active = {}, [], set()

    def read(path, expected=None):
        path = path.resolve()
        if not path.is_relative_to(root / "target") and not path.is_relative_to(root):
            raise ValueError("linker source/report is outside the workspace")
        raw = path.read_bytes()
        observed = digest(raw)
        if expected is not None and observed != expected:
            raise ValueError("linker qualification input differs: " + str(path))
        consumed[str(path.relative_to(root))] = observed
        return raw

    def valid(data):
        if data.get("schema") != 1 or data.get("result") != "passed" or data.get("changed_sources") != [] or data.get("changed_tools") != []:
            raise ValueError("linker qualification did not pass unchanged-input checks")
        if not data.get("sources") or not data.get("steps") or not data.get("tools"):
            raise ValueError("linker qualification lacks its complete source/tool/command manifests")
        for name, value in data.items():
            if name.startswith("changed_") and value != []:
                raise ValueError("linker qualification has a failed input guard: " + name)

    def node(path, expected=None, replacement=None):
        path = path.resolve()
        if not path.is_relative_to(root / "target"):
            raise ValueError("linker report is outside target/")
        if path in active or len(active) >= 16:
            raise ValueError("cyclic or excessive linker qualification chain")
        active.add(path)
        raw = read(path, expected)
        data = json.loads(raw.decode("utf-8"))
        valid(data)
        row = {"path": str(path), "sha256": digest(raw), "stage": data["stage"], "source_revision": data.get("source_revision", data.get("revision")), "steps": len(data["steps"])}
        chain.append(row)
        if data["stage"] == "sg-coff-pe-linker-foundation":
            if replacement is not None and set(replacement) != set(data["sources"]):
                raise ValueError("copied linker source manifest is incomplete or has extra entries")
            sources = {}
            for name, expected_source in data["sources"].items():
                source = replacement[name] if replacement is not None else root / name
                source_raw = read(source, expected_source)
                sources[name] = {"path": str(source.resolve()), "sha256": digest(source_raw)}
            tools = {}
            for name, identity in data["tools"].items():
                tool = Path(identity["path"]).resolve()
                if digest(tool.read_bytes()) != identity["sha256"]:
                    raise ValueError("qualified linker tool differs: " + name)
                tools[name] = identity
            expected_binary = data.get("artifacts", {}).get("sg-linker.exe")
            if not expected_binary:
                raise ValueError("linker qualification has no executable artifact")
            binary = path.parent / "sg-linker.exe"
            binary_raw = binary.read_bytes()
            if digest(binary_raw) != expected_binary:
                raise ValueError("qualified linker executable differs")
            result = {"binary": binary, "binary_sha256": expected_binary, "binary_raw": binary_raw, "sources": sources,
                      "tools": tools, "foundation": row, "sdk": tools.get("sdk-kernel32")}
        elif data["stage"] == "sg-source-sg-link-execute":
            qualified = data.get("qualifications", {}).get("linker")
            if not qualified or not qualified.get("sources"):
                raise ValueError("source/link report has no complete copied linker branch")
            child_path = Path(qualified["path"]).resolve()
            child_key = str(child_path.relative_to(root))
            if data["sources"].get(child_key) != qualified["sha256"]:
                raise ValueError("linker child report is not bound by its parent manifest")
            copied = {}
            for name, identity in qualified["sources"].items():
                saved = root / identity["snapshot"]
                if data["sources"].get(str(saved.relative_to(root))) != identity["sha256"]:
                    raise ValueError("copied linker source is not bound by its parent manifest")
                read(saved, identity["sha256"])
                copied[name] = saved
            result = node(child_path, qualified["sha256"], copied)
            if data.get("tools", {}).get("linker", {}).get("sha256") != result["binary_sha256"]:
                raise ValueError("parent and original qualified linker artifacts differ")
            if qualified.get("steps") != result["foundation"]["steps"]:
                raise ValueError("copied qualification command history differs")
            row["linker_branch_only"] = True
            row["copied_source_count"] = len(copied)
        elif data["stage"] == "sg-native-module-graph":
            qualified = data.get("linker_qualification")
            if not qualified:
                raise ValueError("module report did not qualify an SG linker")
            previous = Path(qualified["evidence"]["path"]).resolve()
            if data["sources"].get(str(previous.relative_to(root))) != qualified["evidence"]["sha256"]:
                raise ValueError("prior linker report is not bound by the module manifest")
            result = node(previous, qualified["evidence"]["sha256"])
            if set(qualified["sources"]) != set(result["sources"]):
                raise ValueError("module linker source manifest is incomplete")
            for name, identity in qualified["sources"].items():
                if identity["sha256"] != result["sources"][name]["sha256"] or data["sources"].get(identity["snapshot"]) != identity["sha256"]:
                    raise ValueError("module linker snapshot is not bound to its original qualification")
                read(root / identity["snapshot"], identity["sha256"])
            binary = Path(qualified["binary"]["path"]).resolve()
            binary_raw = binary.read_bytes()
            if digest(binary_raw) != result["binary_sha256"] or qualified["binary"]["sha256"] != result["binary_sha256"]:
                raise ValueError("module copied linker artifact differs")
            declared = data.get("tools", {}).get("sg-linker", {})
            if declared.get("sha256") != result["binary_sha256"] or Path(declared.get("path", "")).resolve() != binary:
                raise ValueError("module linker artifact is not bound by its tool manifest")
            result.update(binary=binary, binary_raw=binary_raw)
            row["linker_branch_only"] = True
        else:
            raise ValueError("unsupported linker qualification stage: " + str(data["stage"]))
        active.remove(path)
        return result

    resolved = node(evidence)
    resolved.update(chain=chain, consumed=consumed, evidence={"path": str(evidence), "sha256": consumed[str(evidence.relative_to(root))]})
    return resolved


def copy_linker(root, evidence, output, report):
    resolved = load_linker(root, evidence)
    manifest = {}
    for name, source in resolved["sources"].items():
        raw = Path(source["path"]).read_bytes()
        if digest(raw) != source["sha256"]:
            raise ValueError("linker snapshot changed before copying")
        saved = output / "linker-implementation" / name
        if not saved.resolve().is_relative_to((output / "linker-implementation").resolve()):
            raise ValueError("linker source key escapes its copied implementation directory")
        saved.parent.mkdir(parents=True, exist_ok=True)
        saved.write_bytes(raw)
        if digest(saved.read_bytes()) != source["sha256"]:
            raise ValueError("copied linker snapshot differs")
        relative = str(saved.relative_to(root))
        report["sources"][relative] = source["sha256"]
        manifest[name] = {"sha256": source["sha256"], "snapshot": relative, "qualified_snapshot": source["path"]}
    binary = output / "sg-linker.exe"
    binary.write_bytes(resolved["binary_raw"])
    if digest(binary.read_bytes()) != resolved["binary_sha256"]:
        raise ValueError("copied qualified linker binary differs")
    report["sources"].update(resolved["consumed"])
    report["tools"]["sg-linker"] = {"path": str(binary), "sha256": resolved["binary_sha256"]}
    for name, tool in resolved["tools"].items():
        report["tools"]["linker-producer-" + name] = tool
    report["linker_qualification"] = {"evidence": resolved["evidence"], "chain": resolved["chain"], "sources": manifest,
                                       "foundation": resolved["foundation"], "binary": report["tools"]["sg-linker"],
                                       "scope": "pinned historical linker executable; consumes the complete original source-bound linker branch, without rebuilding current linker sources"}
    return binary, resolved
