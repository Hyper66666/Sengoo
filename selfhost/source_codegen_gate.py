"""Compare exact SG sources through the SG native pipeline and frozen Rust/LLVM."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
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
    "float-cast": "def main()->i64{42.9 as i64}",
    "float-nan-cast": "def main()->i64{let x=0.0/0.0;(x as i64)+42}",
    "float-saturate": "def main()->i64{let x=1.0e300;if (x as i64)==9223372036854775807{42}else{1}}",
    "float-narrow-cast": "def main()->i64{let x=42.0 as f32;if (x as f64)==42.0{42}else{1}}",
    "division": "def divide(x:i64,y:i64)->i64{x/y} def main()->i64{divide(85,2)}",
    "remainder": "def rem(x:i64,y:i64)->i64{x%y} def main()->i64{rem(85,43)}",
    "class-read": "class Parent{value:i64;def score(&self)->i64{self.value}} class Child:Parent{extra:i64;} def main()->i64{let child=Child{value:42,extra:0};child.score()}",
    "class-write": "class Parent{value:i64;def update(&mut self){self.value=42;();}} class Child:Parent{extra:i64;} def main()->i64{let mut child=Child{value:0,extra:0};child.update();child.value}",
    "method-names": "struct First{value:i64} struct Second{value:i64} impl First{def score(&self)->i64{self.value}} impl Second{def score(&self)->i64{self.value}} def main()->i64{let a=First{value:20};let b=Second{value:22};a.score()+b.score()}",
    "method-effect-tail": "struct Counter{value:i64} impl Counter{def bump(&mut self){self.value=self.value+1;true;}} def main()->i64{let mut counter=Counter{value:41};counter.bump();counter.value}",
    "array-read": "def answer(index:i64)->i64{let values=[20,22];values[0]+values[index]} def main()->i64{answer(1)}",
    "array-write": "def main()->i64{let mut values=[1,2,3];values[1]=42;values[1]}",
    "array-for": "def main()->i64{let mut sum=0;for value in [20,22,99]{if value==99{break;};sum=sum+value;};sum}",
    "array-tuple-for": "def main()->i64{let mut sum=0;for (a,b) in [(20,22),(1,2)]{sum=sum+a+b;};sum-3}",
    "array-for-simple": "def main()->i64{let mut sum=0;for value in [20,22]{sum=sum+value;};sum}",
    "array-tuple-read": "def main()->i64{let mut sum=0;for pair in [(20,22),(1,2)]{sum=sum+pair.0+pair.1;};sum-3}",
    "string-utf8": r'extern "C"{fn sg_source_string(value:&str,kind:i64)->i64;} def text()->&str{"世界 &\n\t\"\\\r😀"} def main()->i64{sg_source_string(text(),0)}',
    "string-empty": 'extern "C"{fn sg_source_string(value:&str,kind:i64)->i64;} def main()->i64{sg_source_string("",1)}',
    "string-nul": r'extern "C"{fn sg_source_string(value:&str,kind:i64)->i64;} def main()->i64{sg_source_string("A\0B",2)}',
    "impl-if-branches": "def insert()->bool{true} struct State{} impl State{def run(&self)->i64{let mut done=false;while !done{if done{done=true}else{done=true;insert();}}42}} def main()->i64{let s=State{};s.run()}",
    "impl-if-no-else": "def insert()->bool{true} struct State{} impl State{def run(&self)->i64{let mut done=false;while !done{if !done{done=true;insert();}}42}} def main()->i64{let s=State{};s.run()}",
    "operator-add": "trait Add<Rhs,Output>{def add(self,rhs:Rhs)->Output{}} struct Value{n:i64} impl Add<Value,i64> for Value{def add(self,rhs:Value)->i64{self.n+rhs.n}} def main()->i64{let a=Value{n:20};let b=Value{n:22};a+b}",
    "for-break": "def main()->i64{for value in 0..4{break;};42}",
    "for-return": "def main()->i64{for value in 0..4{return 42;};0}",
    "try-result": "struct Result<T,E>{is_ok:bool,value:T,error:E} def answer()->Result<i64,i64>{Result{is_ok:true,value:42,error:0}} def main()->i64{let result=try{let value=answer()?;value;};result.value}",
    "try-option": "struct Option<T>{is_some:bool,value:T} def answer()->Option<i64>{Option{is_some:true,value:42}} def main()->i64{let result=try{let value=answer()?;value;};result.value}",
    "try-error": "struct Result<T,E>{is_ok:bool,value:T,error:E} def answer()->Result<bool,i64>{Result{is_ok:false,value:false,error:42}} def wrapped()->Result<i64,i64>{let value=answer()?;Result{is_ok:true,value:0,error:0}} def main()->i64{let result=wrapped();result.error}",
    "option-none": "struct Option<T>{is_some:bool,value:T} def answer()->Option<i64>{Option{is_some:false,value:0}} def wrapped()->Option<i64>{let value=answer()?;Option{is_some:true,value:42}} def main()->i64{let result=wrapped();if result.is_some{0}else{42}}",
    "drop-scope": 'extern "C"{fn sg_drop_record(value:i64);fn sg_drop_count()->i64;fn sg_drop_at(index:i64)->i64;} struct Owner{value:i64} impl Drop for Owner{def drop(&mut self){sg_drop_record(self.value);}} def scope(){let first=Owner{value:20};let second=Owner{value:22};();} def main()->i64{scope();if sg_drop_count()==2 and sg_drop_at(0)==22 and sg_drop_at(1)==20{42}else{1}}',
    "drop-return": 'extern "C"{fn sg_drop_record(value:i64);fn sg_drop_count()->i64;fn sg_drop_at(index:i64)->i64;} struct Owner{value:i64} impl Drop for Owner{def drop(&mut self){sg_drop_record(self.value);}} def answer()->i64{let owner=Owner{value:42};return 42;} def main()->i64{let answer=answer();if answer==42 and sg_drop_count()==1 and sg_drop_at(0)==42{42}else{1}}',
}

# The frozen reference accepts this source but emits invalid LLVM IR. Keep the
# exact failed link and independently execute the SG result; never call it equal.
REFERENCE_LINK_GAPS = {
    "try-result": (b"defined with type 'ptr' but expected 'i64'", b"insertvalue"),
    "array-for": (b"expected instruction opcode",),
}
REFERENCE_SOURCE_GAPS = {"array-tuple-for": "未定义的变量: a".encode("utf-8")}


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def native_modules():
    pending = [ROOT / "selfhost/tools/native.sg"]
    seen = set()
    while pending:
        source = pending.pop()
        if source in seen:
            continue
        seen.add(source)
        for name in re.findall(r"(?m)^\s*import\s+selfhost_([A-Za-z0-9_]+)\s*;", source.read_text(encoding="utf-8")):
            path = ROOT / "selfhost/src" / (name + ".sg")
            if not path.is_file():
                raise ValueError(f"missing declared native compiler module: {name}")
            pending.append(path)
    return sorted(seen - {ROOT / "selfhost/tools/native.sg"})


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
    modules = native_modules()
    inputs = [Path(__file__), ROOT / "selfhost/bootstrap.lock", ROOT / "selfhost/reference-baseline.lock", ROOT / "selfhost/REFERENCE_GAPS.md",
              ROOT / "selfhost/tools/native.sg", ROOT / "selfhost/tests/native_source_probe.c", *modules]
    inputs += sorted((ROOT / "tools/stdlib").rglob("*.sg"))
    inputs += [ROOT / "tools/stdlib/runtime.c", ROOT / "tools/stdlib/runtime_abi.h"]
    report = {"schema": 1, "result": "running", "stage": "sg-source-to-native-object",
              "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "host": platform.platform(), "sources": {str(p.relative_to(ROOT)): sha(p) for p in inputs if p.is_file()},
              "tools": {}, "steps": [], "cases": [], "m4_complete": False, "reference_link_gaps": [], "reference_source_gaps": [],
              "limits": ["single-source development CLI; not sgc command/module/configuration parity",
                         "SG performs frontend/semantics/HIR/MIR/x64/COFF emission; external clang only links verification artifacts",
                         "frozen qualified Rust reference separately emits LLVM IR for the exact same source bytes",
                         "full ownership, async, numeric, debug, module/tool compatibility and self-build gates remain open"]}
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
            reference = run(name + "-reference-ir", [paths["reference"], "--runtime-mode", "source-development", "build", source,
                                         "--force-rebuild", "-O", "0", "--emit-llvm", "-o", ir],
                            1 if name in REFERENCE_SOURCE_GAPS else 0)
            if name in REFERENCE_SOURCE_GAPS:
                if REFERENCE_SOURCE_GAPS[name] not in reference.stderr or ir.exists():
                    raise AssertionError("frozen reference source failure differs from the reviewed gap")
                if sha(source) != row["sha256"] or actual.stdout or actual.stderr:
                    raise AssertionError("independent SG execution changed inputs or output")
                row.update(object_sha256=sha(obj), result_equal=False, reference_source_gap=True, native_expected_exit_code=42)
                report["reference_source_gaps"].append(name)
                continue
            linked = run(name + "-reference-link", [paths["clang"], ROOT / "selfhost/tests/native_source_probe.c", ir, "-o", reference_exe],
                         1 if name in REFERENCE_LINK_GAPS else 0)
            if name in REFERENCE_LINK_GAPS:
                if not all(marker in linked.stderr for marker in REFERENCE_LINK_GAPS[name]):
                    raise AssertionError("frozen reference failure differs from the reviewed LLVM link gap")
                if sha(source) != row["sha256"] or actual.stdout or actual.stderr:
                    raise AssertionError("independent SG execution changed inputs or output")
                row.update(object_sha256=sha(obj), reference_ir_sha256=sha(ir), result_equal=False,
                           reference_link_gap=True, native_expected_exit_code=42)
                report["reference_link_gaps"].append(name)
                continue
            expected = run(name + "-reference-execute", [reference_exe], 42)
            if (actual.stdout, actual.stderr) != (expected.stdout, expected.stderr) or sha(source) != row["sha256"]:
                raise AssertionError("source/result bytes differ")
            row.update(object_sha256=sha(obj), reference_ir_sha256=sha(ir), result_equal=True)
        for name, text, phase in (
            ("bad-syntax", "def main()->i64{", "frontend"),
            ("bad-semantics", "def main()->i64{missing}", "semantics"),
            ("class-unit-tail", "class Counter{value:i64;def bump(&mut self){self.value=self.value+1;true;}} def main()->i64{42}", "semantics"),
            ("array-constant-bounds", "def main()->i64{let values=[1,2,3];values[3]}", "semantics"),
            ("unsupported-nested-drop", "struct Child{value:i64} impl Drop for Child{def drop(&mut self){}} struct Parent{child:Child} impl Drop for Parent{def drop(&mut self){self.child.drop();}} def main()->i64{let value=Parent{child:Child{value:42}};42}", "native"),
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
        report["reference_parity_complete"] = not (report["reference_link_gaps"] or report["reference_source_gaps"])
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
