"""Reproducible synthetic SQLite/OS-lock verification; never opens user state."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
import pathlib
import sqlite3
import subprocess
import tempfile
import time


def digest(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--compiler", type=pathlib.Path, required=True)
    parser.add_argument("--clang", type=pathlib.Path, required=True)
    parser.add_argument("--formatter", type=pathlib.Path, required=True)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    parser.add_argument("--executable", type=pathlib.Path)
    parser.add_argument("--build-inputs", type=pathlib.Path)
    args = parser.parse_args()
    repo = pathlib.Path(__file__).resolve().parents[3]
    output = args.output.resolve(); output.parent.mkdir(parents=True, exist_ok=True)
    compiler = args.compiler.resolve(); clang = args.clang.resolve(); formatter = args.formatter.resolve()
    for tool in (compiler, clang, formatter):
        assert tool.is_file(), tool
    sources = [repo / p for p in ("packages/sgagent/src/lib.sg", "packages/sgagent/src/sqlite_store.sg",
        "packages/sgagent/tests/sqlite_store.sg", "packages/sgschema/src/lib.sg", "packages/sgsqlite/src/lib.sg")]
    environment = dict(os.environ)
    environment.update(SENGOO_CLANG=str(clang), SENGOO_STDLIB=str(repo / "tools/stdlib"),
        SENGOO_RUNTIME=str(repo / "tools/stdlib/runtime.c"),
        SENGOO_MODULE_MAP=";".join(f"{name}={repo / 'packages' / name / 'src/lib.sg'}" for name in ("sgagent", "sgschema", "sgsqlite")))
    environment["PATH"] = str(clang.parent) + os.pathsep + environment.get("PATH", "")
    records: list[dict] = []
    executable = args.executable.resolve() if args.executable else output.parent / "sqlite-store.exe"

    def run(name: str, command: list[str], *, cwd=repo, expected=0, timeout=240) -> subprocess.CompletedProcess:
        result = subprocess.run(list(map(str, command)), cwd=cwd, env=environment,
            capture_output=True, encoding="utf-8", errors="replace", timeout=timeout)
        (output.parent / (name + ".log")).write_text(result.stdout + result.stderr, encoding="utf-8")
        records.append({"name": name, "exit_code": result.returncode, "stdout": result.stdout, "stderr": result.stderr})
        if expected is not None:
            assert result.returncode == expected, (name, result.returncode, result.stdout, result.stderr)
        return result

    if args.executable:
        assert args.build_inputs is not None, "an existing executable requires a captured build manifest"
        build_inputs = json.loads(args.build_inputs.read_text(encoding="utf-8"))
        assert digest(executable) == build_inputs["executable_sha256"], "executable differs from captured build"
        assert digest(compiler) == build_inputs["compiler_sha256"], "compiler differs from captured build"
    else:
        before = [{"path": str(p.relative_to(repo)), "sha256": digest(p)} for p in sources]
        for index, source in enumerate(sources[:3]):
            run(f"format-{index}", [formatter, "--check", source])
        run("native-build", [compiler, "--runtime-mode", "source-development", "build", sources[2],
            "--force-rebuild", "-O", "0", "-o", executable])
        assert before == [{"path": str(p.relative_to(repo)), "sha256": digest(p)} for p in sources], "sources changed during build"
        build_inputs = {"schema": 1, "compiler_sha256": digest(compiler), "executable_sha256": digest(executable), "sources": before}
        (output.parent / "build-inputs.json").write_text(json.dumps(build_inputs, ensure_ascii=False, indent=2), encoding="utf-8")

    scratch = pathlib.Path(tempfile.mkdtemp(prefix="sgagent-ledger-中文😀-"))
    holder = None
    evidence = {"schema": 1, "result": "running", "scope": "Windows actual SQLite/OS lock and synthetic kernel; no tool process launched",
        "compiler_sha256": digest(compiler), "formatter_sha256": digest(formatter), "executable_sha256": digest(executable),
        "driver_sha256": digest(pathlib.Path(__file__)), "build_inputs": build_inputs, "records": records}

    def invoke(name: str, *arguments, expected=0):
        return run(name, [executable, *arguments], cwd=scratch, expected=expected, timeout=45)

    def read_rows(database: pathlib.Path, sql: str):
        connection = sqlite3.connect(database)
        try:
            return connection.execute(sql).fetchall()
        finally:
            connection.close()

    try:
        suite_result = run("real-database-suite", [executable, "suite", scratch / "suite.sqlite"], cwd=scratch, expected=None, timeout=45)
        database = scratch / "crash.sqlite"; ready = scratch / "ready.txt"
        holder = subprocess.Popen([str(executable), "hold", str(database), str(ready)], cwd=scratch,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
        holder_pid = holder.pid
        deadline = time.monotonic() + 10
        while not ready.exists() and holder.poll() is None and time.monotonic() < deadline:
            time.sleep(.02)
        assert ready.exists(), ("holder-not-ready", holder.poll())
        before = read_rows(database, "SELECT fence FROM sgagent_heads WHERE run_id='unknown-tool'")[0][0]
        started = time.monotonic()
        blocked = subprocess.run([str(executable), "probe", str(database)], cwd=scratch,
            capture_output=True, encoding="utf-8", timeout=10)
        lock_ms = (time.monotonic() - started) * 1000
        assert blocked.returncode != 0 and lock_ms < 2000, ("second owner accepted or blocked", blocked.returncode, lock_ms)
        alias_directory = scratch / "alias"; alias_directory.mkdir()
        alias = str(alias_directory) + "/../crash.sqlite"
        alias_probe = subprocess.run([str(executable), "probe", alias], cwd=scratch,
            capture_output=True, encoding="utf-8", timeout=10)
        assert alias_probe.returncode != 0, "canonical alias acquired a second execution lock"
        invoke("different-run-admission-independent", "other", database)
        after = read_rows(database, "SELECT fence FROM sgagent_heads WHERE run_id='unknown-tool'")[0][0]
        assert before == after == 1
        records.append({"name": "independent-process-lock-and-canonical-alias", "exit_code": blocked.returncode,
            "lock_refusal_ms": lock_ms, "unchanged_fence": before, "owned_holder_pid": holder_pid})
        # Only this driver's own holder is killed. It has launched no tool process;
        # confirmed quiescence in the following synthetic case is therefore true.
        holder.kill(); holder.communicate(timeout=10); holder = None
        invoke("crash-unknown-cleanup-blocks-default-open", "probe", database, expected=12)
        invoke("host-confirmed-recovery-pauses-without-replay", "recover", database)
        row = read_rows(database, "SELECT h.fence,h.revision,h.snapshot,h.digest,r.fence,r.revision,r.digest FROM sgagent_heads h JOIN sgagent_receipts r USING(run_id) WHERE h.run_id='unknown-tool'")[0]
        checkpoint = json.loads(row[2])
        assert checkpoint["phase"] == 10 and checkpoint["reason"] == "tool_outcome_unknown" and checkpoint["cleanup_required"]
        assert row[0] == row[4] == checkpoint["fence"] and row[1] == row[5] == checkpoint["revision"]
        assert row[3] == row[6] == hashlib.sha256(row[2].encode()).hexdigest()
        assert not (scratch / "side-effect-1").exists()
        assert suite_result.returncode == 0, ("native suite failed; independent lock/recovery checks still ran", suite_result.returncode, suite_result.stdout)
        evidence["result"] = "passed"
    except BaseException as error:
        evidence["result"] = "failed"; evidence["error"] = repr(error)
        raise
    finally:
        if holder is not None:
            holder.kill(); holder.communicate(timeout=10)
        # Inventory exact private paths before removing any file; no recursive
        # delete, computed shell command or user directory can be a target.
        inventory = list(scratch.rglob("*"))
        assert all(p.resolve().is_relative_to(scratch.resolve()) for p in inventory)
        for path in inventory:
            if path.is_file(): path.unlink()
        for path in sorted((p for p in inventory if p.is_dir()), key=lambda p: len(p.parts), reverse=True):
            path.rmdir()
        scratch.rmdir()
        evidence["temporary_fixture_removed"] = True
        output.write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({"result": evidence["result"], "executable_sha256": evidence["executable_sha256"],
        "cases": len(records), "report": str(output)}, ensure_ascii=False))


if __name__ == "__main__":
    main()
