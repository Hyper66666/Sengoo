"""Verify contextual projection profiles and the borrowed runtime pointer bridge.

This partial correctness gate does not qualify performance or self-compilation.
"""
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import sys
import time

from compiler_inputs import compiler_closure, module_map

ROOT = Path(__file__).resolve().parents[1]
COMMON = ('extern "C"{fn sg_acquire()->i64;fn sg_release(value:i64);'
          'fn sg_live()->i64;fn sg_created()->i64;fn sg_frees()->i64;'
          'fn sengoo_ffi_buffer_copy_out(handle:i64,output:i64,capacity:i64)->i64;} ')
MAIN = ('def main()->i64{let mut round=0;while round<100{'
        'if exercise()!=42{return 1;};'
        'if sg_live()!=0 or sg_created()!=round+1 or sg_frees()!=round+1{return 2;};'
        'round=round+1;};42}')
CASES = {
    'source-projected': COMMON +
        'struct String{handle:i64} impl Drop for String{def drop(&mut self){sg_release(self.handle);}} '
        'def project(value:String)->i64{value.handle} '
        'def observed(value:String)->i64{sengoo_ffi_buffer_copy_out(0,project(value),0)} '
        'def exercise()->i64{let owner=String{handle:sg_acquire()};observed(owner)+observed(owner)} ' + MAIN,
    'native-projected': COMMON +
        'extern "C"{fn sengoo_ffi_buffer_ptr(handle:i64)->i64;fn sengoo_ffi_buffer_len(handle:i64)->i64;} '
        'struct Buffer{handle:i64} impl Drop for Buffer{def drop(&mut self){sg_release(self.handle);}} '
        'def observed(value:Buffer)->i64{sengoo_ffi_buffer_copy_out(0,sengoo_ffi_buffer_ptr(value.handle),sengoo_ffi_buffer_len(value.handle))} '
        'def exercise()->i64{let owner=Buffer{handle:sg_acquire()};observed(owner)+observed(owner)} ' + MAIN,
    'runtime-pointer': COMMON +
        'extern "C"{fn sengoo_string_as_str_ptr(handle:i64)->i64;fn sg_observe_text(value:&str)->i64;} '
        'struct String{handle:i64} impl Drop for String{def drop(&mut self){sg_release(self.handle);}} '
        'def observed(value:&String)->i64{sg_observe_text(value.as_str())} '
        'def exercise()->i64{let owner=String{handle:sg_acquire()};observed(&owner)+observed(&owner)} ' + MAIN,
}


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def revision() -> str:
    return subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()


def source_inputs() -> tuple[dict[str, str], list[Path]]:
    paths = {Path(__file__).resolve(), ROOT/'selfhost/tests/projection_probe.c',
             ROOT/'selfhost/bootstrap.lock', ROOT/'selfhost/reference-baseline.lock',
             ROOT/'Cargo.toml', ROOT/'Cargo.lock', ROOT/'rust-toolchain.toml'}
    closure = set(compiler_closure(ROOT, [ROOT/'selfhost/tools/native.sg']))
    paths.update(closure)
    for directory in ('tools/stdlib', 'runtime'):
        for path in (ROOT/directory).rglob('*'):
            if path.is_file() and not any(part in {'target', 'build', '__pycache__'} for part in path.relative_to(ROOT/directory).parts):
                paths.add(path)
    return {p.relative_to(ROOT).as_posix(): sha(p) for p in sorted(paths)}, sorted(closure-{ROOT/'selfhost/tools/native.sg'})


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--seed', type=Path, default=ROOT/'target/agent-conditional-compiler/bin/sgc.exe')
    parser.add_argument('--reference', type=Path, default=ROOT/'target/selfhost/seed-reference/release/sgc.exe')
    parser.add_argument('--clang', type=Path, default=Path('C:/Users/admin/AppData/Local/SengooToolchain/clang+llvm-19.1.7-x86_64-pc-windows-msvc/bin/clang.exe'))
    parser.add_argument('--output', type=Path, default=ROOT/'target/selfhost/m3-projection-contract/checkpoint')
    args = parser.parse_args()
    output = args.output.resolve()
    if not output.is_relative_to(ROOT/'target'):
        raise ValueError('gate outputs must remain inside target/')
    output.mkdir(parents=True, exist_ok=True)
    report = {'schema': 1, 'stage': 'contextual-projection-and-runtime-pointer', 'result': 'running',
              'm3_complete': False, 'hir_schema': 7, 'runtime_abi': 1, 'optimization': 0,
              'started_at': dt.datetime.now(dt.timezone.utc).isoformat(),
              'host': {'system': platform.system(), 'release': platform.release(), 'machine': platform.machine(),
                       'python': platform.python_version(), 'python_executable_sha256': sha(Path(sys.executable))},
              'steps': [], 'cases': [], 'tools': {},
              'scope': 'three exact standalone SG sources; each executes 100 scopes with two observations, one acquisition and one release per scope',
              'remaining': ['full standard-library ownership parity', 'general borrowed-result lifetime inference', 'generic ownership summaries',
                            'executable contracts, closures, owned patterns, async and final self-build/cutover gates']}
    before: dict[str, str] = {}
    tools: dict[str, tuple[Path, str]] = {}

    def save() -> None:
        (output/'evidence.json').write_text(json.dumps(report, indent=2)+'\n', encoding='utf-8')

    save()
    try:
        report['source_revision'] = revision()
        lock = json.loads((ROOT/'selfhost/bootstrap.lock').read_text(encoding='utf-8'))
        baseline = json.loads((ROOT/'selfhost/reference-baseline.lock').read_text(encoding='utf-8'))
        tools = {'seed': (args.seed.resolve(), lock['seed']['sha256']),
                 'clang': (args.clang.resolve(), lock['clang']['sha256']),
                 'reference': (args.reference.resolve(), baseline['tools']['sgc']['sha256'])}
        for name, (path, expected) in tools.items():
            if sha(path) != expected:
                raise RuntimeError(name+' identity mismatch')
        for name, (path, expected) in tools.items():
            report['tools'][name] = {'path': str(path), 'sha256': expected,
                                    'version': subprocess.check_output([str(path), '--version'], text=True, stderr=subprocess.STDOUT, timeout=30).strip()}
        before, modules = source_inputs()
        report['inputs_before'] = before
        env = os.environ.copy()
        for name in list(env):
            if name.upper().startswith(('SENGOO_', 'SGPLATFORM_', 'SDL2_')):
                env.pop(name)
        env.update(SENGOO_STDLIB=str(ROOT/'tools/stdlib'), SENGOO_RUNTIME=str(ROOT/'tools/stdlib/runtime.c'),
                   SENGOO_CLANG=str(args.clang.resolve()), SENGOO_MODULE_MAP=module_map(ROOT, modules))
        env['PATH'] = str(args.clang.resolve().parent)+os.pathsep+env.get('PATH', '')

        def run(name: str, command: list, expected: int = 0) -> subprocess.CompletedProcess:
            started = time.perf_counter()
            result = subprocess.run(list(map(str, command)), cwd=ROOT, env=env, capture_output=True, timeout=900)
            stdout, stderr = output/(name+'.stdout.log'), output/(name+'.stderr.log')
            stdout.write_bytes(result.stdout)
            stderr.write_bytes(result.stderr)
            report['steps'].append({'name': name, 'command': list(map(str, command)), 'exit_code': result.returncode,
                                    'expected_exit_code': expected, 'elapsed_s': time.perf_counter()-started,
                                    'stdout_sha256': sha(stdout), 'stderr_sha256': sha(stderr)})
            save()
            print(name, result.returncode, flush=True)
            if result.returncode != expected:
                raise RuntimeError(name+' failed')
            return result

        compiler = output/'native.exe'
        compiler.unlink(missing_ok=True)
        run('compiler-build', [args.seed.resolve(), '--runtime-mode', 'source-development', 'build', ROOT/'selfhost/tools/native.sg',
                               '--force-rebuild', '-O', '0', '-o', compiler])
        report['compiler_sha256'] = sha(compiler)
        for name, text in CASES.items():
            source = output/(name+' 世界 &.sg')
            source.write_bytes(text.encode('utf-8'))
            obj, repeated, ir = (output/(name+suffix) for suffix in ('.obj', '-repeated.obj', '.ll'))
            candidate, reference = output/(name+'-candidate.exe'), output/(name+'-reference.exe')
            for path in (obj, repeated, ir, candidate, reference):
                path.unlink(missing_ok=True)
            row = {'name': name, 'source_sha256': sha(source), 'scope_count': 100, 'observations_per_scope': 2,
                   'expected_created_per_scope': 1, 'expected_releases_per_scope': 1}
            report['cases'].append(row)
            for suffix, artifact in (('emit', obj), ('repeat', repeated)):
                result = run(name+'-'+suffix, [compiler, source, artifact])
                if json.loads(result.stdout) != {'schema_version': 1, 'ok': True, 'phase': 'object'} or result.stderr:
                    raise RuntimeError(name+' object protocol mismatch')
            if obj.read_bytes() != repeated.read_bytes():
                raise RuntimeError(name+' object nondeterminism')
            run(name+'-candidate-link', [args.clang.resolve(), ROOT/'selfhost/tests/projection_probe.c', obj, '-o', candidate])
            actual = run(name+'-candidate-execute', [candidate], 42)
            run(name+'-reference-ir', [args.reference.resolve(), '--runtime-mode', 'source-development', 'build', source,
                                      '--force-rebuild', '-O', '0', '--emit-llvm', '-o', ir])
            run(name+'-reference-link', [args.clang.resolve(), ROOT/'selfhost/tests/projection_probe.c', ir, '-o', reference])
            oracle = run(name+'-reference-execute', [reference], 42)
            if actual.stdout or actual.stderr or oracle.stdout or oracle.stderr or sha(source) != row['source_sha256']:
                raise RuntimeError(name+' unexpected output or changed source')
            row.update(result='passed', object_sha256=sha(obj), reference_ir_sha256=sha(ir),
                       candidate_sha256=sha(candidate), reference_sha256=sha(reference))
        report['result'] = 'passed'
    except BaseException as error:
        report['result'] = 'failed'
        report['failure'] = type(error).__name__+': '+str(error)
    finally:
        try:
            after, _ = source_inputs()
            report['inputs_after'] = after
            report['source_changes'] = sorted(name for name in before.keys() | after.keys() if before.get(name) != after.get(name))
            report['tool_changes'] = [name for name, (path, expected) in tools.items() if sha(path) != expected]
            report['source_revision_after'] = revision()
            if report['source_changes'] or report['tool_changes']:
                report['result'] = 'failed'
                report['source_guard_failure'] = 'source/tool inputs changed during gate'
        except BaseException as error:
            report['result'] = 'failed'
            report['finally_failure'] = type(error).__name__+': '+str(error)
        report['finished_at'] = dt.datetime.now(dt.timezone.utc).isoformat()
        save()
    print(json.dumps({name: report.get(name) for name in ('result', 'failure', 'source_changes', 'tool_changes')}), flush=True)
    return 0 if report['result'] == 'passed' else 1


if __name__ == '__main__':
    raise SystemExit(main())
