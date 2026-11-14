"""Exercise the actual auditor's guards with isolated files and reports."""
from pathlib import Path
import ast
import argparse
import hashlib
import importlib.util
import json
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
AUDITOR = ROOT / 'selfhost/audit_async_factory.py'


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('report', type=Path)
    parser.add_argument('--report-pin', required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if not __debug__:
        raise RuntimeError('controls require Python assertions enabled')
    REPORT, OUT = args.report.resolve(), args.output.resolve()
    if not OUT.is_relative_to(ROOT / 'target'):
        raise ValueError('control output must be below target')
    assert sha(REPORT) == args.report_pin
    OUT.mkdir(parents=True, exist_ok=False)
    source = AUDITOR.read_text('utf8')
    tree = ast.parse(source)
    audit = next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == 'audit')
    check = next(n for n in audit.body if isinstance(n, ast.FunctionDef) and n.name == 'check')
    config_guard = next(n for n in audit.body if isinstance(n, ast.Assert)
                        and 'for p, h in configs.items()' in ast.get_source_segment(source, n))
    environment = {'Path': Path, 'sha': sha, 'observed': {}, 'expected': {}}
    exec(compile(ast.Module(body=[check], type_ignores=[]), str(AUDITOR), 'exec'), environment)
    results = []
    payload = OUT / 'cache.bin'
    payload.write_bytes(b'original receipt')
    environment['check'](payload, sha(payload))
    environment['check'](payload, sha(payload))
    try:
        environment['check'](payload, '0' * 64)
        raise RuntimeError('conflicting cached expectation accepted')
    except AssertionError as error:
        assert 'conflicting expected hash' in str(error)
        results.append({'case': 'conflicting-expected-hash-after-cache', 'result': 'rejected', 'error': str(error)})

    config = OUT / 'previously-absent-config.toml'
    environment['configs'] = {str(config): None}
    guard = compile(ast.Module(body=[config_guard], type_ignores=[]), str(AUDITOR), 'exec')
    exec(guard, environment)
    config.write_text('[build]\njobs=1\n', 'utf8')
    try:
        exec(guard, environment)
        raise RuntimeError('new configuration accepted')
    except AssertionError:
        results.append({'case': 'missing-to-present-configuration', 'result': 'rejected'})

    # A private copy adds the changed copy of a real command receipt, keeping
    # that receipt's original expected hash. All original evidence stays intact.
    original = json.loads(REPORT.read_bytes())
    receipt = REPORT.parent / (original['steps'][0]['name'] + '.stderr')
    changed = OUT / 'mutated-actual-rustc.stderr'
    changed.write_bytes(receipt.read_bytes() + b'changed\n')
    mutated = json.loads(REPORT.read_bytes())
    mutated['artifacts'][str(changed)] = sha(receipt)
    mutated_report = OUT / 'mutated-receipt-report.json'
    mutated_report.write_text(json.dumps(mutated), 'utf8')
    spec = importlib.util.spec_from_file_location('auditor_controls', AUDITOR)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    try:
        module.audit(mutated_report, sha(mutated_report))
        raise RuntimeError('mutated receipt accepted')
    except AssertionError as error:
        assert 'changed: ' + str(changed) == str(error), str(error)
        results.append({'case': 'mutated-command-receipt', 'result': 'rejected', 'error': str(error)})

    # A wrong trusted pin must fail before parsing even malformed JSON, and
    # atomically replace an existing successful audit with failed status.
    malformed = OUT / 'changed-report.json'
    malformed.write_bytes(b'{invalid JSON')
    output = OUT / 'prior-pass.json'
    output.write_text('{"result":"passed-stale"}', 'utf8')
    command = [sys.executable, '-B', '-X', 'utf8', str(AUDITOR), str(malformed),
               '--report-pin', '0' * 64, '--output', str(output)]
    done = subprocess.run(command, cwd=ROOT, capture_output=True)
    (OUT / 'wrong-pin.stdout').write_bytes(done.stdout)
    (OUT / 'wrong-pin.stderr').write_bytes(done.stderr)
    status = json.loads(output.read_bytes())
    assert done.returncode != 0 and status['result'] == 'failed'
    assert 'changed: ' + str(malformed) in status['error']
    assert b'JSONDecodeError' not in done.stderr
    assert not list(OUT.glob('prior-pass.json.tmp-*'))
    results.append({'case': 'report-pin-before-parse-and-stale-pass-invalidation',
                    'result': 'rejected', 'exit_code': done.returncode,
                    'command': command, 'failure_status': status})
    # Optimized Python would remove the auditor's assertions. Explicitly refuse
    # that mode after invalidating an existing output, before parsing the report.
    optimized_output = OUT / 'optimized-prior-pass.json'
    optimized_output.write_text('{"result":"passed-stale"}', 'utf8')
    optimized = [sys.executable, '-O', '-B', '-X', 'utf8', str(AUDITOR), str(REPORT),
                 '--report-pin', sha(REPORT), '--output', str(optimized_output)]
    done = subprocess.run(optimized, cwd=ROOT, capture_output=True)
    (OUT / 'optimized.stdout').write_bytes(done.stdout)
    (OUT / 'optimized.stderr').write_bytes(done.stderr)
    status = json.loads(optimized_output.read_bytes())
    assert done.returncode != 0 and status['result'] == 'failed'
    assert 'RuntimeError: auditor requires Python assertions enabled' in status['error']
    assert not list(OUT.glob('optimized-prior-pass.json.tmp-*'))
    results.append({'case': 'optimized-python-rejected-and-stale-pass-invalidated',
                    'result': 'rejected', 'exit_code': done.returncode,
                    'command': optimized, 'failure_status': status})

    result = {'schema': 1, 'result': 'passed-isolated-auditor-negative-controls',
              'auditor_sha256': sha(AUDITOR), 'controls_sha256': sha(__file__),
              'actual_guard_lines': {'check': check.lineno, 'final_config': config_guard.lineno},
              'original_report_sha256': sha(REPORT), 'controls': results,
              'artifacts': {str(p): sha(p) for p in OUT.iterdir() if p.is_file()}}
    (OUT / 'evidence.json').write_text(json.dumps(result, indent=2) + '\n', 'utf8')
    print(json.dumps({k: v for k, v in result.items() if k != 'artifacts'}))


if __name__ == '__main__':
    main()
