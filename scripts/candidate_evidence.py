#!/usr/bin/env python3
"""Collect exact-revision candidate checks without relaxing stable release gates."""
from __future__ import annotations

import argparse
from collections import Counter
from datetime import datetime, timezone
import json
from pathlib import Path
import re
import subprocess
import sys
from typing import Any


HOSTS = ("ubuntu-latest", "windows-latest", "macos-15", "macos-15-intel")
TARGETS = ("x86_64-unknown-linux-gnu", "x86_64-pc-windows-msvc",
           "aarch64-apple-darwin", "x86_64-apple-darwin")
PLATFORMS = ("linux-x64", "windows-x64", "macos-arm64", "macos-x64")
PYTHONS = ("3.11", "3.12", "3.13", "3.14")
WORKFLOWS = {
    "core-conformance.yml": "core-conformance",
    "native-safety.yml": "native-safety",
    "hardening-fuzz.yml": "hardening-fuzz",
    "compatibility.yml": "compatibility-prerelease",
    "realworld-e2e.yml": "realworld-e2e",
    "perf-smoke.yml": "perf-smoke",
    "toolchain-distribution.yml": "toolchain-distribution",
}


def required_jobs(workflow: str) -> list[str]:
    return {
        "core-conformance.yml": ["core-language"],
        "native-safety.yml": [*(f"generated ownership sanitizers ({host})" for host in HOSTS),
                              "linux-sanitizers", "bounded-longevity"],
        "hardening-fuzz.yml": ["bounded-input-smoke"],
        "compatibility.yml": ["previous-and-current"],
        "realworld-e2e.yml": [*(f"realworld locked loop ({target})" for target in TARGETS),
            "graphics package smoke (ubuntu-latest)", "graphics package smoke (windows-latest)",
            *(f"wheel-build ({platform}, CPython 3.11)" for platform in PLATFORMS),
            *(f"wheel-consumer ({platform}, CPython {version})"
              for platform in PLATFORMS for version in PYTHONS), "python-wheel-gate"],
        "perf-smoke.yml": ["compile-scale-production-gate"],
        "toolchain-distribution.yml": [f"package smoke ({host})" for host in HOSTS],
    }[workflow]


def required_artifacts(workflow: str, sha: str, run_id: int) -> list[str]:
    return {
        "core-conformance.yml": ["debugger-native-lldb-transcripts"],
        "native-safety.yml": [*(f"generated-ownership-sanitizers-{host}-{sha}" for host in HOSTS),
                              f"official-package-sanitizers-{sha}", "native-longevity-transcript"],
        "hardening-fuzz.yml": [f"hardening-fuzz-corpus-{run_id}"],
        "compatibility.yml": ["compatibility-transcript"],
        "realworld-e2e.yml": [*(f"daily-development-{target}-{sha}" for target in TARGETS),
            *(f"python-wheels-{platform}-{sha}" for platform in PLATFORMS),
            *(f"python-wheel-build-evidence-{platform}-{sha}" for platform in PLATFORMS),
            *(f"python-wheel-evidence-{platform}-cp{version}-{sha}"
              for platform in PLATFORMS for version in PYTHONS)],
        "perf-smoke.yml": ["production-performance-evidence"],
        "toolchain-distribution.yml": [*(f"sengoo-toolchain-{platform}" for platform in
            ("linux-x86_64", "windows-x86_64", "macos-arm64", "macos-x86_64")),
            "sengoo-reproducibility-linux-x86_64", "sengoo-reproducibility-windows-x86_64",
            "debugger-native-cdb-transcripts"],
    }[workflow]


def required_steps(workflow: str, name: str) -> list[str]:
    if name.startswith("generated ownership sanitizers ("):
        return ["Build the candidate compiler", "Check generated-case reproducibility and failure retention",
                "Instrument generated ownership code and seven split runtimes"]
    if name.startswith("wheel-build ("):
        if "linux-x64" in name:
            return ["Install source-build toolchains inside manylinux", "Build true manylinux wheels"]
        repair = ("Repair Windows DLL dependencies" if "windows-x64" in name
                  else "Bundle macOS dynamic dependencies and validate deployment tag")
        return ["Build toolchain and backend", "Build two external consumers", repair]
    if name.startswith("wheel-consumer ("):
        return ["Verify offline wheel installation with build tools absent from PATH"]
    if name.startswith("realworld locked loop ("):
        return ["Build toolchain binaries", "Run realworld integration tests", "Verify package release defaults",
                "Run LSP realworld diagnostics", "Validate canonical package registry spec", "Package toolchain",
                "Run installed realworld fixture loop", "Build official offline package artifacts",
                "Verify installed daily development and external consumers",
                "Verify atomic incremental imports after removing consumer sources",
                "Verify installed sgc HTTP TLS composition", "Verify reviewed official package set"]
    if name.startswith("graphics package smoke ("):
        return ["Build package tools", "Verify sgplatform package", "Verify sggame package", "Verify sggui package"]
    if workflow == "toolchain-distribution.yml":
        host = "Windows" if "windows-latest" in name else "POSIX"
        steps = ["Build release tools", "Verify timer socket and owned-handle reactor paths",
                 "Verify production HTTP TLS composition", "Run release smoke matrix", "Verify installer target detection",
                 "Package toolchain", "Prepare local release feed", f"Install package ({host})",
                 "Installed stdlib smoke", f"Install package by version ({host})", f"Upgrade package by version ({host})"]
        if "windows-latest" in name:
            steps += ["Capture required Windows CDB debugger evidence"]
        if "windows-latest" in name or "ubuntu-latest" in name:
            steps += [f"Install reproducibility build B ({host})", "Upload reproducibility evidence"]
        return steps
    return {
        "core-language": ["Validate OpenSpec", "Run core conformance examples", "Verify candidate evidence fails closed",
                          "Run portable target and ABI contract smoke tests", "Capture required LLDB debugger evidence",
                          "Run workspace tests"],
        "linux-sanitizers": ["Run split C runtime under ASan and UBSan",
                             "Run official package native boundaries under ASan and UBSan",
                             "Run Rust runtime and FFI tests under ASan"],
        "bounded-longevity": ["Repeat cancellation, channel, lock, and reactor stress"],
        "bounded-input-smoke": ["Replay compiler corpus and bounded generated inputs", "Replay package and archive corpus",
                                "Exercise runtime parser and decoder boundaries"],
        "previous-and-current": ["Build current toolchain", "Install previous supported prerelease",
                                 "Run retained project with previous and current toolchains outside-checkout"],
        "compile-scale-production-gate": ["Build sgc (release)", "Smoke release_resource_gate against the real sgc",
            "Run advanced pipeline benchmark", "Enforce compile wall-time, RSS, and regression budgets",
            "Enforce artifact, startup, CLI, and runtime budgets"],
        "python-wheel-gate": ["Require all four hosts and sixteen interpreter consumers"],
    }.get(name, [])


def timestamp(value: str) -> datetime:
    parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    if parsed.tzinfo is None:
        raise ValueError("timestamp must include timezone")
    return parsed


def validate(document: dict[str, Any], now: datetime | None = None) -> list[str]:
    """Fail closed on incomplete API snapshots; preserve all findings in the report."""
    errors: list[str] = []
    now = now or datetime.now(timezone.utc)
    sha = document.get("source_revision", "")
    repo = document.get("repository", "")
    branch = document.get("source_ref", "")
    if document.get("schema_version") != 1 or document.get("kind") != "candidate":
        errors.append("expected candidate schema version 1")
    if not re.fullmatch(r"[0-9a-f]{40}", sha):
        errors.append("a full committed source revision is required")
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repo) or not branch:
        errors.append("repository and source branch are required")
    runs = document.get("runs", [])
    for workflow, name in WORKFLOWS.items():
        matches = [run for run in runs if run.get("workflow_file") == workflow]
        if len(matches) != 1:
            errors.append(f"{workflow}: expected exactly one run, found {len(matches)}")
            continue
        run = matches[0]
        prefix = f"{workflow} run {run.get('id')}"
        try:
            attempt_start = timestamp(run["attempt_started_at"])
            if not isinstance(run.get("run_attempt"), int) or run["run_attempt"] < 1:
                raise ValueError("invalid attempt")
        except (KeyError, TypeError, ValueError):
            errors.append(f"{prefix}: a valid current attempt identity and start time are required")
            attempt_start = now
        if run.get("name") != name or str(run.get("path", "")).split("@", 1)[0] != f".github/workflows/{workflow}":
            errors.append(f"{prefix}: workflow identity mismatch")
        if run.get("head_sha") != sha or run.get("head_branch") != branch:
            errors.append(f"{prefix}: source revision or branch mismatch")
        if run.get("head_repository", {}).get("full_name") != repo:
            errors.append(f"{prefix}: source repository mismatch")
        event = run.get("event")
        if event not in ("workflow_dispatch", "pull_request", "push") or (event == "push" and branch != "main"):
            errors.append(f"{prefix}: unsupported candidate event {event}")
        if run.get("status") != "completed" or run.get("conclusion") != "success":
            errors.append(f"{prefix}: required run did not complete successfully")
        jobs = run.get("jobs", [])
        # Reusable workflows are displayed as 'caller / callee'. Matrix names
        # carry target/version identities explicitly and must occur exactly once.
        names = Counter(str(job.get("name", "")).rsplit(" / ", 1)[-1] for job in jobs)
        for required in required_jobs(workflow):
            if names[required] != 1:
                errors.append(f"{prefix}: required job {required!r} count is {names[required]}")
        for job in jobs:
            job_name = str(job.get("name", ""))
            allowed_skip = workflow == "toolchain-distribution.yml" and (
                job_name.startswith("published upgrade and rollback") or
                job_name == "publish complete toolchain release")
            if allowed_skip:
                if job.get("conclusion") != "skipped":
                    errors.append(f"{prefix}: tag-only job {job_name!r} must remain skipped for a candidate")
                continue
            if job.get("status") != "completed" or job.get("conclusion") != "success":
                errors.append(f"{prefix}: required job {job_name!r} failed, skipped or incomplete")
            identity = [step for step in job.get("steps", []) if step.get("name") == "Verify candidate checkout"]
            if len(identity) != 1 or identity[0].get("status") != "completed" or identity[0].get("conclusion") != "success":
                errors.append(f"{prefix}: job {job_name!r} lacks successful checkout identity proof")
            for required_step in required_steps(workflow, job_name.rsplit(" / ", 1)[-1]):
                matching_steps = [step for step in job.get("steps", []) if step.get("name") == required_step]
                if (len(matching_steps) != 1 or matching_steps[0].get("status") != "completed"
                        or matching_steps[0].get("conclusion") != "success"):
                    errors.append(f"{prefix}: job {job_name!r} required step {required_step!r} did not succeed")
        artifacts = run.get("artifacts", [])
        for artifact_name in required_artifacts(workflow, sha, run.get("id", 0)):
            artifacts_named = [artifact for artifact in artifacts if artifact.get("name") == artifact_name]
            if len(artifacts_named) != 1:
                errors.append(f"{prefix}: required artifact {artifact_name!r} count is {len(artifacts_named)}")
                continue
            artifact = artifacts_named[0]
            try:
                expired = timestamp(artifact["expires_at"]) <= now
                stale = timestamp(artifact["created_at"]) < attempt_start
            except (ValueError, KeyError, TypeError):
                expired = True
                stale = True
            if artifact.get("expired") is not False or expired or artifact.get("size_in_bytes", 0) <= 0:
                errors.append(f"{prefix}: required artifact {artifact_name!r} expired, empty or invalid")
            if stale:
                errors.append(f"{prefix}: required artifact {artifact_name!r} predates the current attempt")
            origin = artifact.get("workflow_run", {})
            if origin.get("head_sha") != sha or origin.get("id") != run.get("id"):
                errors.append(f"{prefix}: artifact {artifact_name!r} belongs to another revision or run")
    extras = [run.get("workflow_file") for run in runs if run.get("workflow_file") not in WORKFLOWS]
    if extras:
        errors.append(f"unexpected workflow snapshots: {extras}")
    return errors


class GitHub:
    def __init__(self, executable: str, repository: str):
        self.executable = executable
        self.repository = repository

    def command(self, *args: str) -> str:
        result = subprocess.run([self.executable, *args], text=True, capture_output=True, check=False)
        if result.returncode:
            raise RuntimeError(result.stderr.strip() or f"gh failed with exit {result.returncode}")
        return result.stdout

    def api(self, path: str) -> Any:
        return json.loads(self.command("api", f"repos/{self.repository}/{path}"))

    def pages(self, path: str, key: str) -> list[dict[str, Any]]:
        result: list[dict[str, Any]] = []
        separator = "&" if "?" in path else "?"
        for page in range(1, 1001):
            response = self.api(f"{path}{separator}per_page=100&page={page}")
            values = response[key]
            result.extend(values)
            if len(values) < 100:
                return result
        raise RuntimeError("GitHub pagination exceeded the bounded limit")


def collect(github: GitHub, sha: str, branch: str) -> dict[str, Any]:
    document: dict[str, Any] = {"schema_version": 1, "kind": "candidate",
        "repository": github.repository, "source_revision": sha, "source_ref": branch,
        "generated_at_utc": datetime.now(timezone.utc).isoformat(), "runs": [], "collection_errors": [],
        "stable_release_qualified": False,
        "deferred_gates": ["successful six main-push gates and published release transition",
                           "independent two-week human pilot (deferred by owner)"]}
    for workflow in WORKFLOWS:
        try:
            available = github.pages(f"actions/workflows/{workflow}/runs?head_sha={sha}", "workflow_runs")
            matching = [run for run in available if run.get("head_sha") == sha and run.get("head_branch") == branch
                and run.get("event") in ("workflow_dispatch", "pull_request", "push")]
            if not matching:
                continue
            # Never select an older green run to hide a newer failed/pending run.
            run = max(matching, key=lambda value: (value["created_at"], value["id"], value.get("run_attempt", 1)))
            attempt = run.get("run_attempt", 1)
            run["workflow_file"] = workflow
            attempt_detail = github.api(f"actions/runs/{run['id']}/attempts/{attempt}")
            if attempt_detail.get("head_sha") != sha or attempt_detail.get("run_attempt") != attempt:
                raise RuntimeError("attempt metadata does not identify the selected revision/attempt")
            run["attempt_started_at"] = attempt_detail["run_started_at"]
            run["jobs"] = github.pages(f"actions/runs/{run['id']}/attempts/{attempt}/jobs", "jobs")
            artifacts = github.pages(f"actions/runs/{run['id']}/artifacts", "artifacts")
            run["artifacts"] = []
            run["prior_attempt_artifacts"] = []
            for artifact in artifacts:
                # A rerun's API lists old artifacts as well. Preserve their
                # inventory separately; they cannot satisfy current-attempt gates.
                if timestamp(artifact["created_at"]) < timestamp(run["attempt_started_at"]):
                    run["prior_attempt_artifacts"].append(artifact)
                else:
                    run["artifacts"].append(artifact)
            current = github.api(f"actions/runs/{run['id']}")
            if (current.get("run_attempt") != attempt or current.get("head_sha") != sha
                    or current.get("status") != run.get("status") or current.get("conclusion") != run.get("conclusion")):
                raise RuntimeError("workflow changed during collection; collect a fresh snapshot")
            document["runs"].append(run)
        except (RuntimeError, KeyError, ValueError) as error:
            document["collection_errors"].append(f"{workflow}: {error}")
    document["errors"] = [*document["collection_errors"], *validate(document)]
    document["ok"] = not document["errors"]
    return document


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="mode", required=True)
    collector = sub.add_parser("collect", help="capture current GitHub results, retaining incomplete evidence")
    collector.add_argument("--repo", required=True)
    collector.add_argument("--sha", required=True)
    collector.add_argument("--ref", required=True, help="candidate branch name")
    collector.add_argument("--gh", default="gh")
    collector.add_argument("--output", type=Path, required=True)
    checker = sub.add_parser("validate", help="revalidate a retained candidate snapshot, including expiry")
    checker.add_argument("path", type=Path)
    args = parser.parse_args()
    try:
        if args.mode == "collect":
            if not re.fullmatch(r"[0-9a-f]{40}", args.sha):
                parser.error("--sha must be a full committed lowercase SHA")
            github = GitHub(args.gh, args.repo)
            document = collect(github, args.sha, args.ref)
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")
            errors = document["errors"]
        else:
            document = json.loads(args.path.read_text(encoding="utf-8"))
            errors = [*document.get("collection_errors", []), *validate(document)]
        for error in errors:
            print(error, file=sys.stderr)
        print("Candidate checks passed; stable release and independent pilot remain separate."
              if not errors else f"Candidate remains incomplete: {len(errors)} finding(s).")
        return int(bool(errors))
    except (OSError, ValueError, TypeError, KeyError) as error:
        print(f"Cannot validate candidate evidence: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
