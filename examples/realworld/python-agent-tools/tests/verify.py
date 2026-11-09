"""Build a locked external wheel, then test an offline isolated consumer."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
import pathlib
import platform
import shutil
import subprocess
import sys
import tempfile


def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()


def source_identity(repo):
    roots=(repo/"runtime",repo/"tools/stdlib",repo/"packages/sgschema",repo/"tools/sengoo-python/src",repo/"examples/realworld/python-agent-tools")
    return {str(path.relative_to(repo)):sha(path) for root in roots for path in root.rglob("*")
        if path.is_file() and path.suffix in (".rs",".sg",".c",".h",".toml",".py",".lock")
        and not any(part in ("target","build","tests","__pycache__") for part in path.relative_to(root).parts)}


def clean_environment(python):
    environment=dict(os.environ)
    for name in list(environment):
        if name.startswith(("SENGOO_","SGPM_","DYLD_","LLVM_")) or name in ("PYTHONPATH","PYTHONHOME","CARGO_HOME","RUSTUP_HOME","LD_LIBRARY_PATH","LIB","LIBPATH","INCLUDE","CC","CXX","CL","CFLAGS","LDFLAGS"):
            environment.pop(name)
    environment["PATH"]=str(python.parent)+os.pathsep+str(pathlib.Path(os.environ["SystemRoot"])/"System32")
    environment.update(PIP_NO_INDEX="1",PIP_DISABLE_PIP_VERSION_CHECK="1",PYTHONUTF8="1")
    return environment


def cleanup(directory):
    inventory=list(directory.rglob("*"));assert all(p.resolve().is_relative_to(directory.resolve()) for p in inventory)
    for path in inventory:
        if path.is_file():path.unlink()
    for path in sorted((p for p in inventory if p.is_dir()),key=lambda p:len(p.parts),reverse=True):path.rmdir()
    directory.rmdir()


def main():
    parser=argparse.ArgumentParser()
    for name in ("compiler","clang","formatter","sgpm","output"):parser.add_argument("--"+name,type=pathlib.Path,required=True)
    parser.add_argument("--wheel",type=pathlib.Path)
    args=parser.parse_args();repo=pathlib.Path(__file__).resolve().parents[4];source=repo/"examples/realworld/python-agent-tools"
    output=args.output.resolve();output.parent.mkdir(parents=True,exist_ok=True)
    scratch=pathlib.Path(tempfile.mkdtemp(prefix="python-agent-external-中文😀-"));workspace=scratch/"workspace";project=workspace/"examples/realworld/python-agent-tools";runtime=scratch/"runtime";runtime.mkdir()
    records=[];evidence={"schema":1,"result":"running","scope":"external locked PEP517 wheel build and isolated offline venv; synthetic schema tools only","records":records}
    env=dict(os.environ);env.update(SENGOO_CLANG=str(args.clang.resolve()),SENGOO_STDLIB=str(repo/"tools/stdlib"),SENGOO_RUNTIME=str(repo/"tools/stdlib/runtime.c"),SGPM_SGC=str(args.compiler.resolve()),SENGOO_SGPM=str(args.sgpm.resolve()),SGPM_SGFMT=str(args.formatter.resolve()),SENGOO_PYTHON_RUNTIME_MODE="source-development",PYTHONPATH=str(repo/"tools/sengoo-python/src"))
    env["PATH"]=str(args.clang.resolve().parent)+os.pathsep+env.get("PATH","");env.pop("SENGOO_MODULE_MAP",None)
    sources=[source/p for p in ("Sengoo.toml","Sengoo.lock","pyproject.toml","src/lib.sg","tests/consumer.py")]
    frozen_sources=source_identity(repo)
    frozen_tools={name:{"path":str(getattr(args,name).resolve()),"sha256":sha(getattr(args,name).resolve())}
        for name in ("compiler","clang","formatter","sgpm")}
    evidence.update(host=platform.node(),platform=platform.platform(),python=sys.version,
        source_identity=frozen_sources,tools=frozen_tools)
    def run(name,command,cwd,environment):
        result=subprocess.run(list(map(str,command)),cwd=cwd,env=environment,capture_output=True,encoding="utf-8",errors="replace",timeout=360)
        (output.parent/(name+".log")).write_text(result.stdout+result.stderr,encoding="utf-8")
        records.append({"name":name,"exit_code":result.returncode,"stdout":result.stdout,"stderr":result.stderr})
        assert result.returncode==0,(name,result.returncode,result.stdout,result.stderr)
        return result
    try:
        if args.wheel:
            wheel=runtime/args.wheel.name;shutil.copyfile(args.wheel.resolve(),wheel)
        else:
            for path in sources[:4]:
                target=project/path.relative_to(source);target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(path,target)
            shutil.copytree(repo/"packages/sgschema",workspace/"packages/sgschema",ignore=shutil.ignore_patterns("build","target","__pycache__",".git"))
            run("format",[args.formatter.resolve(),"--check",project/"src/lib.sg"],project,env)
            before=sha(project/"Sengoo.lock")
            run("pep517-wheel",[sys.executable,"-c","import sys; from sengoo_python import backend; print(backend.build_wheel(sys.argv[1]))",runtime],project,env)
            assert before==sha(project/"Sengoo.lock"),"wheel build modified locked graph"
            wheels=list(runtime.glob("*.whl"));assert len(wheels)==1
            wheel=wheels[0];shutil.copyfile(wheel,output.parent/wheel.name)
        shutil.copyfile(source/"tests/consumer.py",runtime/"consumer.py")
        # Remove all copied Sengoo project/dependencies before installing anything.
        if workspace.exists():cleanup(workspace)
        initial=clean_environment(pathlib.Path(sys.executable))
        run("fresh-venv",[sys.executable,"-m","venv",runtime/"venv"],runtime,initial)
        python=runtime/"venv"/("Scripts/python.exe" if os.name=="nt" else "bin/python")
        clean=clean_environment(python)
        run("offline-wheel-install",[python,"-X","utf8","-I","-m","pip","install","--no-index","--no-deps","--only-binary=:all:",wheel],runtime,clean)
        result=run("installed-wheel-consumer",[python,"-X","utf8","-I",runtime/"consumer.py"],runtime,clean)
        checks=json.loads(result.stdout)
        final_sources=source_identity(repo)
        evidence["source_changes_during_verification"]=[name for name in sorted(set(frozen_sources)|set(final_sources))
            if frozen_sources.get(name)!=final_sources.get(name)]
        assert not evidence["source_changes_during_verification"],"runtime, SDK, backend or input sources changed during verification"
        assert all(sha(pathlib.Path(tool["path"]))==tool["sha256"] for tool in frozen_tools.values()),"selected tool changed during verification"
        evidence.update(result="passed",mode="prebuilt-offline-reinstall" if args.wheel else "external-locked-build-and-install",lock_unchanged=None if args.wheel else True,copied_sources_removed_before_install=not args.wheel,wheel_name=wheel.name,wheel_sha256=sha(wheel),compiler_sha256=sha(args.compiler.resolve()),formatter_sha256=sha(args.formatter.resolve()),sgpm_sha256=sha(args.sgpm.resolve()),backend_sha256=sha(repo/"tools/sengoo-python/src/sengoo_python/backend.py"),consumer=checks,sources=[{"path":str(p.relative_to(repo)),"sha256":sha(p)} for p in sources])
    except BaseException as error:evidence.update(result="failed",error=repr(error));raise
    finally:
        cleanup(scratch);evidence["temporary_workspace_removed"]=True
        output.write_text(json.dumps(evidence,ensure_ascii=False,indent=2),encoding="utf-8")
    print(json.dumps({"result":evidence["result"],"wheel":evidence["wheel_name"],"wheel_sha256":evidence["wheel_sha256"],"report":str(output)},ensure_ascii=False))


if __name__=="__main__":main()
