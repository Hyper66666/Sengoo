"""Compare exact SG sources through the SG native pipeline and frozen Rust/LLVM."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
CASES = {
    "constant": "def main()->i64{42}",
    "call": "def add(a:i64,b:i64)->i64{a+b} def main()->i64{add(40,2)}",
    "branch": "def choose(x:bool)->i64{if x {42}else{7}} def main()->i64{choose(true)}",
    "while": "def main()->i64{let mut i=0;let mut sum=0;while i<10{sum=sum+i;i=i+1;}sum-3}",
    "for": "def main()->i64{let mut sum=0;for i in 0..10{sum=sum+i;}sum-3}",
    "match": "def main()->i64{let x=7;match x{0=>40,7=>42,_=>1}}",
    "narrow": "def add8(a:i8,b:i8)->i8{a+b} def main()->i64{let x=40i8;let y=2i8;let z=add8(x,y);z as i64}",
    "c-call": 'extern "C"{fn sg_native_external6(a:i64,b:i64,c:i64,d:i64,e:i64,f:i64)->i64;} def main()->i64{sg_native_external6(1,2,3,4,5,6)+21}',
    "unicode": "// 独立原生路径，UTF-8 byte spans\ndef main()->i64{let answer=40;answer+2}",
    "generic-reuse": "def identity<T>(value:T)->T{value} def main()->i64{if identity(true){identity(40)+identity(2)}else{0}}",
    "generic-nested": "def identity<T>(value:T)->T{value} def wrapper<U>(value:U)->U{identity(value)} def main()->i64{wrapper(42)}",
    "generic-recursion": "def descend<T>(value:T,depth:i64)->T{if depth==0{value}else{descend(value,depth-1)}} def main()->i64{descend(42,3)}",
    "generic-field": "struct Holder<T>{value:T} def unwrap<T>(holder:Holder<T>)->T{holder.value} def main()->i64{unwrap(Holder{value:42})}",
    "aggregate": "struct Pair{left:i64,right:i64} def make(a:i64,b:i64)->Pair{Pair{left:a,right:b}} def main()->i64{let pair=make(40,2);pair.left+pair.right}",
    "float-f64": "def add(a:f64,b:f64)->f64{a+b} def main()->i64{if add(40.0,2.0)==42.0{42}else{1}}",
    "float-f32": "def add(a:f32,b:f32)->f32{a+b} def main()->i64{if add(40.0f32,2.0f32)==42.0f32{42}else{1}}",
    "class-read": "class Parent{value:i64;def score(&self)->i64{self.value}} class Child:Parent{extra:i64;} def main()->i64{let child=Child{value:42,extra:0};child.score()}",
    "class-write": "class Parent{value:i64;def update(&mut self){self.value=42;();}} class Child:Parent{extra:i64;} def main()->i64{let mut child=Child{value:0,extra:0};child.update();child.value}",
    "method-names": "struct First{value:i64} struct Second{value:i64} impl First{def score(&self)->i64{self.value}} impl Second{def score(&self)->i64{self.value}} def main()->i64{let a=First{value:20};let b=Second{value:22};a.score()+b.score()}",
}


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("seed", "clang", "formatter", "reference"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--output", type=Path, default=ROOT / "target/selfhost/m4-source-final")
    args = parser.parse_args()
    out = args.output.resolve()
    if not out.is_relative_to(ROOT / "target"):
        raise ValueError("outputs must stay inside target/")
    out.mkdir(parents=True, exist_ok=True)
    paths = {name: getattr(args, name).resolve() for name in ("seed", "clang", "formatter", "reference")}
    lock = json.loads((ROOT / "selfhost/bootstrap.lock").read_text())
    baseline = json.loads((ROOT / "selfhost/reference-baseline.lock").read_text())
    modules = sorted((ROOT / "selfhost/src").glob("*.sg"))
    inputs = [Path(__file__), ROOT / "selfhost/bootstrap.lock", ROOT / "selfhost/reference-baseline.lock",
              ROOT / "selfhost/tools/native.sg", ROOT / "selfhost/tests/native_source_probe.c", *modules]
    inputs += sorted((ROOT / "tools/stdlib").rglob("*.sg"))
    inputs += [ROOT / "tools/stdlib/runtime.c", ROOT / "tools/stdlib/runtime_abi.h"]
    report = {"schema": 1, "result": "running", "stage": "sg-source-to-native-object",
              "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "host": platform.platform(), "sources": {str(p.relative_to(ROOT)): sha(p) for p in inputs if p.is_file()},
              "tools": {}, "steps": [], "cases": [], "m4_complete": False,
              "limits": ["single-source development CLI; not sgc command/module/configuration parity",
                         "SG performs frontend/semantics/HIR/MIR/x64/COFF emission; external clang only links verification artifacts",
                         "frozen qualified Rust reference separately emits LLVM IR for the exact same source bytes",
                         "full ownership/async/numeric-conversion/debug/linker/self-build gates remain open"]}
    env = dict(os.environ)
    for name in list(env):
        if name.upper().startswith(("SENGOO_", "SGPLATFORM_", "SDL2_")):
            env.pop(name)
    env.update(SENGOO_STDLIB=str(ROOT / "tools/stdlib"), SENGOO_RUNTIME=str(ROOT / "tools/stdlib/runtime.c"),
               SENGOO_CLANG=str(paths["clang"]), SENGOO_MODULE_MAP=";".join(
                   "selfhost_" + path.stem + "=" + str(path) for path in modules))
    env["PATH"] = str(paths["clang"].parent) + os.pathsep + env.get("PATH", "")

    def run(name, command, code=0):
        started = time.monotonic()
        done = subprocess.run(list(map(str, command)), cwd=ROOT, env=env, capture_output=True, timeout=600)
        (out / (name + ".stdout.log")).write_bytes(done.stdout)
        (out / (name + ".stderr.log")).write_bytes(done.stderr)
        report["steps"].append({"name": name, "command": list(map(str, command)), "exit_code": done.returncode,
                                "expected_exit_code": code, "seconds": time.monotonic() - started})
        if done.returncode != code:
            raise AssertionError(f"{name} failed: {done.returncode}")
        print(f"{name}: {done.returncode}", flush=True)
        return done

    try:
        for name, path in paths.items():
            digest = sha(path)
            expected = baseline["tools"]["sgc"]["sha256"] if name == "reference" else lock[name]["sha256"]
            if digest != expected:
                raise ValueError(f"{name} identity mismatch")
            report["tools"][name] = {"path": str(path), "sha256": digest}
        run("format-cli", [paths["formatter"], "--check", ROOT / "selfhost/tools/native.sg"])
        binary = out / "native.exe"
        binary.unlink(missing_ok=True)
        run("build-cli", [paths["seed"], "--runtime-mode", "source-development", "build",
                          ROOT / "selfhost/tools/native.sg", "--force-rebuild", "-O", "0", "-o", binary])
        for name, text in CASES.items():
            source = out / (name + " 世界 &.sg")
            source.write_bytes(text.encode("utf-8"))
            obj, repeat, ir = (out / (name + suffix) for suffix in (".obj", "-repeat.obj", ".ll"))
            native_exe, reference_exe = out / (name + "-native.exe"), out / (name + "-reference.exe")
            for path in (obj, repeat, ir, native_exe, reference_exe):
                path.unlink(missing_ok=True)
            row = {"name": name, "source": str(source), "sha256": sha(source)}
            report["cases"].append(row)
            emitted = run(name + "-native-emit", [binary, source, obj])
            if json.loads(emitted.stdout) != {"schema_version": 1, "ok": True, "phase": "object"}:
                raise AssertionError("native CLI success protocol differs")
            run(name + "-native-repeat", [binary, source, repeat])
            if obj.read_bytes() != repeat.read_bytes():
                raise AssertionError("source object emission is nondeterministic")
            run(name + "-native-link", [paths["clang"], ROOT / "selfhost/tests/native_source_probe.c", obj, "-o", native_exe])
            actual = run(name + "-native-execute", [native_exe], 42)
            run(name + "-reference-ir", [paths["reference"], "--runtime-mode", "source-development", "build", source,
                                         "--force-rebuild", "-O", "0", "--emit-llvm", "-o", ir])
            run(name + "-reference-link", [paths["clang"], ROOT / "selfhost/tests/native_source_probe.c", ir, "-o", reference_exe])
            expected = run(name + "-reference-execute", [reference_exe], 42)
            if (actual.stdout, actual.stderr) != (expected.stdout, expected.stderr) or sha(source) != row["sha256"]:
                raise AssertionError("source/result bytes differ")
            row.update(object_sha256=sha(obj), reference_ir_sha256=sha(ir), result_equal=True)
        for name, text, phase in (
            ("bad-syntax", "def main()->i64{", "frontend"),
            ("bad-semantics", "def main()->i64{missing}", "semantics"),
            ("unsupported-float", "def main()->i64{let x:f64=42.0;x as i64}", "native"),
        ):
            source, obj = out / (name + ".sg"), out / (name + ".obj")
            source.write_bytes(text.encode("utf-8"))
            sentinel = b"previous valid output must survive a failed compile"
            obj.write_bytes(sentinel)
            rejected = run(name, [binary, source, obj], 65)
            diagnostic = json.loads(rejected.stdout)
            if diagnostic.get("ok") is not False or diagnostic.get("phase") != phase or obj.read_bytes() != sentinel:
                raise AssertionError("failed compile published an output or incorrect phase")
            report["cases"].append({"name": name, "sha256": sha(source), "diagnostic": diagnostic, "previous_output_preserved": True})
        report["artifacts"] = {binary.name: sha(binary)}
        report["result"] = "passed"
    except BaseException as error:
        report.update(result="failed", error=repr(error))
        raise
    finally:
        changed = [name for name, digest in report["sources"].items() if not (ROOT / name).is_file() or sha(ROOT / name) != digest]
        changed_tools = [name for name, identity in report["tools"].items()
                         if not Path(identity["path"]).is_file() or sha(Path(identity["path"])) != identity["sha256"]]
        report.update(changed_sources=changed, changed_tools=changed_tools)
        if changed or changed_tools:
            report.update(result="failed", error="source pipeline inputs changed during verification")
        (out / "evidence.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        if changed or changed_tools:
            raise AssertionError("source pipeline inputs changed")


if __name__ == "__main__":
    main()
