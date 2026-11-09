"""Verify actual native provider behavior behind the three audited copy-input contracts."""
from pathlib import Path
import argparse, datetime as dt, hashlib, json, platform, subprocess, sys, time
ROOT = Path(__file__).resolve().parents[1]
PROBE = ROOT / 'selfhost/tests/copy_input_contract_probe.c'

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def revision():
    return subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()

def inputs():
    paths = {Path(__file__).resolve(), PROBE, ROOT / 'selfhost/bootstrap.lock', ROOT / 'selfhost/src/borrow.sg', ROOT / 'selfhost/tests/borrow.sg'}
    paths.update((path for path in (ROOT / 'tools/stdlib').rglob('*') if path.is_file() and (not any((part in {'target', 'build', '__pycache__'} for part in path.relative_to(ROOT / 'tools/stdlib').parts)))))
    return {str(path): sha(path) for path in sorted(paths)}

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=ROOT / 'target/selfhost/m3-audited-copy-inputs/provider-checkpoint')
    parser.add_argument('--clang', type=Path, default=Path('C:/Users/admin/AppData/Local/SengooToolchain/clang+llvm-19.1.7-x86_64-pc-windows-msvc/bin/clang.exe'))
    args = parser.parse_args()
    out = args.output.resolve()
    clang = args.clang.resolve()
    if not out.is_relative_to(ROOT / 'target'):
        raise ValueError('outputs must remain inside target/')
    out.mkdir(parents=True, exist_ok=True)
    report = {'schema': 1, 'stage': 'audited-native-copy-input-providers', 'result': 'running', 'm3_complete': False, 'started_at': dt.datetime.now(dt.timezone.utc).isoformat(), 'host': {'platform': platform.platform(), 'python': platform.python_version(), 'python_executable_sha256': sha(sys.executable)}, 'steps': [], 'scope': '100 real registered scopes: independently allocated UTF8 slice storage and non-retaining bounded status destination copies'}
    before = {}
    expected = ''

    def save():
        (out / 'evidence.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    save()
    try:
        report['source_revision'] = revision()
        lock = json.loads((ROOT / 'selfhost/bootstrap.lock').read_text(encoding='utf-8'))
        expected = lock['clang']['sha256']
        if sha(clang) != expected:
            raise RuntimeError('clang identity mismatch')
        before = inputs()
        report['inputs_before'] = before
        report['clang'] = {'path': str(clang), 'sha256': expected, 'version': subprocess.check_output([str(clang), '--version'], text=True, stderr=subprocess.STDOUT, timeout=30).strip()}

        def run(name, command):
            begin = time.perf_counter()
            result = subprocess.run(list(map(str, command)), cwd=ROOT, capture_output=True, timeout=120)
            log = out / (name + '.log')
            log.write_bytes(result.stdout + result.stderr)
            report['steps'].append({'name': name, 'command': list(map(str, command)), 'exit_code': result.returncode, 'seconds': time.perf_counter() - begin, 'log_sha256': sha(log)})
            save()
            print(name, result.returncode, flush=True)
            if result.returncode or (name == 'execute' and (result.stdout or result.stderr)):
                raise RuntimeError(name + ' failed')
        for name in ['runtime', 'runtime_string']:
            obj = out / (name + '.obj')
            obj.unlink(missing_ok=True)
            run(name + '-compile', [clang, '-O0', '-c', ROOT / 'tools/stdlib' / (name + '.c'), '-o', obj])
        exe = out / 'provider.exe'
        exe.unlink(missing_ok=True)
        run('link', [clang, '-O0', '-I', ROOT / 'tools/stdlib', PROBE, out / 'runtime.obj', out / 'runtime_string.obj', '-o', exe])
        run('execute', [exe])
        report['executable_sha256'] = sha(exe)
        report['result'] = 'passed'
    except BaseException as error:
        report.update(result='failed', failure=type(error).__name__ + ': ' + str(error))
    finally:
        try:
            after = inputs() if before else {}
            report['inputs_after'] = after
            report['source_changes'] = [path for path in sorted(set(before) | set(after)) if before.get(path) != after.get(path)]
            report['tool_changes'] = [] if not expected or sha(clang) == expected else ['clang']
            report['source_revision_after'] = revision()
            if report['source_changes'] or report['tool_changes']:
                report['result'] = 'failed'
        except BaseException as error:
            report.update(result='failed', finally_failure=type(error).__name__ + ': ' + str(error))
        report['finished_at'] = dt.datetime.now(dt.timezone.utc).isoformat()
        save()
    print(json.dumps({name: report.get(name) for name in ['result', 'failure', 'source_changes', 'tool_changes']}), flush=True)
    return 0 if report['result'] == 'passed' else 1
if __name__ == '__main__':
    raise SystemExit(main())
