#!/usr/bin/env python3
"""Verify the real CSV index through an outside-checkout registry consumer.

Requires Python 3.11+, a built toolchain, and a checksum-verified official
registry distribution. No network service or fake Sengoo compiler is used.
Runtime assertions use separate delivered processes after deleting the consumer
sources and removing development tools from PATH. A failure writes JSON and
returns nonzero. This evidence does not replace the independent user pilot.
"""
from __future__ import annotations

import argparse
from contextlib import closing
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import sqlite3
import subprocess
import tempfile

from package_ecosystem import sha256, unpack, verify


def acceptance(repo: Path, bin_dir: Path, distribution: Path, mode: str,
               records: list[dict]) -> dict:
    repo, bin_dir, distribution = repo.resolve(), bin_dir.resolve(), distribution.resolve()
    suffix = ".exe" if os.name == "nt" else ""
    tools = {name: bin_dir / (name + suffix) for name in ("sgc", "sgpm", "sgfmt")}
    for path in tools.values():
        if not path.is_file():
            raise ValueError(f"required tool is missing: {path}")
    verify(distribution)
    environment = os.environ.copy()
    for name in list(environment):
        if name.startswith(("SENGOO_", "SGPM_")):
            environment.pop(name)
    environment.update(SGPM_SGC=str(tools["sgc"]), SGPM_SGFMT=str(tools["sgfmt"]))
    environment["PATH"] = str(bin_dir) + os.pathsep + environment.get("PATH", "")

    def command(argv: list[str], cwd: Path, env: dict[str, str], expected: int = 0):
        result = subprocess.run(argv, cwd=cwd, env=env, capture_output=True,
            text=True, encoding="utf-8", errors="replace", timeout=600)
        records.append({"argv": argv, "exit_code": result.returncode,
            "stdout": result.stdout, "stderr": result.stderr})
        if result.returncode != expected:
            raise AssertionError(f"expected exit {expected}, got {result.returncode}: "
                f"{argv!r}\n{result.stdout}\n{result.stderr}")
        return result

    def check(name: str, **details):
        records.append({"check": name, "ok": True, **details})

    with tempfile.TemporaryDirectory(prefix="sengoo-index-incremental-") as temporary:
        work = Path(temporary).resolve()
        if work.is_relative_to(repo):
            raise ValueError("the consumer must be built outside the toolchain checkout")
        project = work / "consumer"
        shutil.copytree(repo / "examples/realworld/file-csv-index", project,
            ignore=shutil.ignore_patterns("registry", "target", "build", "Sengoo.lock"))
        bundle = work / "official"
        unpack(distribution / "sengoo-official-registry-0.1.0.tar.gz", bundle)
        shutil.copytree(bundle / "registry", project / "registry")

        def sgpm(*arguments: str):
            return command([str(tools["sgpm"]), "--runtime-mode", mode, *arguments],
                project, environment)

        sgpm("update")
        locked = (project / "Sengoo.lock").read_bytes()
        for arguments in (("check", "--locked"), ("fmt", "--check", "--locked"),
                          ("test", "--locked"), ("doc", "--locked"), ("build", "--locked")):
            sgpm(*arguments)
            if (project / "Sengoo.lock").read_bytes() != locked:
                raise AssertionError(f"{arguments!r} changed the lockfile")
        binary = project / "target/debug" / ("file_csv_index" + suffix)
        if not binary.is_file():
            raise AssertionError(f"expected native application artifact is missing: {binary}")
        delivered = work / "delivered"
        delivered.mkdir()
        executable = delivered / binary.name
        shutil.copy2(binary, executable)
        # Only a verified temporary descendant can be deleted here.
        if project.parent != work or project.name != "consumer":
            raise ValueError("unexpected disposable consumer source path")
        shutil.rmtree(project)
        shutil.rmtree(bundle)
        runtime_environment = environment.copy()
        runtime_environment["PATH"] = str(delivered)
        for name in list(runtime_environment):
            if name.startswith(("SENGOO_", "SGPM_", "CARGO_", "RUST", "PYTHON")):
                runtime_environment.pop(name)

        database = delivered / "索引.db"
        source = delivered / "資料.csv"

        def app(*arguments: str, expected: int = 0, db: str = database.name):
            return command([str(executable), "--db", db, *arguments], delivered,
                runtime_environment, expected)

        def index(*arguments: str, expected: int = 0):
            return app("index", "--csv", source.name, *arguments, expected=expected)

        def query(name: str, count: int, contains: str | None = None):
            result = app("query", "--name", name)
            if not result.stdout.rstrip().endswith(f"Matching rows:\n{count}"):
                raise AssertionError(f"wrong query count: {result.stdout!r}")
            if contains is not None and contains not in result.stdout:
                raise AssertionError(f"missing expected query value: {contains!r}")
            return result

        def rows():
            with closing(sqlite3.connect(database)) as db, db:
                entries = db.execute("SELECT source,row_number,name,note FROM entries ORDER BY source,row_number").fetchall()
                sources = db.execute("SELECT source,content,row_count FROM index_sources ORDER BY source").fetchall()
                return entries, sources

        original = "name,note\r\nAlice,first\r\n世界,\"line one\nline \"\"two\"\"\"\r\nO'Brien; DROP TABLE entries;--,safe\r\n"
        source.write_bytes(original.encode("utf-8"))
        first = index()
        if "Indexed rows:\n3" not in first.stdout:
            raise AssertionError("initial import did not report three rows")
        query("Alice", 1, "first")
        query("世界", 1, 'line one\nline "two"')
        query("O'Brien; DROP TABLE entries;--", 1, "safe")
        query("' OR 1=1 --", 0)
        check("separate-process-persistence-Unicode-and-SQL-parameter-binding")

        with closing(sqlite3.connect(database)) as db, db:
            db.executescript("""
                CREATE TABLE audit(event TEXT NOT NULL);
                CREATE TRIGGER audit_insert AFTER INSERT ON entries BEGIN INSERT INTO audit VALUES('insert'); END;
                CREATE TRIGGER audit_delete AFTER DELETE ON entries BEGIN INSERT INTO audit VALUES('delete'); END;
                CREATE TRIGGER audit_source_insert AFTER INSERT ON index_sources BEGIN INSERT INTO audit VALUES('source'); END;
                CREATE TRIGGER audit_source_update AFTER UPDATE ON index_sources BEGIN INSERT INTO audit VALUES('source'); END;
            """)
        before = database.read_bytes()
        unchanged = index()
        if "Unchanged rows:\n3" not in unchanged.stdout:
            raise AssertionError("identical input was not explicitly reported unchanged")
        if database.read_bytes() != before:
            raise AssertionError("an unchanged import wrote database bytes")
        with closing(sqlite3.connect(database)) as db, db:
            if db.execute("SELECT count(*) FROM audit").fetchone()[0] != 0:
                raise AssertionError("an unchanged import mutated rows or source metadata")
        check("unchanged-content-zero-writes", database_sha256=hashlib.sha256(before).hexdigest())

        updated = "name,note\nAlice,updated\n新しい名前,\"comma, and quotes \"\"kept\"\"\"\n"
        source.write_bytes(updated.encode("utf-8"))
        changed = index()
        if "Indexed rows:\n2" not in changed.stdout:
            raise AssertionError("changed input did not replace the old rows")
        query("Alice", 1, "updated")
        query("世界", 0)
        query("新しい名前", 1, 'comma, and quotes "kept"')
        entries, sources = rows()
        if len(entries) != 2 or sources != [(source.name, updated, 2)]:
            raise AssertionError("changed input or its exact metadata was not committed together")
        with closing(sqlite3.connect(database)) as db, db:
            if db.execute("SELECT count(*) FROM audit WHERE event='delete'").fetchone()[0] != 3:
                raise AssertionError("the old rows were not replaced exactly once")
        check("changed-content-atomic-replacement")

        other = delivered / "other.csv"
        other.write_bytes(b"name,note\nOther,retained\n")
        app("index", "--csv", other.name)
        source.write_bytes(b"name,note\n")
        empty = index()
        if "Indexed rows:\n0" not in empty.stdout:
            raise AssertionError("header-only input should replace its source with zero data rows")
        query("Alice", 0)
        query("Other", 1, "retained")
        source.write_bytes(updated.encode("utf-8"))
        index()
        query("Alice", 1, "updated")
        query("Other", 1, "retained")
        check("header-only-clears-selected-source-and-other-sources-remain-queryable")

        committed = rows()
        corruptions = [
            (b'name,note\nAlice,"unterminated', "record 2, field 2"),
            (b"name,note\nAlice,one,extra\n", "record 2: expected 2 columns, found 3"),
            (b"name,wrong\nAlice,note\n", "CSV header must be name,note"),
            (b"", "CSV must contain exactly"),
            (b"name,note\nAlice,\xff\n", "not valid UTF-8"),
            (b"name,note\nAlice,\x00\n", "input contains NUL"),
        ]
        for damaged, message in corruptions:
            source.write_bytes(damaged)
            failure = index(expected=1)
            if message not in failure.stderr or source.name not in failure.stderr:
                raise AssertionError(f"missing actionable CSV diagnosis: {failure.stderr!r}")
            if rows() != committed:
                raise AssertionError("invalid CSV modified the last successful import")
            query("Alice", 1, "updated")
        source.unlink()
        missing = index(expected=1)
        if "cannot open UTF-8 input file" not in missing.stderr or rows() != committed:
            raise AssertionError("a missing file lost old rows or lacked a diagnosis")
        check("damaged-invalid-header-UTF8-NUL-and-missing-input-preserve-old-data")

        # Inject a real SQLite failure after one new row has been inserted. Both
        # the DELETE and the partial INSERT must be rolled back, with the old
        # source snapshot intact. This tests write failure, beyond parser rejection.
        with closing(sqlite3.connect(database)) as db, db:
            db.executescript("""
                CREATE TRIGGER reject_partial BEFORE INSERT ON entries
                WHEN NEW.name='reject-marker'
                BEGIN SELECT RAISE(ABORT, 'acceptance injected write failure'); END;
            """)
        source.write_bytes(b"name,note\npartial,new\nreject-marker,fails\n")
        rejected = index(expected=1)
        if "acceptance injected write failure" not in rejected.stderr or rows() != committed:
            raise AssertionError("a mid-import SQLite error did not preserve old rows and its error message")
        query("partial", 0)
        query("Alice", 1, "updated")
        with closing(sqlite3.connect(database)) as db, db:
            db.execute("DROP TRIGGER reject_partial")
        check("mid-write-error-rolls-back-delete-partial-insert-and-source-metadata")

        source.write_bytes(updated.encode("utf-8"))
        recovered = index()
        if "Unchanged rows:\n2" not in recovered.stdout:
            raise AssertionError("restoring last good input should be recognized unchanged")
        check("restore-good-input-after-failed-update")

        exact = b"name,note\nLimit," + b"x" * (64 - len(b"name,note\nLimit,") - 1) + b"\n"
        if len(exact) != 64:
            raise AssertionError("test fixture length is wrong")
        source.write_bytes(exact)
        index("--max-bytes", "64")
        limited = rows()
        source.write_bytes(exact + b"\n")
        overflow = index("--max-bytes=64", expected=1)
        if "file exceeds maximum input bytes" not in overflow.stderr or rows() != limited:
            raise AssertionError("byte limit did not preserve old rows")
        for invalid in ("0", "-1", "16777217"):
            rejected_limit = index("--max-bytes", invalid, expected=2)
            if "--max-bytes must be from 1 to 16777216" not in rejected_limit.stderr:
                raise AssertionError("invalid limit did not produce a usage diagnosis")
        source.write_bytes(b"name,note\nLimit," + b"x" * (16 * 1024 * 1024) + b"\n")
        index(expected=1)
        if rows() != limited:
            raise AssertionError("the default 16-MiB limit lost previously committed rows")
        check("explicit-exact-byte-boundary-invalid-limits-and-default-16MiB-cap")

        # A multi-MiB valid record exercises bounded (non-streaming) ingestion.
        large = "name,note\n世界," + "x" * (2 * 1024 * 1024) + "\n"
        source.write_bytes(large.encode("utf-8"))
        index()
        with closing(sqlite3.connect(database)) as db, db:
            if db.execute("SELECT name,length(note) FROM entries WHERE source=?", (source.name,)).fetchall() != [("世界", 2 * 1024 * 1024)]:
                raise AssertionError("the bounded large record was truncated or corrupted")
        query("Other", 1, "retained")
        check("valid-multi-MiB-input", input_bytes=len(large.encode("utf-8")))

        # Existing 0.1.0 application databases have only entries. Opening them
        # adds metadata without deleting rows; the first reimport seeds metadata.
        legacy = delivered / "legacy.db"
        with closing(sqlite3.connect(legacy)) as db, db:
            db.execute("CREATE TABLE entries(source TEXT NOT NULL,row_number INTEGER NOT NULL,name TEXT NOT NULL,note TEXT NOT NULL,PRIMARY KEY(source,row_number))")
            db.execute("INSERT INTO entries VALUES(?,1,'Old','kept')", (source.name,))
        old_query = app("query", "--name", "Old", db=legacy.name)
        if "kept" not in old_query.stdout:
            raise AssertionError("old-schema rows were lost on opening the database")
        source.write_bytes(b"name,note\nNew,replaced\n")
        app("index", "--csv", source.name, db=legacy.name)
        with closing(sqlite3.connect(legacy)) as db, db:
            if db.execute("SELECT name,note FROM entries").fetchall() != [("New", "replaced")]:
                raise AssertionError("the first old-schema reimport failed to replace rows")
            if db.execute("SELECT row_count FROM index_sources").fetchall() != [(1,)]:
                raise AssertionError("the first old-schema reimport did not seed metadata")
        check("old-schema-additive-migration-and-first-reimport")

        for option in ("--help", "--version"):
            command([str(executable), option], delivered, runtime_environment)
        command([str(executable), "--unknown"], delivered, runtime_environment, 2)
        app("index", "--csv", source.name, expected=1, db="missing/parent/index.db")
        check("CLI-exit-contract-and-unopenable-database")
        check("delivered-executable-with-consumer-sources-deleted-and-development-PATH-removed",
            executable_sha256=sha256(executable))

    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip()
    return {"schema_version": 1, "ok": True, "revision": revision,
        "host": platform.platform(), "runtime_mode": mode, "outside_checkout": True,
        "tools": {name: {"path": str(path), "sha256": sha256(path)} for name, path in tools.items()},
        "distribution_manifest_sha256": sha256(distribution / "ecosystem-manifest.json"),
        "application_source_sha256": sha256(repo / "examples/realworld/file-csv-index/src/main.sg"),
        "commands": records}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--bin-dir", type=Path, required=True)
    parser.add_argument("--distribution", type=Path, required=True)
    parser.add_argument("--runtime-mode", choices=("installed", "source-development"), default="installed")
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    args.report.parent.mkdir(parents=True, exist_ok=True)
    records: list[dict] = []
    try:
        result = acceptance(args.repo, args.bin_dir, args.distribution, args.runtime_mode, records)
    except Exception as error:
        args.report.write_text(json.dumps({"schema_version": 1, "ok": False,
            "error": str(error), "runtime_mode": args.runtime_mode, "commands": records},
            ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        raise
    args.report.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"CSV index incremental acceptance passed: {args.report}")


if __name__ == "__main__":
    main()
