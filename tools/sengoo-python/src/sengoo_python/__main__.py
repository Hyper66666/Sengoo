import argparse
import json
import shutil
import sys
from . import __version__
from .backend import _configuration, _tool, _ensure_toolchain


def main():
    parser = argparse.ArgumentParser(description="Check a Sengoo Python source-build project")
    parser.add_argument("--version", action="version", version=__version__)
    parser.add_argument("command", choices=["check"])
    parser.add_argument("--format", choices=["text", "json"], default="text")
    args = parser.parse_args()
    report = {"schema_version": 1, "backend_version": __version__, "checks": []}
    def check(name, action):
        try:
            detail = action()
            report["checks"].append({"name": name, "ok": True, "detail": str(detail)})
        except Exception as error:
            report["checks"].append({"name": name, "ok": False, "detail": str(error)})
    def python():
        if not (3, 11) <= sys.version_info[:2] <= (3, 14):
            raise RuntimeError("use CPython 3.11–3.14")
        return sys.version.split()[0]
    check("python", python)
    check("project", lambda: _configuration()[3])
    check("sgpm", lambda: _tool("sgpm", "SENGOO_SGPM"))
    check("sgc", lambda: _tool("sgc", "SGPM_SGC"))
    check("toolchain-match", lambda: _ensure_toolchain(_tool("sgpm", "SENGOO_SGPM")) or "matching compiler and package manager >=0.2.1-dev.0")
    check("clang", lambda: shutil.which("clang") or (_ for _ in ()).throw(RuntimeError("install clang and run sgc doctor")))
    report["ok"] = all(item["ok"] for item in report["checks"])
    if args.format == "json":
        print(json.dumps(report, ensure_ascii=False))
    else:
        for item in report["checks"]:
            print(f"{'ok' if item['ok'] else 'FAIL'} {item['name']}: {item['detail']}")
    return 0 if report["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
