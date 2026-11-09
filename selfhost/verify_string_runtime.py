"""Link a qualified pointer-bridge artifact to the actual registered C String runtime.

The producer gate must have passed first. This gate verifies its exact object,
LLVM IR and source hashes instead of claiming to rebuild their frontend.
"""
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def revision() -> str:
    return subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--producer', type=Path, default=ROOT/'target/selfhost/m3-projection-contract/final-checkpoint/evidence.json')
    parser.add_argument('--clang', type=Path, default=Path('C:/Users/admin/AppData/Local/SengooToolchain/clang+llvm-19.1.7-x86_64-pc-windows-msvc/bin/clang.exe'))
    parser.add_argument('--output', type=Path, default=ROOT/'target/selfhost/m3-string-runtime/checkpoint')
    args = parser.parse_args()
    out = args.output.resolve()
    if not out.is_relative_to(ROOT/'target'):
        raise ValueError('gate outputs must remain inside target/')
    out.mkdir(parents=True, exist_ok=True)
    producer_path, clang = args.producer.resolve(), args.clang.resolve()
    report = {'schema': 1, 'stage': 'registered-string-runtime-pointer-bridge', 'result': 'running', 'm3_complete': False,
              'started_at': dt.datetime.now(dt.timezone.utc).isoformat(), 'host': platform.platform(), 'steps': [],
              'scope': 'reuse exact qualified producer artifacts; compile actual runtime.c/runtime_string.c; 100 registered String scopes per executable',
              'remaining': ['complete stdlib lowering/ownership parity', 'full runtime SDK integration', 'self-build and performance cutover']}
    before: dict[str, str] = {}
    expected_clang = ''

    def save() -> None:
        (out/'evidence.json').write_text(json.dumps(report, indent=2)+'\n', encoding='utf-8')

    save()
    try:
        report['source_revision'] = revision()
        lock = json.loads((ROOT/'selfhost/bootstrap.lock').read_text(encoding='utf-8'))
        baseline = json.loads((ROOT/'selfhost/reference-baseline.lock').read_text(encoding='utf-8'))
        expected_clang = lock['clang']['sha256']
        if sha(clang) != expected_clang:
            raise RuntimeError('clang identity mismatch')
        producer = json.loads(producer_path.read_text(encoding='utf-8'))
        if producer.get('result') != 'passed' or producer.get('stage') != 'contextual-projection-and-runtime-pointer' or producer.get('source_changes') or producer.get('tool_changes'):
            raise RuntimeError('producer gate is not a successful unchanged-input pointer-bridge gate')
        for name, expected in [('seed', lock['seed']['sha256']), ('reference', baseline['tools']['sgc']['sha256']), ('clang', expected_clang)]:
            if producer['tools'][name]['sha256'] != expected:
                raise RuntimeError('producer '+name+' identity mismatch')
        matches = [row for row in producer['cases'] if row['name'] == 'runtime-pointer']
        if len(matches) != 1 or matches[0].get('result') != 'passed':
            raise RuntimeError('missing successful runtime-pointer producer case')
        row = matches[0]
        for field, expected in [('scope_count', 100), ('observations_per_scope', 2), ('expected_created_per_scope', 1), ('expected_releases_per_scope', 1)]:
            if row.get(field) != expected:
                raise RuntimeError('producer resource contract mismatch')
        source, obj, ir = (producer_path.parent/name for name in ('runtime-pointer 世界 &.sg', 'runtime-pointer.obj', 'runtime-pointer.ll'))
        for path, field in [(source, 'source_sha256'), (obj, 'object_sha256'), (ir, 'reference_ir_sha256')]:
            if sha(path) != row[field]:
                raise RuntimeError('producer artifact identity mismatch: '+path.name)
        report['producer'] = {'report': str(producer_path), 'sha256': sha(producer_path), 'compiler_sha256': producer['compiler_sha256'],
                              'source_revision': producer['source_revision'], 'source_revision_after': producer['source_revision_after'], 'case': row}
        paths = {Path(__file__).resolve(), ROOT/'selfhost/tests/string_runtime_probe.c', ROOT/'selfhost/bootstrap.lock',
                 ROOT/'selfhost/reference-baseline.lock', producer_path, source, obj, ir}
        paths.update(path for path in (ROOT/'tools/stdlib').rglob('*') if path.is_file() and not any(part in {'build', 'target', '__pycache__'} for part in path.relative_to(ROOT/'tools/stdlib').parts))
        before = {str(path): sha(path) for path in sorted(paths)}
        report['inputs_before'] = before
        report['clang'] = {'path': str(clang), 'sha256': expected_clang,
                           'version': subprocess.check_output([str(clang), '--version'], text=True, stderr=subprocess.STDOUT, timeout=30).strip()}
        env = os.environ.copy()
        for name in list(env):
            if name.upper().startswith(('SENGOO_', 'SGPLATFORM_', 'SDL2_')):
                env.pop(name)
        env['PATH'] = str(clang.parent)+os.pathsep+env.get('PATH', '')

        def run(name: str, command: list, expected: int = 0) -> None:
            started = time.perf_counter()
            result = subprocess.run(list(map(str, command)), cwd=ROOT, env=env, capture_output=True, timeout=120)
            log = out/(name+'.log')
            log.write_bytes(result.stdout+result.stderr)
            report['steps'].append({'name': name, 'command': list(map(str, command)), 'exit_code': result.returncode,
                                    'expected_exit_code': expected, 'elapsed_s': time.perf_counter()-started, 'log_sha256': sha(log)})
            save()
            print(name, result.returncode, flush=True)
            if result.returncode != expected or name.endswith('execute') and (result.stdout or result.stderr):
                raise RuntimeError(name+' failed')

        objects = []
        for stem in ('runtime', 'runtime_string'):
            target = out/(stem+'.obj')
            target.unlink(missing_ok=True)
            run(stem+'-compile', [clang, '-O0', '-ffunction-sections', '-fdata-sections', '-c', ROOT/'tools/stdlib'/(stem+'.c'), '-o', target])
            objects.append(target)
        for name, artifact in [('candidate', obj), ('reference', ir)]:
            executable = out/(name+'.exe')
            executable.unlink(missing_ok=True)
            run(name+'-link', [clang, ROOT/'selfhost/tests/string_runtime_probe.c', *objects, artifact, '-Wl,/OPT:REF', '-o', executable])
            run(name+'-execute', [executable], 42)
        report['artifacts'] = {name: sha(out/name) for name in ('runtime.obj', 'runtime_string.obj', 'candidate.exe', 'reference.exe')}
        report['result'] = 'passed'
    except BaseException as error:
        report['result'] = 'failed'
        report['failure'] = type(error).__name__+': '+str(error)
    finally:
        try:
            report['inputs_after'] = {name: sha(Path(name)) for name in before}
            report['source_changes'] = [name for name in before if before[name] != report['inputs_after'][name]]
            report['tool_changes'] = [] if expected_clang and sha(clang) == expected_clang else ['clang']
            report['source_revision_after'] = revision()
            if report['source_changes'] or report['tool_changes']:
                report['result'] = 'failed'
                report['source_guard_failure'] = 'inputs changed during actual-runtime gate'
        except BaseException as error:
            report['result'] = 'failed'
            report['finally_failure'] = type(error).__name__+': '+str(error)
        report['finished_at'] = dt.datetime.now(dt.timezone.utc).isoformat()
        save()
    print(json.dumps({name: report.get(name) for name in ('result', 'failure', 'source_changes', 'tool_changes')}))
    return 0 if report['result'] == 'passed' else 1


if __name__ == '__main__':
    raise SystemExit(main())
