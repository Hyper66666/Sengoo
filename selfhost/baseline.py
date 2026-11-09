"""Freeze/compare Windows daily workloads; provisional runs never qualify cutover."""
from __future__ import annotations

import argparse
import ctypes
from ctypes import wintypes
from datetime import datetime, timezone
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import queue
import shutil
import statistics
import subprocess
import sys
import threading
import time

ROOT = Path(__file__).resolve().parents[1]
CONFIG = ROOT / "selfhost/baseline-workloads.json"
# Reuse the existing Windows descendant working-set sampler, with its explicit
# sampling limitations. OS lifetime peak for the held root process is recorded
# separately, including processes too short for a sampling interval.
sys.path.insert(0, str(ROOT / "scripts"))
from measure_agent_workload import ProcessRss  # noqa: E402


def sha(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(65536), b""):
            digest.update(block)
    return digest.hexdigest()


def inventory(config: dict) -> dict[str, str]:
    files = {CONFIG, Path(__file__).resolve(), ROOT / "scripts/measure_agent_workload.py",
             ROOT / "scripts/package_ecosystem.py", ROOT / "rust-toolchain.toml",
             ROOT / "Cargo.toml", ROOT / "Cargo.lock"}
    files.update((ROOT / "selfhost/bench").glob("*.sg"))
    files.update(ROOT / path for path in config["authoritative_sources"])
    for group in config["compatibility_corpus"]:
        selected = set()
        for pattern in group["paths"]:
            selected.update(path for path in ROOT.glob(pattern) if path.is_file())
        if not selected:
            raise ValueError(f"corpus group has no files: {group['id']}")
        files.update(selected)
    # Native build inputs, not just the assertion/fixture corpus.
    for directory in ("compiler", "runtime", "tools/sgc", "tools/sgfmt", "tools/sgpm", "tools/sglsp", "tools/stdlib"):
        for path in (ROOT / directory).rglob("*"):
            if path.is_file() and path.suffix in (".rs", ".sg", ".c", ".h", ".toml"):
                if not any(part in ("build", "target", "__pycache__") for part in path.relative_to(ROOT / directory).parts):
                    files.add(path)
    return {path.relative_to(ROOT).as_posix(): sha(path) for path in sorted(files)}


def summary(values: list[float]) -> dict:
    ordered = sorted(values)
    return {"count": len(ordered), "min": ordered[0], "median": statistics.median(ordered),
            "p95": ordered[math.ceil(len(ordered) * .95) - 1], "max": ordered[-1]}


class RootMemory:
    """Query the OS lifetime high-water mark through Popen's retained handle."""
    def __init__(self):
        class Counters(ctypes.Structure):
            _fields_ = [("cb", wintypes.DWORD), ("PageFaultCount", wintypes.DWORD)] + [
                (name, ctypes.c_size_t) for name in ("PeakWorkingSetSize", "WorkingSetSize",
                 "QuotaPeakPagedPoolUsage", "QuotaPagedPoolUsage", "QuotaPeakNonPagedPoolUsage",
                 "QuotaNonPagedPoolUsage", "PagefileUsage", "PeakPagefileUsage")]
        self.Counters = Counters
        self.psapi = ctypes.WinDLL("psapi", use_last_error=True)
        self.psapi.GetProcessMemoryInfo.argtypes = [wintypes.HANDLE, ctypes.POINTER(Counters), wintypes.DWORD]

    def read(self, process: subprocess.Popen) -> int:
        counters = self.Counters()
        counters.cb = ctypes.sizeof(counters)
        if not self.psapi.GetProcessMemoryInfo(int(process._handle), ctypes.byref(counters), counters.cb):
            raise OSError(ctypes.get_last_error(), "GetProcessMemoryInfo")
        if counters.PeakWorkingSetSize <= 0:
            raise RuntimeError("OS returned no lifetime RSS peak")
        return int(counters.PeakWorkingSetSize)


def tool_identity(directory: Path, environment: dict[str, str]) -> dict:
    tools = {}
    for name in ("sgc", "sgfmt", "sgpm", "sglsp"):
        path = (directory / (name + ".exe")).resolve(strict=True)
        identity = {"path": str(path), "sha256": sha(path), "bytes": path.stat().st_size}
        version = subprocess.run([str(path), "--version"], env=environment, cwd=ROOT,
                                 capture_output=True, timeout=20, check=True)
        identity["version"] = version.stdout.decode("utf-8", "strict").strip()
        tools[name] = identity
    return tools


def qualification(path: Path | None, tools: dict) -> dict:
    if path is None:
        return {"qualified_release": False, "reason": "no qualification evidence supplied"}
    path = path.resolve(strict=True)
    data = json.loads(path.read_text(encoding="utf-8"))
    valid = data.get("result") == "passed" and data.get("profile") == "release"
    valid = valid and bool(data.get("source_revision"))
    for name, identity in tools.items():
        valid = valid and data.get("tools", {}).get(name, {}).get("sha256") == identity["sha256"]
    return {"path": str(path), "sha256": sha(path), "qualified_release": bool(valid),
            "source_revision": data.get("source_revision"),
            "reason": "matching release evidence" if valid else "evidence/profile/tool hash mismatch"}


def qualify_existing(args: argparse.Namespace) -> None:
    """Attach later release qualification without manufacturing new samples."""
    path = args.qualify_report.resolve(strict=True)
    report = json.loads(path.read_text(encoding="utf-8"))
    if report.get("result") not in ("provisional-reference-baseline", "provisional-comparison-passed"):
        raise ValueError("only a completed provisional measurement can be qualified")
    if report.get("measurement_context") != "quiet-host":
        raise ValueError("development/concurrent-load measurements cannot be promoted; collect a fresh quiet-host run")
    config = json.loads(CONFIG.read_text(encoding="utf-8"))
    if inventory(config) != report["sources"]:
        raise ValueError("measurement inputs changed since the measured run")
    if report.get("changed_sources") or report.get("changed_tools") or report.get("violations"):
        raise ValueError("measurement is not clean")
    qualification_inputs = {}
    for side, tools in report["tools"].items():
        for identity in tools.values():
            if sha(Path(identity["path"])) != identity["sha256"]:
                raise ValueError("measured executable changed")
        evidence = qualification(getattr(args, side + "_qualification"), tools)
        if not evidence["qualified_release"]:
            raise ValueError(f"{side}: release qualification is absent or mismatched")
        qualification_inputs[side] = evidence
    record = {"schema": 1, "result": "qualified-comparison-passed" if "candidate" in report["tools"] else "qualified-reference-baseline",
              "measurement_path": str(path), "measurement_sha256": sha(path),
              "config_sha256": sha(CONFIG), "qualifications": qualification_inputs,
              "recorded_at_utc": datetime.now(timezone.utc).isoformat(),
              "note": "original raw measurement preserved; current source/tool fingerprints revalidated; no new samples claimed"}
    destination = path.with_name("qualification.json")
    destination.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    print(str(destination))


class Runner:
    def __init__(self, output: Path, tools: dict, environment: dict, config: dict, side: str, report: dict):
        self.output, self.tools, self.env, self.config, self.side, self.report = output, tools, environment, config, side, report
        self.directory = output / side
        self.directory.mkdir(exist_ok=True)
        self.logs = self.directory / "logs"
        self.logs.mkdir(exist_ok=True)
        self.project = self.directory / "project"
        self.project.mkdir(exist_ok=True)
        for path in (ROOT / "selfhost/bench").glob("*.sg"):
            shutil.copyfile(path, self.project / path.name)
        self.binary = self.project / "program.exe"
        # Deliberately exercise the supported embedded analyzer. A sibling sgc
        # is discovered by Windows before PATH, so isolate the identical LSP
        # bytes as well as clearing PATH for this one workload.
        self.lsp_image = self.directory / "embedded-lsp.exe"
        shutil.copyfile(tools["sglsp"]["path"], self.lsp_image)
        if sha(self.lsp_image) != tools["sglsp"]["sha256"]:
            raise ValueError("isolated LSP copy hash mismatch")
        self.memory = RootMemory()

    def sgc(self, *arguments: str | Path) -> list[str | Path]:
        return [self.tools["sgc"]["path"], "--runtime-mode", self.config["runtime_mode"], *arguments]

    def execute(self, name: str, command: list[str | Path], expected: bytes | None = None) -> dict:
        started = time.perf_counter()
        process = subprocess.Popen(list(map(str, command)), cwd=ROOT, env=self.env,
                                   stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        monitor = ProcessRss(process.pid)
        try:
            stdout, stderr = process.communicate(timeout=600)
        except BaseException:
            process.kill()
            process.communicate()
            raise
        finally:
            elapsed = (time.perf_counter() - started) * 1000
            root_peak = self.memory.read(process)
            rss = monitor.finish()
        (self.logs / (name + ".stdout.log")).write_bytes(stdout)
        (self.logs / (name + ".stderr.log")).write_bytes(stderr)
        record = {"side": self.side, "name": name, "command": list(map(str, command)),
                  "exit_code": process.returncode, "elapsed_ms": elapsed,
                  "root_peak_rss_bytes": root_peak, "rss": rss,
                  "stdout_sha256": hashlib.sha256(stdout).hexdigest(),
                  "stderr_sha256": hashlib.sha256(stderr).hexdigest()}
        self.report["raw"].append(record)
        if process.returncode != 0 or (expected is not None and stdout.replace(b"\r\n", b"\n") != expected):
            raise RuntimeError(f"{self.side}/{name}: failed result or output; see logs")
        if rss["sample_errors"]:
            raise RuntimeError(f"{self.side}/{name}: memory sampling errors")
        return record

    def lsp(self, name: str) -> dict:
        started = time.perf_counter()
        lsp_env = {key: value for key, value in self.env.items() if not key.upper().startswith("SENGOO_")}
        lsp_env["PATH"] = ""
        lsp_env["RUST_LOG"] = "off"
        process = subprocess.Popen([str(self.lsp_image)], cwd=self.project, env=lsp_env,
                                   stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        monitor = ProcessRss(process.pid)
        messages = queue.Queue()
        raw_stdout, raw_stderr = bytearray(), bytearray()

        def receive():
            try:
                while True:
                    headers = {}
                    while True:
                        line = process.stdout.readline()
                        raw_stdout.extend(line)
                        if not line:
                            raise EOFError("LSP stdout ended")
                        if line == b"\r\n":
                            break
                        key, value = line.decode("ascii").split(":", 1)
                        headers[key.lower()] = value.strip()
                    length = int(headers["content-length"])
                    if not 0 <= length <= 16 * 1024 * 1024:
                        raise ValueError("LSP frame length")
                    body = process.stdout.read(length)
                    raw_stdout.extend(body)
                    if len(body) != length:
                        raise EOFError("LSP partial frame")
                    messages.put(json.loads(body))
            except BaseException as error:
                messages.put(error)

        def errors():
            raw_stderr.extend(process.stderr.read())

        reader = threading.Thread(target=receive, daemon=True)
        stderr_reader = threading.Thread(target=errors, daemon=True)
        reader.start(); stderr_reader.start()

        def send(message: dict):
            body = json.dumps({"jsonrpc": "2.0", **message}, ensure_ascii=False).encode("utf-8")
            process.stdin.write(f"Content-Length: {len(body)}\r\n\r\n".encode() + body)
            process.stdin.flush()

        def wait(predicate):
            deadline = time.monotonic() + 30
            while True:
                message = messages.get(timeout=max(.001, deadline - time.monotonic()))
                if isinstance(message, BaseException):
                    raise message
                if predicate(message):
                    return message
                if time.monotonic() >= deadline:
                    raise TimeoutError("LSP expected message")

        latencies = {}
        try:
            before = time.perf_counter()
            send({"id": 1, "method": "initialize", "params": {"processId": None,
                  "rootUri": self.project.as_uri(), "capabilities": {},
                  "workspaceFolders": [{"uri": self.project.as_uri(), "name": "baseline"}]}})
            initialized = wait(lambda value: value.get("id") == 1)
            if "error" in initialized or "capabilities" not in initialized.get("result", {}):
                raise ValueError("LSP initialize result")
            latencies["initialize_ms"] = (time.perf_counter() - before) * 1000
            send({"method": "initialized", "params": {}})
            uri = (self.project / "lsp.sg").as_uri()
            text = (self.project / "lsp.sg").read_text(encoding="utf-8")
            send({"method": "textDocument/didOpen", "params": {"textDocument": {
                  "uri": uri, "languageId": "sengoo", "version": 1, "text": text}}})
            clean = wait(lambda value: value.get("method") == "textDocument/publishDiagnostics"
                         and value.get("params", {}).get("uri") == uri)
            if clean["params"]["diagnostics"]:
                raise ValueError("valid LSP fixture produced diagnostics")
            before = time.perf_counter()
            send({"method": "textDocument/didChange", "params": {"textDocument": {"uri": uri, "version": 2},
                  "contentChanges": [{"text": text.replace("value: 41", "value: unknown_baseline_symbol")}]}})
            invalid = wait(lambda value: value.get("method") == "textDocument/publishDiagnostics"
                           and value.get("params", {}).get("uri") == uri
                           and bool(value.get("params", {}).get("diagnostics")))
            latencies["diagnostic_ms"] = (time.perf_counter() - before) * 1000
            if not any(item.get("severity") == 1 and "unknown_baseline_symbol" in item.get("message", "")
                       for item in invalid["params"]["diagnostics"]):
                raise ValueError("LSP did not report the expected undefined identifier")
            send({"id": 99, "method": "shutdown"})
            shutdown = wait(lambda value: value.get("id") == 99)
            if "error" in shutdown:
                raise ValueError("LSP shutdown rejected")
            send({"method": "exit"})
            process.stdin.close()
            process.wait(timeout=20)
            if process.returncode != 0:
                raise RuntimeError("LSP exited unsuccessfully")
        finally:
            if process.poll() is None:
                process.kill(); process.wait(timeout=10)
            reader.join(timeout=5); stderr_reader.join(timeout=5)
            root_peak = self.memory.read(process)
            rss = monitor.finish()
            (self.logs / (name + ".stdout.log")).write_bytes(raw_stdout)
            (self.logs / (name + ".stderr.log")).write_bytes(raw_stderr)
            record = {"side": self.side, "name": name, "command": [str(self.lsp_image)],
                      "exit_code": process.returncode, "elapsed_ms": latencies.get("diagnostic_ms"),
                      "lifecycle_ms": (time.perf_counter() - started) * 1000,
                      "latencies": latencies, "root_peak_rss_bytes": root_peak, "rss": rss,
                      "analysis_mode": "embedded; no sibling or PATH sgc; external diagnostic chain is a separate compatibility gate"}
            self.report["raw"].append(record)
        if rss["sample_errors"]:
            raise RuntimeError("LSP memory sampling errors")
        return record

    def prepare(self):
        self.execute("prepare-runtime", self.sgc("build", self.project / "main.sg", "--force-rebuild",
                                               "-O", "2", "-o", self.binary))
        self.execute("prepare-runtime-check", [self.binary], b"selfhost baseline runtime: ok\n7\n")

    def workload(self, workload: str, iteration: int) -> dict:
        name = f"{workload}-{iteration:02d}"
        main = self.project / "main.sg"
        if workload == "startup":
            return self.execute(name, [self.tools["sgc"]["path"], "--version"])
        if workload == "check":
            return self.execute(name, self.sgc("check", main))
        if workload == "build-full":
            return self.execute(name, self.sgc("build", main, "--force-rebuild", "-O", "2", "-o", self.binary))
        if workload == "build-incremental":
            library = (ROOT / "selfhost/bench/library.sg").read_text(encoding="utf-8")
            (self.project / "library.sg").write_text(library.replace("    7;", f"    {7 + iteration % 2};"), encoding="utf-8")
            result = self.execute(name, self.sgc("build", main, "-O", "2", "-o", self.binary))
            # Verify actual edit results outside the timed build interval.
            revision = 7 + iteration % 2
            expected = f"selfhost baseline runtime: ok\n{revision}\n".encode("ascii")
            self.execute(name + "-verify", [self.binary], expected)
            result["expected_revision"] = revision
            return result
        if workload == "format":
            return self.execute(name, [self.tools["sgfmt"]["path"], "--check", main])
        if workload == "runtime":
            return self.execute(name, [self.binary], b"selfhost baseline runtime: ok\n7\n")
        if workload == "package-metadata":
            result = self.execute(name, [self.tools["sgpm"]["path"], "--runtime-mode", "source-development",
                                  "metadata", "--locked", "--manifest-path",
                                  ROOT / "examples/compat/v0.2.0-rc.1/Sengoo.toml", "--format", "json"])
            metadata = json.loads((self.logs / (name + ".stdout.log")).read_bytes())
            if not metadata.get("packages"):
                raise ValueError("package metadata omitted package graph")
            return result
        if workload == "lsp-diagnostics":
            return self.lsp(name)
        raise ValueError("unknown frozen workload")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference-dir", type=Path)
    parser.add_argument("--candidate-dir", type=Path)
    parser.add_argument("--reference-qualification", type=Path)
    parser.add_argument("--candidate-qualification", type=Path)
    parser.add_argument("--clang", type=Path)
    parser.add_argument("--qualify-report", type=Path,
                        help="revalidate a provisional report against subsequently completed release qualification")
    parser.add_argument("--measurement-context", choices=("development", "quiet-host"), default="development",
                        help="quiet-host declares no competing build/test workload; development observations cannot qualify cutover")
    parser.add_argument("--output", type=Path, default=ROOT / "target/selfhost/m0")
    args = parser.parse_args()
    if args.qualify_report:
        qualify_existing(args)
        return
    if args.reference_dir is None or args.clang is None:
        parser.error("measurement requires --reference-dir and --clang")
    if os.name != "nt" or platform.machine().lower() not in ("amd64", "x86_64"):
        raise ValueError("native Windows x64 required")
    config = json.loads(CONFIG.read_text(encoding="utf-8"))
    if config["schema"] != 1 or config["samples"] < 10 or config["warmups"] < 1:
        raise ValueError("invalid baseline policy")
    output = args.output.resolve()
    if not output.is_relative_to(ROOT / "target"):
        raise ValueError("output must be inside ignored repository target/")
    output.mkdir(parents=True, exist_ok=True)
    report_path = output / "evidence.json"
    report = {"schema": 1, "result": "running", "recorded_at_utc": datetime.now(timezone.utc).isoformat(),
              "measurement_context": args.measurement_context,
              "host": {"name": platform.node(), "platform": platform.platform(), "machine": platform.machine(),
                       "processor": platform.processor(), "python": sys.version}, "raw": [], "summary": {},
              "config_sha256": sha(CONFIG), "limits": [config["cache_policy"],
              "not a compiler-correctness, compatibility-corpus pass, self-hosting, installation or cross-host proof",
              "Windows RSS is resident working set; tree sampling may miss short-lived descendants and double-count shared pages",
              "root peak uses OS lifetime peak from a retained process handle; timing includes the same observer setup for both tools",
              "quiet-host is an operator declaration, not proof of an isolated VM or zero background OS activity"]}
    report_path.write_text(json.dumps(report, indent=2), encoding="utf-8")
    sources, identities = {}, {}
    try:
        sources = inventory(config)
        report["sources"] = sources
        report["source_revision"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
        report["working_tree"] = subprocess.check_output(["git", "status", "--porcelain=v1"], cwd=ROOT, text=True)
        env = dict(os.environ)
        # No environment values are serialized. Eliminate optional compiler and
        # tool redirection while retaining normal Windows loader/SDK settings.
        for key in list(env):
            if key.upper().startswith("SENGOO_") or key.upper() in ("RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_RUSTC"):
                del env[key]
        clang = args.clang.resolve(strict=True)
        if sha(clang) != "56ef1abd0df11a3b1b77822193e927c6345078c24511567ae10b3afd803d03a3":
            raise ValueError("unverified clang")
        env.update(SENGOO_STDLIB=str(ROOT / "tools/stdlib"), SENGOO_RUNTIME=str(ROOT / "tools/stdlib/runtime.c"), SENGOO_CLANG=str(clang))
        env["PATH"] = str(clang.parent) + os.pathsep + env.get("PATH", "")
        report["native_tools"] = {"clang": {"path": str(clang), "sha256": sha(clang)}}
        runners = {}
        qualifications = {}
        for side in ("reference", "candidate"):
            directory = getattr(args, side + "_dir")
            if directory is None:
                continue
            identities[side] = tool_identity(directory, env)
            qualifications[side] = qualification(getattr(args, side + "_qualification"), identities[side])
            runners[side] = Runner(output, identities[side], env, config, side, report)
        report["tools"], report["qualifications"] = identities, qualifications
        for runner in runners.values():
            runner.prepare()
        names = [workload["id"] for workload in config["performance_workloads"]]
        measured = {side: {name: [] for name in names} for side in runners}
        # Alternate AB/BA per sample to balance order and drift. Warmups use
        # the same behavior but are excluded from every summary and decision.
        for name in names:
            for iteration in range(-config["warmups"], config["samples"]):
                sides = list(runners)
                if iteration % 2:
                    sides.reverse()
                for side in sides:
                    record = runners[side].workload(name, iteration + config["warmups"])
                    record.update(workload=name, iteration=iteration, warmup=iteration < 0)
                    if iteration >= 0:
                        measured[side][name].append(record)
                    report_path.write_text(json.dumps(report, indent=2, ensure_ascii=False), encoding="utf-8")
                print(f"{name}: {'warmup' if iteration < 0 else iteration + 1}/{config['samples']}", flush=True)
        violations = []
        for side, workloads in measured.items():
            report["summary"][side] = {}
            for name, records in workloads.items():
                report["summary"][side][name] = {
                    "elapsed_ms": summary([record["elapsed_ms"] for record in records]),
                    "root_peak_rss_bytes": summary([record["root_peak_rss_bytes"] for record in records]),
                    "sampled_tree_peak_rss_bytes": summary([max(record["root_peak_rss_bytes"], record["rss"]["sampled_process_tree_peak_rss_bytes"]) for record in records])}
        if "candidate" in measured:
            report["ratios"] = {}
            for name in names:
                ratios = {}
                for metric, limit in (("elapsed_ms", config["max_time_ratio"]), ("root_peak_rss_bytes", config["max_peak_memory_ratio"]),
                                      ("sampled_tree_peak_rss_bytes", config["max_peak_memory_ratio"])):
                    ref = report["summary"]["reference"][name][metric]["median"]
                    candidate = report["summary"]["candidate"][name][metric]["median"]
                    ratios[metric] = candidate / ref
                    if ratios[metric] > limit:
                        violations.append(f"{name}/{metric}: {ratios[metric]:.4f} > {limit}")
                report["ratios"][name] = ratios
        report["violations"] = violations
        qualified = args.measurement_context == "quiet-host" and all(item["qualified_release"] for item in qualifications.values())
        report["result"] = "failed" if violations else (
            "qualified-comparison-passed" if qualified and "candidate" in runners else
            "qualified-reference-baseline" if qualified else
            "provisional-comparison-passed" if "candidate" in runners else "provisional-reference-baseline")
        if violations:
            raise RuntimeError("performance budget exceeded; see evidence")
    except BaseException as error:
        report.update(result="failed", error=repr(error))
        raise
    finally:
        changed = []
        if sources:
            try:
                current = inventory(config)
                changed = sorted(path for path in sources.keys() | current.keys() if sources.get(path) != current.get(path))
            except BaseException as error:
                changed = ["inventory failed: " + repr(error)]
        report["changed_sources"] = changed
        changed_tools = []
        for side, tools in identities.items():
            for name, identity in tools.items():
                try:
                    if sha(Path(identity["path"])) != identity["sha256"]:
                        changed_tools.append(side + "/" + name)
                except OSError:
                    changed_tools.append(side + "/" + name)
        report["changed_tools"] = changed_tools
        if changed or changed_tools:
            report.update(result="failed", error="source/tool changed during measurement")
        report_path.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
        if changed or changed_tools:
            raise RuntimeError(report["error"])


if __name__ == "__main__":
    main()
