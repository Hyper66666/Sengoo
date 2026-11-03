#!/usr/bin/env python3
"""Bounded, deterministic ownership programs and their independent Drop oracle.

Generate: python scripts/ownership-combinations.py --seed 20261104 --cases 4
Run the generated corpus with the existing ownership-sanitizer-gate.py. Artifacts
are never deleted by this generator or that gate, including on a failed case.
The seed, generator version, shape, execution paths and source digest are saved.
"""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path


VERSION = 1
DEFAULT_SEED = 20261104
DEFAULT_CASES = 4
MAX_CASES = 32
MAX_DEPTH = 3
MAX_NODES = 32
MASK = (1 << 64) - 1


class Random:
    """SplitMix64, defined here so Python's random implementation is irrelevant."""
    def __init__(self, seed: int):
        self.state = seed & MASK

    def take(self, bound: int) -> int:
        if bound <= 0:
            raise ValueError("random bound must be positive")
        self.state = (self.state + 0x9E3779B97F4A7C15) & MASK
        value = self.state
        value = ((value ^ (value >> 30)) * 0xBF58476D1CE4E5B9) & MASK
        value = ((value ^ (value >> 27)) * 0x94D049BB133111EB) & MASK
        return (value ^ (value >> 31)) % bound


def shape(rng: Random, depth: int, budget: list[int], forced: str | None = None) -> dict:
    if budget[0] < 1:
        raise ValueError("ownership shape exhausted its node budget")
    budget[0] -= 1
    kind = forced or ("owned" if depth == 0 or budget[0] < 3 else
        ("owned", "vec", "option", "result_ok", "result_error", "rc", "map")[rng.take(7)])
    if depth == 0 or budget[0] < 1:
        kind = "owned"
    children = []
    if kind != "owned":
        count = 1 + rng.take(2) if kind == "vec" and budget[0] > 3 else 1
        # Elements of a Vec share a type, but constructors own distinct values.
        child = shape(rng, depth - 1, budget)
        children = [child]
        duplicate_cost = statistics(child)["nodes"]
        if count == 2 and budget[0] >= duplicate_cost:
            budget[0] -= duplicate_cost
            children.append(json.loads(json.dumps(child)))
    return {"kind": kind, "children": children}


def statistics(node: dict) -> dict:
    children = [statistics(child) for child in node["children"]]
    return {"nodes": 1 + sum(child["nodes"] for child in children),
        "depth": 0 if not children else 1 + max(child["depth"] for child in children),
        "resources": int(node["kind"] == "owned") + sum(child["resources"] for child in children)}


def resource_ids(node: dict) -> list[int]:
    """Interpret ownership metadata independently of source construction."""
    if node["kind"] == "owned":
        return [node["id"]]
    return [identifier for child in node["children"] for identifier in resource_ids(child)]


class Program:
    def __init__(self, seed: int, index: int):
        self.seed = seed
        self.index = index
        self.rng = Random(seed ^ (index * 0xD1342543DE82EF95))
        self.next_id = 1000 + index * 100
        self.functions: list[str] = []
        self.types: list[str] = []
        self.next_function = 0

    def owned_id(self) -> int:
        result = self.next_id
        self.next_id += 1
        return result

    def lower(self, node: dict) -> tuple[str, str]:
        kind = node["kind"]
        serial = self.next_function
        self.next_function += 1
        name = f"construct_{serial}"
        statements = []
        children = []
        for i, child in enumerate(node["children"]):
            ty, build = self.lower(child)
            children.append((ty, f"child_{i}"))
            statements.append(f"let child_{i} = {build}()?;")
        if kind == "owned":
            identifier = self.owned_id()
            node["id"] = identifier
            ty = "Owned"
            expression = f"make_owned({identifier})?"
        elif kind == "vec":
            ty = f"Vec<{children[0][0]}>"
            statements.append(f"let value: {ty} = vec_new();")
            for _, child in children:
                # Allocation failure still makes the exact-resource assertion
                # fail; no placeholder owning aggregate is fabricated here.
                statements.append(f"value.push({child});")
            expression = "value"
        elif kind == "option":
            ty = f"Option<{children[0][0]}>"
            expression = f"option_some({children[0][1]})"
        elif kind == "result_ok":
            ty = f"Result<{children[0][0]}, i64>"
            expression = f"result_ok_with({children[0][1]}, 0)"
        elif kind == "result_error":
            ty = f"Result<i64, {children[0][0]}>"
            expression = f"result_err_with(0, {children[0][1]})"
        elif kind == "rc":
            ty = f"Rc<{children[0][0]}>"
            statements.append(f"let value: {ty} = rc_new({children[0][1]});")
            # The alias dies before the returned Rc, exercising reference counts.
            statements.append("{ let alias = value.clone(); if alias.strong_count() != 2 { return result_err_with(value, 91); }; };")
            expression = "value"
        elif kind == "map":
            ty = f"HashMap<i64, {children[0][0]}>"
            statements.append(f"let value: {ty} = hashmap_new();")
            statements.append(f"value.insert({1 + self.rng.take(100)}, {children[0][1]});")
            expression = "value"
        else:
            raise ValueError(f"unknown ownership node {kind}")
        # These are separate functions; consumer Drop cannot rely on helpers
        # materialized privately by a constructor's lowering context.
        self.functions.append(f"def {name}() -> Result<{ty}, i64> {{\n    " + "\n    ".join(statements) +
            f"\n    let result: {ty} = {expression};\n    result_ok_with(result, 0)\n}}")
        return ty, name

    def generate(self) -> dict:
        # Rotate mandatory top-level wrappers. The nested grammar is seeded;
        # four default cases cover every ownership container and descriptor.
        mandatory = ("vec", "map", "rc", "option")
        budget = [18]
        nodes = [shape(self.rng, MAX_DEPTH, budget, mandatory[(self.index + i) % 4]) for i in range(2)]
        # Pin an active error payload in every case (often missed by success-only
        # generators) and nest it inside a typed Vec descriptor.
        nodes.append({"kind": "vec", "children": [{"kind": "result_error", "children": [{"kind": "owned", "children": []}]}]})
        stats = [statistics(node) for node in nodes]
        if sum(item["nodes"] for item in stats) > MAX_NODES or max(item["depth"] for item in stats) > MAX_DEPTH:
            raise ValueError("ownership generator exceeded its declared bounds")
        built = [self.lower(node) for node in nodes]
        fields = ", ".join(f"field_{i}: {ty}" for i, (ty, _) in enumerate(built))
        constructor = "\n    ".join(f"let field_{i} = {build}()?;" for i, (_, build) in enumerate(built))
        literal = ", ".join(f"field_{i}: field_{i}" for i in range(len(built)))
        marker = self.owned_id()
        branch_true = self.owned_id()
        branch_false = self.owned_id()
        loop_base = self.owned_id()
        iterations = 2 + self.rng.take(3)
        self.next_id += iterations
        tree_resources = [identifier for node in nodes for identifier in resource_ids(node)]
        expected = Counter(tree_resources * 3)
        expected.update({marker: 3, branch_true: 2, branch_false: 1})
        expected.update(loop_base + i for i in range(iterations))
        resource_count = len(tree_resources) * 3 + 3 + iterations
        source = f"""// ownership generator v{VERSION}, seed={self.seed}, case={self.index}
import std::collections;
extern "C" {{
    fn ownership_probe_new(value: i64) -> i64;
    fn ownership_probe_drop(handle: i64);
    fn ownership_probe_read(handle: i64) -> i64;
    fn ownership_probe_live() -> i64;
    fn ownership_probe_created() -> i64;
    fn ownership_probe_dropped() -> i64;
    fn sengoo_string_live_handle_count() -> i64;
}}
struct Owned {{ id: i64, handle: i64, text: String }}
impl Drop for Owned {{
    def drop(&mut self) {{ print(self.id); ownership_probe_drop(self.handle); }}
}}
def make_owned(id: i64) -> Result<Owned, i64> {{
    let text = string_from_str("owned-世界-{self.seed & 65535}")?;
    result_ok_with(Owned {{ id: id, handle: ownership_probe_new(id), text: text }}, 0)
}}
{chr(10).join(self.functions)}
struct Envelope {{ marker: i64, {fields} }}
impl Drop for Envelope {{ def drop(&mut self) {{ print(self.marker); }} }}
def cross_function_return() -> Result<Envelope, i64> {{
    {constructor}
    result_ok_with(Envelope {{ marker: {marker}, {literal} }}, 0)
}}
def exercise(fail: bool, early: bool, flag: bool) -> Result<i64, i64> {{
    let envelope = cross_function_return()?;
    let selected = if flag {{ make_owned({branch_true})? }} else {{ make_owned({branch_false})? }};
    let gate = if fail {{ result_err_i64(9) }} else {{ result_ok_i64(1) }};
    gate?;
    if early {{ return result_ok_i64(ownership_probe_read(selected.handle)); }};
    let mut index = 0;
    while index < {iterations + 2} {{
        let current = make_owned({loop_base} + index)?;
        index = index + 1;
        if index == 1 {{ continue; }};
        if ownership_probe_read(current.handle) != {loop_base} + index - 1 {{ return result_err_i64(92); }};
        if index == {iterations} {{ break; }};
    }};
    result_ok_i64(index)
}}
def main() -> i64 {{
    let before = sengoo_string_live_handle_count();
    let failed = exercise(true, false, true);
    let early = exercise(false, true, false);
    let passed = exercise(false, false, true);
    let after = sengoo_string_live_handle_count();
    if failed.error != 9 || early.value != {branch_false} || passed.value != {iterations} {{ return 1; }};
    if before != after {{ return 2; }};
    if ownership_probe_live() != 0 || ownership_probe_created() != {resource_count} || ownership_probe_dropped() != {resource_count} {{ return 3; }};
    println("ownership-generated: passed");
    0
}}
"""
        return {"name": f"seed-{self.seed:016x}-case-{self.index:02d}", "expect": "success", "source": source,
            "seed": self.seed, "case_index": self.index, "shape": nodes, "expected_drop_counts": dict(sorted(expected.items())),
            "shape_statistics": {"nodes": sum(item["nodes"] for item in stats), "depth": max(item["depth"] for item in stats)},
            "expected_resources": resource_count, "paths": ["question-mark-error", "explicit-return", "loop-continue-break"],
            "path_oracle": {"aggregate_runs": 3, "aggregate_resource_ids": tree_resources, "aggregate_drop_id": marker,
                "branch_true_id": branch_true, "branch_true_runs": 2, "branch_false_id": branch_false,
                "branch_false_runs": 1, "loop_resource_ids": list(range(loop_base, loop_base + iterations))}}


def rejected_cases(seed: int) -> list[dict]:
    prefix = "struct Owned { id: i64 }\nimpl Drop for Owned { def drop(&mut self) {} }\n"
    variants = [
        ("tagged-partial-move", "unsupported-partial-move-with-drop", "import std::collections;\n" + prefix +
            "struct Pair { first: Owned, second: Owned }\ndef main() -> i64 { let wrapped = option_some(Pair { first: Owned { id: 1 }, second: Owned { id: 2 } }); let moved = wrapped.value.first; moved.id }\n"),
        ("owned-closure-capture", "unsupported-closure-capture", prefix +
            "def main() -> i64 { let owner = Owned { id: 1 }; let callback: fn(i64) -> i64 = |value| owner.id + value; callback(2) }\n"),
        ("nested-borrow-escape", "borrow-escapes-owner", prefix +
            "struct Envelope { owner: Owned }\ndef escape() -> &Owned { let value = Envelope { owner: Owned { id: 1 } }; &value.owner }\ndef main() -> i64 { 0 }\n"),
    ]
    return [{"name": f"seed-{seed:016x}-reject-{name}", "seed": seed, "expect": "diagnostic", "diagnostic": code,
        "source": f"// ownership generator v{VERSION}, seed={seed}, expected diagnostic={code}\n{source}"}
        for name, code, source in variants]


def corpus(seed: int = DEFAULT_SEED, cases: int = DEFAULT_CASES) -> list[dict]:
    if not 0 <= seed <= MASK:
        raise ValueError("seed must be an unsigned 64-bit integer")
    if not 1 <= cases <= MAX_CASES:
        raise ValueError(f"cases must be between 1 and {MAX_CASES}; no cases may be skipped")
    return [Program(seed, index).generate() for index in range(cases)] + rejected_cases(seed)


def write_corpus(directory: Path, seed: int = DEFAULT_SEED, cases: int = DEFAULT_CASES) -> dict:
    directory.mkdir(parents=True, exist_ok=True)
    records = []
    for case in corpus(seed, cases):
        case_directory = directory / case["name"]
        case_directory.mkdir(parents=True, exist_ok=True)
        path = case_directory / "source.sg"
        source = case.pop("source")
        path.write_text(source, encoding="utf-8", newline="\n")
        case.update({"source": str(path.resolve()), "source_sha256": hashlib.sha256(source.encode()).hexdigest()})
        (case_directory / "case.json").write_text(json.dumps(case, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        records.append(case)
    result = {"schema_version": 1, "generator_version": VERSION, "seed": seed,
        "bounds": {"positive_cases": cases, "negative_cases": 3, "max_shape_depth": MAX_DEPTH, "max_shape_nodes": MAX_NODES}, "cases": records}
    (directory / "manifest.json").write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return result


def verify_output(case: dict, stdout: str) -> None:
    actual = Counter(int(line.strip()) for line in stdout.splitlines() if line.strip().lstrip("-").isdigit())
    expected = Counter({int(key): count for key, count in case["expected_drop_counts"].items()})
    if actual != expected:
        raise RuntimeError(f"{case['name']}: exact Drop oracle failed: expected={dict(expected)}, observed={dict(actual)}")
    if "ownership-generated: passed" not in stdout:
        raise RuntimeError(f"{case['name']}: missing successful resource/String baseline checks")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--seed", type=lambda value: int(value, 0), default=DEFAULT_SEED)
    parser.add_argument("--cases", type=int, default=DEFAULT_CASES)
    parser.add_argument("--output", type=Path, default=Path("target/ownership-generated"))
    args = parser.parse_args()
    manifest = write_corpus(args.output.resolve(), args.seed, args.cases)
    print(f"ownership corpus: {len(manifest['cases'])} cases, seed={args.seed}; {args.output.resolve()}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
