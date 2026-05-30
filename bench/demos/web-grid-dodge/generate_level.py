#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path


def exe_name(stem: str) -> str:
    return f"{stem}.exe" if sys.platform.startswith("win") else stem


def run_checked(cmd: list[str], cwd: Path | None = None) -> subprocess.CompletedProcess[str]:
    proc = subprocess.run(
        cmd,
        cwd=str(cwd) if cwd else None,
        text=True,
        capture_output=True,
        encoding="utf-8",
        errors="replace",
    )
    if proc.returncode != 0:
        raise RuntimeError(
            f"command failed ({proc.returncode}): {' '.join(cmd)}\nstdout:\n{proc.stdout}\nstderr:\n{proc.stderr}"
        )
    return proc


def resolve_sengoo_root(demo_dir: Path) -> Path:
    bench_root = demo_dir.parents[1]
    env_root = os.environ.get("SENGOO_ROOT")

    candidates = []
    if env_root:
        candidates.append(Path(env_root).expanduser().resolve())
    candidates.append(bench_root.parent.resolve())
    candidates.append((bench_root.parent / "Sengoo").resolve())

    for candidate in candidates:
        if (candidate / "Cargo.toml").exists() and (candidate / "tools" / "sgc" / "src" / "main.rs").exists():
            return candidate

    raise RuntimeError(
        "cannot resolve Sengoo root; set SENGOO_ROOT or place bench next to Sengoo directory"
    )


def main() -> int:
    demo_dir = Path(__file__).resolve().parent
    repo_root = resolve_sengoo_root(demo_dir)

    sgc_candidates = [
        repo_root / "target" / "release" / exe_name("sgc"),
        repo_root / "target" / "debug" / exe_name("sgc"),
    ]
    sgc = next((p for p in sgc_candidates if p.exists()), None)
    if sgc is None:
        raise RuntimeError("sgc binary not found; run `cargo build -p sgc --release` first")

    build_dir = demo_dir / "build"
    build_dir.mkdir(parents=True, exist_ok=True)
    level_bin = build_dir / exe_name("level_generator")
    level_source = demo_dir / "level_generator.sg"

    compile_cmd = [
        str(sgc),
        "build",
        str(level_source),
        "-O",
        "2",
        "--reflect",
        "off",
        "--force-rebuild",
        "-o",
        str(level_bin),
    ]
    run_checked(compile_cmd, cwd=repo_root)

    proc = run_checked([str(level_bin)], cwd=repo_root)
    numbers = []
    for line in proc.stdout.splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            numbers.append(int(line))
        except ValueError:
            continue

    if len(numbers) < 4:
        raise RuntimeError(f"unexpected generator output:\n{proc.stdout}")

    width = numbers[0]
    rows = numbers[1]
    start_col = numbers[2]
    cols = numbers[3:]
    if len(cols) != rows:
        raise RuntimeError(f"row count mismatch: header rows={rows}, actual={len(cols)}")

    payload = {
        "title": "Meteor Dodge",
        "generated_by": "Sengoo level_generator.sg",
        "width": width,
        "visible_rows": 14,
        "start_col": start_col,
        "obstacle_cols": cols,
    }

    js_path = demo_dir / "level_data.js"
    json_path = demo_dir / "level_data.json"

    js_path.write_text(
        "window.SENGOO_LEVEL = " + json.dumps(payload, ensure_ascii=False, separators=(",", ":")) + ";\n",
        encoding="utf-8",
    )
    json_path.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    print(f"[ok] generated {rows} rows")
    print(f"[ok] {js_path}")
    print(f"[ok] {json_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
