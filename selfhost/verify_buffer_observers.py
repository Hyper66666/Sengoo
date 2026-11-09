"""Verify audited native input reads and independent frozen-reference resource gaps."""
from __future__ import annotations
import argparse, datetime as dt, hashlib, json, os, platform, re, shutil, subprocess, sys, time
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
CASES = {'owned-input': 'struct Buffer{handle:i64} extern "C"{fn sg_buffer_acquire()->i64;fn sg_buffer_release(value:i64);fn sg_buffer_live()->i64;fn sg_buffer_frees()->i64;fn sg_check_owned(value:i64,raw:i64,first:i64,second:i64)->i64;fn sg_check_copy(value:i64,kind:i64,first:i64,second:i64)->i64;fn sengoo_ffi_buffer_ptr(value:i64)->i64;fn sengoo_ffi_buffer_len(value:i64)->i64;fn sengoo_ffi_buffer_get_u8(value:i64,index:i64)->i64;} impl Drop for Buffer{def drop(&mut self){sg_buffer_release(self.handle);}} def owned(buffer:Buffer)->i64{let raw=sengoo_ffi_buffer_ptr(buffer.handle);let first=sengoo_ffi_buffer_get_u8(buffer.handle,0);let second=sengoo_ffi_buffer_get_u8(buffer.handle,1);check_owned(&buffer,raw,first,second)} def exercise()->i64{owned(Buffer{handle:sg_buffer_acquire()})} def main()->i64{let mut round=0;while round<100{if exercise()!=42{return 1;};if sg_buffer_live()!=0 or sg_buffer_frees()!=round+1{return 2;};round=round+1;};42} def check_owned(buffer:&Buffer,raw:i64,first:i64,second:i64)->i64{sg_check_owned(buffer.handle,raw,first,second)} def check_copy(buffer:&Buffer,kind:i64,first:i64,second:i64)->i64{sg_check_copy(buffer.handle,kind,first,second)}', 'unicode_version': 'struct Buffer{handle:i64} extern "C"{fn sg_buffer_acquire()->i64;fn sg_buffer_release(value:i64);fn sg_buffer_live()->i64;fn sg_buffer_frees()->i64;fn sg_check_owned(value:i64,raw:i64,first:i64,second:i64)->i64;fn sg_check_copy(value:i64,kind:i64,first:i64,second:i64)->i64;fn sengoo_ffi_buffer_ptr(value:i64)->i64;fn sengoo_ffi_buffer_len(value:i64)->i64;fn sengoo_ffi_buffer_get_u8(value:i64,index:i64)->i64;} impl Drop for Buffer{def drop(&mut self){sg_buffer_release(self.handle);}} extern "C"{fn sengoo_unicode_version_copy(output:i64,capacity:i64)->i64;} def copy(buffer:Buffer)->i64{sengoo_unicode_version_copy(sengoo_ffi_buffer_ptr(buffer.handle),sengoo_ffi_buffer_len(buffer.handle))} def exercise()->i64{let buffer=Buffer{handle:sg_buffer_acquire()};let first=copy(buffer);let second=copy(buffer);check_copy(&buffer,0,first,second)} def main()->i64{let mut round=0;while round<100{if exercise()!=42{return 1;};if sg_buffer_live()!=0 or sg_buffer_frees()!=round+1{return 2;};round=round+1;};42} def check_owned(buffer:&Buffer,raw:i64,first:i64,second:i64)->i64{sg_check_owned(buffer.handle,raw,first,second)} def check_copy(buffer:&Buffer,kind:i64,first:i64,second:i64)->i64{sg_check_copy(buffer.handle,kind,first,second)}', 'unicode_provenance': 'struct Buffer{handle:i64} extern "C"{fn sg_buffer_acquire()->i64;fn sg_buffer_release(value:i64);fn sg_buffer_live()->i64;fn sg_buffer_frees()->i64;fn sg_check_owned(value:i64,raw:i64,first:i64,second:i64)->i64;fn sg_check_copy(value:i64,kind:i64,first:i64,second:i64)->i64;fn sengoo_ffi_buffer_ptr(value:i64)->i64;fn sengoo_ffi_buffer_len(value:i64)->i64;fn sengoo_ffi_buffer_get_u8(value:i64,index:i64)->i64;} impl Drop for Buffer{def drop(&mut self){sg_buffer_release(self.handle);}} extern "C"{fn sengoo_unicode_provenance_copy(output:i64,capacity:i64)->i64;} def copy(buffer:Buffer)->i64{sengoo_unicode_provenance_copy(sengoo_ffi_buffer_ptr(buffer.handle),sengoo_ffi_buffer_len(buffer.handle))} def exercise()->i64{let buffer=Buffer{handle:sg_buffer_acquire()};let first=copy(buffer);let second=copy(buffer);check_copy(&buffer,1,first,second)} def main()->i64{let mut round=0;while round<100{if exercise()!=42{return 1;};if sg_buffer_live()!=0 or sg_buffer_frees()!=round+1{return 2;};round=round+1;};42} def check_owned(buffer:&Buffer,raw:i64,first:i64,second:i64)->i64{sg_check_owned(buffer.handle,raw,first,second)} def check_copy(buffer:&Buffer,kind:i64,first:i64,second:i64)->i64{sg_check_copy(buffer.handle,kind,first,second)}', 'ffi_last_error': 'extern "C"{fn sg_prepare_error(capacity:i64)->i64;} struct Buffer{handle:i64} extern "C"{fn sg_buffer_acquire()->i64;fn sg_buffer_release(value:i64);fn sg_buffer_live()->i64;fn sg_buffer_frees()->i64;fn sg_check_owned(value:i64,raw:i64,first:i64,second:i64)->i64;fn sg_check_copy(value:i64,kind:i64,first:i64,second:i64)->i64;fn sengoo_ffi_buffer_ptr(value:i64)->i64;fn sengoo_ffi_buffer_len(value:i64)->i64;fn sengoo_ffi_buffer_get_u8(value:i64,index:i64)->i64;} impl Drop for Buffer{def drop(&mut self){sg_buffer_release(self.handle);}} extern "C"{fn sengoo_ffi_last_error_copy(output:i64,capacity:i64)->i64;} def copy(buffer:Buffer)->i64{sengoo_ffi_last_error_copy(sengoo_ffi_buffer_ptr(buffer.handle),sg_prepare_error(sengoo_ffi_buffer_len(buffer.handle)))} def exercise()->i64{let buffer=Buffer{handle:sg_buffer_acquire()};let first=copy(buffer);let second=copy(buffer);check_copy(&buffer,2,first,second)} def main()->i64{let mut round=0;while round<100{if exercise()!=42{return 1;};if sg_buffer_live()!=0 or sg_buffer_frees()!=round+1{return 2;};round=round+1;};42} def check_owned(buffer:&Buffer,raw:i64,first:i64,second:i64)->i64{sg_check_owned(buffer.handle,raw,first,second)} def check_copy(buffer:&Buffer,kind:i64,first:i64,second:i64)->i64{sg_check_copy(buffer.handle,kind,first,second)}'}
GAP = b'reference-resource-gap live=1 created=1 freed=0 verified=1'

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def revision():
    return subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()

def declared_inputs(extra=()):
    paths = {Path(__file__).resolve(), ROOT / 'selfhost/tests/buffer_observer_probe.c', ROOT / 'selfhost/bootstrap.lock', ROOT / 'selfhost/reference-baseline.lock', ROOT / 'Cargo.toml', ROOT / 'Cargo.lock', ROOT / 'rust-toolchain.toml'}
    for directory in ['tools/stdlib', 'runtime']:
        paths.update((path for path in (ROOT / directory).rglob('*') if path.is_file() and (not any((part in {'target', 'build', '__pycache__'} for part in path.relative_to(ROOT / directory).parts)))))
    paths.update(extra)
    return {str(path): sha(path) for path in sorted(paths)}

def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--output', type=Path, default=ROOT / 'target/selfhost/m3-buffer-observers/checkpoint')
    p.add_argument('--seed', type=Path, default=ROOT / 'target/agent-conditional-compiler/bin/sgc.exe')
    p.add_argument('--reference', type=Path, default=ROOT / 'target/selfhost/seed-reference/release/sgc.exe')
    p.add_argument('--clang', type=Path, default=Path('C:/Users/admin/AppData/Local/SengooToolchain/clang+llvm-19.1.7-x86_64-pc-windows-msvc/bin/clang.exe'))
    p.add_argument('--reuse-compilers', type=Path, help='reuse only exact source/tool-bound compilers from a previous successful report')
    a = p.parse_args()
    out = a.output.resolve()
    if not out.is_relative_to(ROOT / 'target'):
        raise ValueError('outputs must stay within target/')
    out.mkdir(parents=True, exist_ok=True)
    report = {'schema': 1, 'stage': 'audited-native-input-borrow', 'result': 'running', 'm3_complete': False, 'parity_pass': False, 'started_at': dt.datetime.now(dt.timezone.utc).isoformat(), 'host': {'platform': platform.platform(), 'python': platform.python_version(), 'python_executable_sha256': sha(sys.executable)}, 'steps': [], 'cases': [], 'reference_resource_gaps': [], 'scope': 'candidate resource correctness; four precisely diagnosed frozen-reference Buffer cleanup leaks are retained as differences'}
    before = {}
    tools = {}
    snapshot = {}
    source_origins = {}
    verified_tools = {}
    extra_inputs = []

    def save():
        (out / 'evidence.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    save()
    try:
        report['source_revision'] = revision()
        lock = json.loads((ROOT / 'selfhost/bootstrap.lock').read_text(encoding='utf-8'))
        baseline = json.loads((ROOT / 'selfhost/reference-baseline.lock').read_text(encoding='utf-8'))
        tools = {'seed': (a.seed.resolve(), lock['seed']['sha256']), 'reference': (a.reference.resolve(), baseline['tools']['sgc']['sha256']), 'clang': (a.clang.resolve(), lock['clang']['sha256'])}
        for name, (path, digest) in tools.items():
            if sha(path) != digest:
                raise RuntimeError(name + ' identity mismatch')
            verified_tools[name] = (path, digest)
        report['tools'] = {name: {'path': str(path), 'sha256': digest, 'version': subprocess.check_output([str(path), '--version'], text=True, stderr=subprocess.STDOUT, timeout=30).strip()} for name, (path, digest) in tools.items()}
        entries = [ROOT / 'selfhost/tools/native.sg', ROOT / 'selfhost/tools/semantic.sg']
        pending = list(entries)
        seen = set()
        modules = {}
        snap = out / 'compiler-snapshot'
        snap.mkdir(exist_ok=True)
        while pending:
            path = pending.pop()
            if path in seen:
                continue
            seen.add(path)
            data = path.read_bytes()
            source_origins[str(path)] = hashlib.sha256(data).hexdigest()
            destination = snap / (('tool-' if path in entries else '') + path.stem + '.sg')
            destination.write_bytes(data)
            snapshot[str(destination)] = sha(destination)
            if path not in entries:
                modules[path.stem] = destination
            for name in re.findall('^\\s*import\\s+selfhost_([a-z0-9_]+)\\s*;', data.decode('utf-8'), re.M):
                pending.append(ROOT / 'selfhost/src' / (name + '.sg'))
        if any((sha(path) != digest for path, digest in source_origins.items())):
            raise RuntimeError('compiler source changed while capturing snapshot')
        report['compiler_source_origins'] = source_origins
        report['compiler_snapshot'] = snapshot
        if a.reuse_compilers:
            extra_inputs = [a.reuse_compilers.resolve(), a.reuse_compilers.resolve().parent / 'native.exe', a.reuse_compilers.resolve().parent / 'semantic.exe']
        before = declared_inputs(extra_inputs)
        report['inputs_before'] = before
        env = os.environ.copy()
        for name in list(env):
            if name.upper().startswith(('SENGOO_', 'SGPLATFORM_', 'SDL2_')):
                env.pop(name)
        env.update(SENGOO_STDLIB=str(ROOT / 'tools/stdlib'), SENGOO_RUNTIME=str(ROOT / 'tools/stdlib/runtime.c'), SENGOO_CLANG=str(a.clang.resolve()), SENGOO_MODULE_MAP=';'.join(('selfhost_' + name + '=' + str(path) for name, path in modules.items())))
        env['PATH'] = str(a.clang.resolve().parent) + os.pathsep + env.get('PATH', '')

        def run(name, command, expected=0):
            started = time.perf_counter()
            done = subprocess.run(list(map(str, command)), cwd=ROOT, env=env, capture_output=True, timeout=900)
            stdout = out / (name + '.stdout.log')
            stderr = out / (name + '.stderr.log')
            stdout.write_bytes(done.stdout)
            stderr.write_bytes(done.stderr)
            report['steps'].append({'name': name, 'command': list(map(str, command)), 'exit_code': done.returncode, 'expected_exit_code': expected, 'elapsed_s': time.perf_counter() - started, 'stdout_sha256': sha(stdout), 'stderr_sha256': sha(stderr)})
            save()
            print(name, done.returncode, flush=True)
            if done.returncode != expected:
                raise RuntimeError(name + ' failed')
            return done
        if a.reuse_compilers:
            producer_path = a.reuse_compilers.resolve()
            producer = json.loads(producer_path.read_text(encoding='utf-8'))
            if producer.get('stage') != report['stage'] or producer.get('result') != 'passed-with-reference-resource-gaps' or any(producer.get(field, []) for field in ['source_changes', 'snapshot_changes', 'tool_changes']):
                raise RuntimeError('compiler producer is not a successful unchanged-input report')
            if producer.get('compiler_source_origins') != source_origins:
                raise RuntimeError('compiler producer source closure differs')
            excluded = {str(Path(__file__).resolve()), str(ROOT / 'selfhost/tests/buffer_observer_probe.c')}
            for filename, digest in producer['inputs_before'].items():
                if filename not in excluded and sha(filename) != digest:
                    raise RuntimeError('compiler producer dependency differs: ' + filename)
            for name, (path, digest) in tools.items():
                if producer['tools'][name]['sha256'] != digest:
                    raise RuntimeError('compiler producer tool differs')
            for name in ['native', 'semantic']:
                source = producer_path.parent / (name + '.exe')
                if sha(source) != producer['artifacts'][name + '.exe']:
                    raise RuntimeError('compiler producer executable differs')
                shutil.copyfile(source, out / source.name)
            report['compiler_reuse'] = {'report': str(producer_path), 'sha256': sha(producer_path), 'source_revision': producer['source_revision'], 'source_revision_after': producer['source_revision_after'], 'artifacts': producer['artifacts'], 'scope': 'reuses exact source/tool-bound compilers; this run recompiles runtime C and reruns all four resource cases'}
        else:
            for name in ['native', 'semantic']:
                target = out / (name + '.exe')
                target.unlink(missing_ok=True)
                run(name + '-build', [a.seed.resolve(), '--runtime-mode', 'source-development', 'build', snap / ('tool-' + name + '.sg'), '--force-rebuild', '-O', '0', '-o', target])
        for stem in ['runtime', 'runtime_string']:
            run(stem + '-compile', [a.clang.resolve(), '-O0', '-c', ROOT / 'tools/stdlib' / (stem + '.c'), '-o', out / (stem + '.obj')])
        for name, text in CASES.items():
            source = out / (name + ' 世界 &.sg')
            source.write_bytes(text.encode('utf-8'))
            obj = out / (name + '.obj')
            repeat = out / (name + '-repeat.obj')
            ir = out / (name + '.ll')
            candidate = out / (name + '-candidate.exe')
            reference = out / (name + '-reference.exe')
            for path in [obj, repeat, ir, candidate, reference]:
                path.unlink(missing_ok=True)
            row = {'name': name, 'source_sha256': sha(source), 'candidate_scopes': 100, 'reference_gap': 'first-scope Buffer retained by frozen legacy parameter/caller cleanup', 'result_equal': False}
            report['cases'].append(row)
            for suffix, path in [('emit', obj), ('repeat', repeat)]:
                emitted = run(name + '-' + suffix, [out / 'native.exe', source, path])
                if json.loads(emitted.stdout) != {'schema_version': 1, 'ok': True, 'phase': 'object'} or emitted.stderr:
                    raise RuntimeError('object protocol mismatch')
            if obj.read_bytes() != repeat.read_bytes():
                raise RuntimeError('object nondeterminism')
            base = [a.clang.resolve(), ROOT / 'selfhost/tests/buffer_observer_probe.c', out / 'runtime.obj', out / 'runtime_string.obj']
            run(name + '-candidate-link', [*base, obj, '-o', candidate])
            actual = run(name + '-candidate-execute', [candidate], 42)
            if actual.stdout or actual.stderr:
                raise RuntimeError('candidate resource/output assertion failed')
            run(name + '-reference-ir', [a.reference.resolve(), '--runtime-mode', 'source-development', 'build', source, '--force-rebuild', '-O', '0', '--emit-llvm', '-o', ir])
            run(name + '-reference-link', [*base, ir, '-o', reference])
            oracle = run(name + '-reference-execute', [reference], 2)
            if oracle.stdout or oracle.stderr.strip() != GAP:
                raise RuntimeError('reference failed outside the precise resource gap')
            if sha(source) != row['source_sha256']:
                raise RuntimeError('source changed during differential execution')
            row.update(candidate_result='passed', reference_result='resource-gap', object_sha256=sha(obj), reference_ir_sha256=sha(ir))
            report['reference_resource_gaps'].append(name)
        entry = out / 'actual-string.sg'
        entry.write_text('import std::string;def main(){}', encoding='utf-8')
        result = run('actual-stdlib-string', [out / 'semantic.exe', entry])
        actual = json.loads(result.stdout)
        if actual.get('ok') is not True or actual.get('phase') != 'ownership' or actual.get('diagnostics') != []:
            raise RuntimeError('actual String module did not pass ownership')
        report['actual_string'] = actual
        report['actual_string_source_sha256'] = sha(entry)
        report['artifacts'] = {name: sha(out / name) for name in ['native.exe', 'semantic.exe', 'runtime.obj', 'runtime_string.obj']}
        report['result'] = 'passed-with-reference-resource-gaps'
    except BaseException as error:
        report['result'] = 'failed'
        report['failure'] = type(error).__name__ + ': ' + str(error)
    finally:
        try:
            report['inputs_after'] = declared_inputs(extra_inputs) if before else {}
            report['source_changes'] = [path for path in sorted(set(before) | set(report['inputs_after'])) if before.get(path) != report['inputs_after'].get(path)]
            report['snapshot_changes'] = [path for path, digest in snapshot.items() if sha(path) != digest]
            report['checkout_changes'] = [path for path, digest in source_origins.items() if sha(path) != digest]
            report['tool_changes'] = [name for name, (path, digest) in verified_tools.items() if sha(path) != digest]
            report['source_revision_after'] = revision()
            if report['source_changes'] or report['snapshot_changes'] or report['tool_changes']:
                report['result'] = 'failed'
                report['source_guard_failure'] = 'snapshot/dependency/tool inputs changed'
        except BaseException as error:
            report['result'] = 'failed'
            report['finally_failure'] = type(error).__name__ + ': ' + str(error)
        report['finished_at'] = dt.datetime.now(dt.timezone.utc).isoformat()
        save()
    print(json.dumps({key: report.get(key) for key in ['result', 'failure', 'source_changes', 'snapshot_changes', 'checkout_changes', 'tool_changes']}), flush=True)
    return 0 if report['result'] == 'passed-with-reference-resource-gaps' else 1
if __name__ == '__main__':
    raise SystemExit(main())
