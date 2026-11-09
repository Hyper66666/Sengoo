"""Build the complete consumer outside the checkout, then run without sources."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile


def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()


def cleanup(root):
    inventory=list(root.rglob("*"));assert all(p.resolve().is_relative_to(root.resolve()) for p in inventory)
    for p in inventory:
        if p.is_file():p.unlink()
    for p in sorted((p for p in inventory if p.is_dir()),key=lambda p:len(p.parts),reverse=True):p.rmdir()
    root.rmdir()


def main():
    parser=argparse.ArgumentParser()
    for name in ("compiler","clang","formatter","sgpm","output"):parser.add_argument("--"+name,type=pathlib.Path,required=True)
    args=parser.parse_args();repo=pathlib.Path(__file__).resolve().parents[4]
    output=args.output.resolve();output.parent.mkdir(parents=True,exist_ok=True)
    source=repo/"examples/realworld/agent-knowledge"; scratch=pathlib.Path(tempfile.mkdtemp(prefix="knowledge-external-中文😀-"));workspace=scratch/"workspace";consumer=workspace/"examples/realworld/agent-knowledge";runtime=scratch/"runtime";runtime.mkdir()
    records=[];evidence={"schema":1,"result":"running","scope":"repository-external locked source consumer then source-removed no-tool-PATH synthetic MCP protocol","records":records}
    env=dict(os.environ);env.update(SENGOO_CLANG=str(args.clang.resolve()),SENGOO_STDLIB=str(repo/"tools/stdlib"),SENGOO_RUNTIME=str(repo/"tools/stdlib/runtime.c"),SGPM_SGC=str(args.compiler.resolve()),SGPM_SGFMT=str(args.formatter.resolve()))
    env["PATH"]=str(args.clang.resolve().parent)+os.pathsep+env.get("PATH","");env.pop("SENGOO_MODULE_MAP",None)
    def run(name,command,cwd,environment):
        result=subprocess.run(list(map(str,command)),cwd=cwd,env=environment,capture_output=True,encoding="utf-8",errors="replace",timeout=240)
        (output.parent/(name+".log")).write_text(result.stdout+result.stderr,encoding="utf-8")
        records.append({"name":name,"exit_code":result.returncode,"stdout":result.stdout,"stderr":result.stderr})
        assert result.returncode==0,(name,result.returncode,result.stdout,result.stderr)
    try:
        for name in ("Sengoo.toml","Sengoo.lock","src/main.sg"):
            destination=consumer/name;destination.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(source/name,destination)
        for package in ("sgmcp","sgschema","sgsqlite"):
            shutil.copytree(repo/"packages"/package,workspace/"packages"/package,ignore=shutil.ignore_patterns("build","target",".git","__pycache__"))
        before=sha(consumer/"Sengoo.lock")
        run("locked-external-build",[args.sgpm.resolve(),"--runtime-mode","source-development","build","--locked","--manifest-path",consumer/"Sengoo.toml"],consumer,env)
        assert sha(consumer/"Sengoo.lock")==before,"locked build changed dependency graph"
        exe=runtime/"agent-knowledge.exe";shutil.copyfile(consumer/"target/debug/agent-knowledge.exe",exe)
        # Delete the complete copied source and dependency tree before execution.
        cleanup(workspace)
        clean={k:v for k,v in os.environ.items() if not k.startswith(("SENGOO_","SGPM_"))};clean["PATH"]=str(pathlib.Path(os.environ["SystemRoot"])/"System32")
        run("source-removed-protocol",[sys.executable,"-X","utf8",source/"tests/verify.py",exe,"--report",output.parent/"protocol.json"],runtime,clean)
        evidence.update(result="passed",lock_unchanged=True,copied_sources_removed_before_runtime=True,compiler_sha256=sha(args.compiler.resolve()),formatter_sha256=sha(args.formatter.resolve()),sgpm_sha256=sha(args.sgpm.resolve()),executable_sha256=sha(exe),sources=[{"path":str(p.relative_to(repo)),"sha256":sha(p)} for p in (source/"Sengoo.toml",source/"Sengoo.lock",source/"src/main.sg",source/"tests/verify.py")])
        shutil.copyfile(exe,output.parent/"agent-knowledge.exe")
    except BaseException as error:evidence.update(result="failed",error=repr(error));raise
    finally:
        cleanup(scratch);evidence["temporary_workspace_removed"]=True
        output.write_text(json.dumps(evidence,ensure_ascii=False,indent=2),encoding="utf-8")
    print(json.dumps({"result":evidence["result"],"report":str(output),"executable_sha256":evidence["executable_sha256"]},ensure_ascii=False))


if __name__=="__main__":main()
