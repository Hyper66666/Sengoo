"""Verify the production runtime pointer status guard against qualified old compiler controls."""
from pathlib import Path
import argparse, ctypes, datetime as dt, hashlib, json, os, platform, re, subprocess, sys, time
ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUT = ROOT / 'target/selfhost/m4-runtime-pointer-status/production-checkpoint'
PRODUCER = ROOT / 'target/selfhost/m3-buffer-observers/final-checkpoint/evidence.json'
POSITIVE = ROOT / 'target/selfhost/m3-projection-contract/final-checkpoint/evidence.json'
SEED = ROOT / 'target/agent-conditional-compiler/bin/sgc.exe'
CLANG = Path('C:/Users/admin/AppData/Local/SengooToolchain/clang+llvm-19.1.7-x86_64-pc-windows-msvc/bin/clang.exe')

def sha(p):
    return hashlib.sha256(Path(p).read_bytes()).hexdigest()

def revision():
    return subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()

def dependency_inputs(extra=()):
    paths = set(extra)
    paths.update((ROOT / name for name in ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'selfhost/bootstrap.lock']))
    for directory in ['tools/stdlib', 'runtime']:
        paths.update((path for path in (ROOT / directory).rglob('*') if path.is_file() and (not any((part in {'target', 'build', '__pycache__'} for part in path.relative_to(ROOT / directory).parts)))))
    return {str(path): sha(path) for path in sorted(paths)}

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=DEFAULT_OUT)
    parser.add_argument('--old-producer', type=Path, default=PRODUCER)
    parser.add_argument('--positive-producer', type=Path, default=POSITIVE)
    args = parser.parse_args()
    OUT = args.output.resolve()
    if not OUT.is_relative_to(ROOT / 'target'):
        raise ValueError('outputs must remain within target/')
    old_producer = args.old_producer.resolve()
    positive_producer = args.positive_producer.resolve()
    OUT.mkdir(parents=True, exist_ok=True)
    report = {'schema': 1, 'stage': 'production-runtime-pointer-status-guard', 'result': 'running', 'm3_complete': False, 'started_at': dt.datetime.now(dt.timezone.utc).isoformat(), 'host': {'platform': platform.platform(), 'python': platform.python_version(), 'python_executable_sha256': sha(sys.executable)}, 'steps': [], 'scope': 'immutable production native snapshot; checkout edits after capture are reported separately; reject non-positive provider results before a safe view consumer; no proof that arbitrary positive addresses are valid'}
    before = {}
    snapshots = {}
    tool_identity = {}
    paths = set()
    origins = {}

    def save():
        (OUT / 'evidence.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    save()
    try:
        if os.name != 'nt':
            raise RuntimeError('Windows native status gate unavailable on this host')
        report['source_revision'] = revision()
        lock = json.loads((ROOT / 'selfhost/bootstrap.lock').read_text(encoding='utf-8'))
        for name, path, expected in [('seed', SEED, lock['seed']['sha256']), ('clang', CLANG, lock['clang']['sha256'])]:
            if sha(path) != expected:
                raise RuntimeError(name + ' identity mismatch')
            tool_identity[str(path)] = expected
        producer = json.loads(old_producer.read_text(encoding='utf-8'))
        if producer['stage'] != 'audited-native-input-borrow' or producer['result'] != 'passed-with-reference-resource-gaps' or producer['source_changes'] or producer['snapshot_changes'] or producer['tool_changes']:
            raise RuntimeError('unqualified compiler producer')
        old = old_producer.parent / 'native.exe'
        if sha(old) != producer['artifacts']['native.exe']:
            raise RuntimeError('old compiler identity mismatch')
        report['producer'] = {'report_path': str(old_producer), 'report_sha256': sha(old_producer), 'compiler_sha256': sha(old)}
        for filename, expected in producer['compiler_snapshot'].items():
            if sha(filename) != expected:
                raise RuntimeError('old producer snapshot identity mismatch')
        if producer['tools']['seed']['sha256'] != lock['seed']['sha256'] or producer['tools']['clang']['sha256'] != lock['clang']['sha256']:
            raise RuntimeError('old producer tool identity mismatch')
        snapshot = OUT / 'snapshot'
        snapshot.mkdir(exist_ok=True)
        pending = [ROOT / 'selfhost/tools/native.sg']
        seen = set()
        modules = {}
        origins = {}
        while pending:
            path = pending.pop()
            if path in seen:
                continue
            seen.add(path)
            data = path.read_bytes()
            origins[str(path)] = hashlib.sha256(data).hexdigest()
            dest = snapshot / ('tool-native.sg' if path == ROOT / 'selfhost/tools/native.sg' else path.name)
            dest.write_bytes(data)
            snapshots[str(dest)] = sha(dest)
            if dest.name != 'tool-native.sg':
                modules[path.stem] = dest
            for name in re.findall('^\\s*import\\s+selfhost_([a-z0-9_]+)\\s*;', data.decode('utf-8'), re.M):
                pending.append(ROOT / 'selfhost/src' / (name + '.sg'))
        if any((sha(path) != digest for path, digest in origins.items())):
            raise RuntimeError('mixed source snapshot')
        report['source_origins'] = origins
        report['source_snapshot'] = snapshots
        positive = json.loads(positive_producer.read_text(encoding='utf-8'))
        if positive.get('result') != 'passed' or positive.get('stage') != 'contextual-projection-and-runtime-pointer' or positive.get('source_changes') or positive.get('tool_changes'):
            raise RuntimeError('positive fixture producer is not qualified')
        positive_row = next((row for row in positive['cases'] if row['name'] == 'runtime-pointer'))
        positive_source = positive_producer.parent / 'runtime-pointer 世界 &.sg'
        if sha(positive_source) != positive_row['source_sha256']:
            raise RuntimeError('positive source identity mismatch')
        paths = {Path(__file__), old_producer, old, positive_producer, positive_source, ROOT / 'selfhost/bootstrap.lock', ROOT / 'selfhost/tests/string_runtime_probe.c'}
        paths.update((Path(path) for path in producer['compiler_snapshot']))
        before = dependency_inputs(paths)
        report['inputs_before'] = before
        env = os.environ.copy()
        for name in list(env):
            if name.upper().startswith(('SENGOO_', 'SGPLATFORM_', 'SDL2_')):
                env.pop(name)
        env.update(SENGOO_STDLIB=str(ROOT / 'tools/stdlib'), SENGOO_RUNTIME=str(ROOT / 'tools/stdlib/runtime.c'), SENGOO_CLANG=str(CLANG), SENGOO_MODULE_MAP=';'.join(('selfhost_' + name + '=' + str(path) for name, path in modules.items())))
        env['PATH'] = str(CLANG.parent) + os.pathsep + env.get('PATH', '')
        ctypes.windll.kernel32.SetErrorMode(32775)

        def run(name, argv, expected):
            begin = time.perf_counter()
            result = subprocess.run(list(map(str, argv)), cwd=ROOT, env=env, capture_output=True, timeout=900)
            log = OUT / (name + '.log')
            log.write_bytes(result.stdout + result.stderr)
            report['steps'].append({'name': name, 'command': list(map(str, argv)), 'exit_code': result.returncode, 'expected_exit_code': expected, 'elapsed_s': time.perf_counter() - begin, 'log_sha256': sha(log)})
            save()
            print(name, result.returncode, flush=True)
            if result.returncode != expected:
                raise RuntimeError(name + ' failed')
            if name.endswith('-emit') and (json.loads(result.stdout) != {'schema_version': 1, 'ok': True, 'phase': 'object'} or result.stderr):
                raise RuntimeError(name + ' object protocol mismatch')
            return result
        candidate = OUT / 'native.exe'
        candidate.unlink(missing_ok=True)
        run('guard-compiler-build', [SEED, '--runtime-mode', 'source-development', 'build', snapshot / 'tool-native.sg', '--force-rebuild', '-O', '0', '-o', candidate], 0)
        for stem in ['runtime', 'runtime_string']:
            run(stem + '-compile', [CLANG, '-O0', '-c', ROOT / 'tools/stdlib' / (stem + '.c'), '-o', OUT / (stem + '.obj')], 0)
        source = positive_source
        obj = OUT / 'registered.obj'
        obj.unlink(missing_ok=True)
        run('registered-emit', [candidate, source, obj], 0)
        run('registered-link', [CLANG, ROOT / 'selfhost/tests/string_runtime_probe.c', OUT / 'runtime.obj', OUT / 'runtime_string.obj', obj, '-o', OUT / 'registered.exe'], 0)
        result = run('registered-100scopes', [OUT / 'registered.exe'], 42)
        if result.stdout or result.stderr:
            raise RuntimeError('registered resource assertion failed')
        callback = '#include <stdint.h>\n#include <stdio.h>\n#include <stdlib.h>\nint64_t sg_unreachable_text(const void *p){(void)p;fputs("borrowed-pointer-consumer-entered\\n",stderr);exit(55);}\n'
        for name, status, actual in [('zero', 0, False), ('invalid-status', -3, False), ('oom-status', -14, False), ('minimum-status', -9223372036854775807 - 1, False), ('actual-invalid-handle', 0, True)]:
            source = OUT / (name + '.sg')
            literal = '0' if status == -9223372036854775807 - 1 else str(status)
            source.write_text('struct String{handle:i64} impl Drop for String{def drop(&mut self){}} extern "C"{fn sengoo_string_as_str_ptr(handle:i64)->i64;fn sg_unreachable_text(value:&str)->i64;} def main()->i64{let owner=String{handle:' + literal + '};sg_unreachable_text(owner.as_str())}', encoding='utf-8')
            probe = OUT / (name + '.c')
            probe.write_text(callback + ('' if actual else 'int64_t sengoo_string_as_str_ptr(int64_t handle){(void)handle;return INT64_MIN;}\n' if status == -9223372036854775807 - 1 else 'int64_t sengoo_string_as_str_ptr(int64_t handle){return handle;}\n'), encoding='utf-8')
            objects = [OUT / 'runtime.obj', OUT / 'runtime_string.obj'] if actual else []
            for kind, compiler, expected in [('old', old, 55), ('guard', candidate, 3221225501)]:
                obj = OUT / (name + '-' + kind + '.obj')
                exe = OUT / (name + '-' + kind + '.exe')
                obj.unlink(missing_ok=True)
                exe.unlink(missing_ok=True)
                run(name + '-' + kind + '-emit', [compiler, source, obj], 0)
                run(name + '-' + kind + '-link', [CLANG, probe, *objects, obj, '-o', exe], 0)
                result = run(name + '-' + kind + '-execute', [exe], expected)
                if result.stdout or result.stderr.replace(b'\r\n', b'\n') != (b'borrowed-pointer-consumer-entered\n' if kind == 'old' else b''):
                    raise RuntimeError('consumer reachability assertion failed')
        report['candidate_compiler_sha256'] = sha(candidate)
        report['result'] = 'passed'
    except BaseException as error:
        report['result'] = 'failed'
        report['failure'] = type(error).__name__ + ': ' + str(error)
    finally:
        try:
            report['inputs_after'] = dependency_inputs(paths) if before else {}
            report['source_changes'] = [path for path in sorted(set(before) | set(report['inputs_after'])) if before.get(path) != report['inputs_after'].get(path)]
            report['checkout_changes'] = [path for path, digest in origins.items() if sha(path) != digest]
            report['snapshot_changes'] = [path for path, digest in snapshots.items() if sha(path) != digest]
            report['tool_changes'] = [path for path, digest in tool_identity.items() if sha(path) != digest]
            report['source_revision_after'] = revision()
            if report['source_changes'] or report['snapshot_changes'] or report['tool_changes']:
                report['result'] = 'failed'
        except BaseException as error:
            report['result'] = 'failed'
            report['finally_failure'] = str(error)
        report['finished_at'] = dt.datetime.now(dt.timezone.utc).isoformat()
        save()
    print(json.dumps({name: report.get(name) for name in ['result', 'failure', 'source_changes', 'snapshot_changes', 'tool_changes']}), flush=True)
    return 0 if report['result'] == 'passed' else 1
if __name__ == '__main__':
    raise SystemExit(main())
