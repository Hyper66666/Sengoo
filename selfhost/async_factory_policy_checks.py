"""Case-policy controls against a caller-supplied immutable compiler snapshot.

run_policy_checks accepts run(name, command, env, expected=0, timeout=900)
returning (stdout_bytes, stderr_bytes). The callback must reject unexpected exits.
The standalone CLI supplies this callback. Parent qualification must additionally
guard its complete seed/Rust/system-link provenance; this component does not
inherit a qualification result from earlier private experiments.
"""
from __future__ import annotations
import argparse
import datetime as dt
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def dependencies(path):
    raw = Path(path).read_text('utf8').replace('\\\n', ' ')
    assert raw.startswith('snapshot_dependency:')
    payload = raw.split(':', 1)[1]
    words, word, position = [], [], 0
    while position < len(payload):
        c = payload[position]
        if c == '\\' and position + 1 < len(payload) and payload[position + 1] in ' \t#':
            word.append(payload[position + 1]); position += 2
        elif c == '$' and position + 1 < len(payload) and payload[position + 1] == '$':
            word.append('$'); position += 2
        elif c.isspace():
            if word:
                words.append(''.join(word)); word = []
            position += 1
        else:
            word.append(c); position += 1
    if word:
        words.append(''.join(word))
    assert words
    return {str(Path(p).resolve()): sha(p) for p in words}


def run_policy_checks(run, build_env, pure_env, snapshot, output, seed, clang):
    snapshot, output = Path(snapshot).resolve(), Path(output).resolve()
    seed, clang = Path(seed).resolve(), Path(clang).resolve()
    assert snapshot.is_dir() and output != snapshot and not snapshot.is_relative_to(output)
    output.mkdir(parents=True, exist_ok=False)
    implementation = output / 'implementation'
    templates = Path(__file__).resolve().parent
    sources, origins, artifacts = {}, {}, {}
    report = dict(schema=1, result='running', stage='factory-case-policy-response-controls',
        scope='Current caller snapshot; original adapter plus injected Win32 responses; development only',
        started_at=dt.datetime.now(dt.timezone.utc).isoformat(), sources=sources,
        source_origins=origins, artifacts=artifacts, steps=[], c_headers={},
        source_snapshot=str(snapshot), production_sources_modified=False)

    def save():
        (output / 'evidence.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf8')

    def bind(path, expected=None):
        path = Path(path).resolve(); value = sha(path)
        assert expected is None or value == expected, str(path)
        assert str(path) not in sources or sources[str(path)] == value
        sources[str(path)] = value
        return value

    def artifact(path):
        path = Path(path).resolve(); artifacts[str(path.relative_to(output))] = sha(path)

    def command(name, arguments, env, expected=0, timeout=900):
        arguments = list(map(str, arguments)); started = time.monotonic()
        stdout, stderr = run(name, arguments, env, expected=expected, timeout=timeout)
        assert isinstance(stdout, bytes) and isinstance(stderr, bytes)
        for stream, value in [('stdout', stdout), ('stderr', stderr)]:
            receipt = output / (name + '.' + stream + '.log')
            receipt.write_bytes(value); artifact(receipt)
        report['steps'].append(dict(name=name, command=arguments,
            expected_exit_code=expected, callback_confirmed_exit_code=expected,
            stdout_sha256=hashlib.sha256(stdout).hexdigest(),
            stderr_sha256=hashlib.sha256(stderr).hexdigest(), seconds=time.monotonic() - started))
        save()
        return stdout, stderr

    save()
    try:
        for path in [Path(__file__), templates / 'policy_probe.sg', templates / 'policy_wrapper.c', seed, clang]:
            bind(path)
        helper = snapshot / 'selfhost/compiler_inputs.py'; bind(helper)
        spec = importlib.util.spec_from_file_location('factory_policy_snapshot_inputs', helper)
        module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
        closure = module.compiler_closure(snapshot, [snapshot / 'selfhost/src/semantic_modules.sg'])
        paths = set(closure)
        paths.update(p for p in (snapshot / 'tools/stdlib').rglob('*') if p.is_file()
                     and not set(p.relative_to(snapshot / 'tools/stdlib').parts) & {'build', 'target', '__pycache__'})
        original_adapter = snapshot / 'selfhost/compiler_support/native/source_snapshot_os.c'
        for path in sorted(paths):
            raw = path.read_bytes(); digest = hashlib.sha256(raw).hexdigest()
            destination = implementation / path.relative_to(snapshot)
            # The original adapter keeps its own name in the input snapshot;
            # only this isolated output copy receives the API response wrapper.
            if path == original_adapter:
                destination = destination.with_name('source_snapshot_original.inc')
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(raw); origins[str(path)] = digest; bind(destination, digest)
        adapter = implementation / original_adapter.relative_to(snapshot)
        retained = adapter.with_name('source_snapshot_original.inc')
        assert retained.is_file()
        adapter.write_bytes((templates / 'policy_wrapper.c').read_bytes()); bind(adapter)
        report['derivation'] = dict(original=str(original_adapter), original_sha256=sha(original_adapter),
            retained=str(retained), retained_sha256=sha(retained), wrapper=str(adapter), wrapper_sha256=sha(adapter),
            transformation='Win32 query-response and observation macros include the unchanged adapter; actual C policy routine and all SG remain unchanged')
        entry = output / 'entry.sg'; entry.write_bytes(b'import std::file;\ndef main()->i64{42;}\n'); bind(entry)
        library = implementation / 'tools/stdlib'
        text = (templates / 'policy_probe.sg').read_text('utf8')
        text = text.replace('@ENTRY@', entry.as_posix()).replace('@LIBRARY@', library.as_posix())
        text = text.replace('@CASE_LIBRARY@', library.as_posix().upper())
        assert '@ENTRY@' not in text and '@LIBRARY@' not in text and '@CASE_LIBRARY@' not in text
        tests = implementation / 'selfhost/tests'; tests.mkdir(exist_ok=True)
        probe = tests / 'factory_case_policy.sg'; probe.write_text(text, encoding='utf8'); bind(probe)
        package = tests / 'Sengoo.toml'
        package.write_text('[package]\nname="factory-case-policy-controls"\nversion="0.0.1"\nedition="2026"\n', encoding='utf8'); bind(package)
        captured_closure = module.compiler_closure(implementation, [probe])
        report['actual_compiler_closure'] = {str(p): h for p, h in captured_closure.items()}
        for path, digest in captured_closure.items():
            bind(path, digest)
        env = dict(build_env, SENGOO_STDLIB=str(library), SENGOO_RUNTIME=str(library / 'runtime.c'),
            SENGOO_CLANG=str(clang), SENGOO_MODULE_MAP=module.module_map(implementation, captured_closure))
        pure = dict(pure_env, PATH='')
        flags = ['-std=c11', '-O2', '-Wall', '-Wextra', '-Werror', '-Werror=date-time']
        for index, source in enumerate(sorted(p for p in captured_closure if p.suffix == '.c')):
            before, after = output / f'c-{index}-before.d', output / f'c-{index}-after.d'
            obj = output / f'c-{index}.obj'
            command(f'policy-c-{index}-headers', [clang, *flags, '-M', '-MT', 'snapshot_dependency', '-MF', before, source], env)
            headers = dependencies(before)
            for path, digest in headers.items():
                bind(path, digest)
            command(f'policy-c-{index}-compile', [clang, *flags, '-MD', '-MT', 'snapshot_dependency', '-MF', after, '-c', source, '-o', obj], env)
            assert dependencies(after) == headers
            report['c_headers'][str(source)] = headers
            artifact(before); artifact(after); artifact(obj)
        cexe = output / 'retained-policy.exe'
        command('policy-independent-c-build', [clang, *flags, '-DPOLICY_STANDALONE', adapter, '-o', cexe], env)
        artifact(cexe)
        stdout, stderr = command('policy-independent-c-responses', [cexe, entry], pure)
        lines = stdout.decode('utf8').splitlines()
        assert not stderr and len(lines) == 4
        counts = []
        for index, line in enumerate(lines[:3]):
            match = re.fullmatch(r'policy C scope (\d+): (\d+) retained ancestor handles checked', line)
            assert match and int(match[1]) == index and int(match[2]) > 0
            counts.append(int(match[2]))
        assert lines[3] == 'policy C injected observations: ' + str(sum(counts) * 4)
        report['independent_c'] = dict(resource_scopes=3, ancestors_per_scope=counts,
            injected_observations=sum(counts) * 4, query_time_file_opens=0,
            query_handles='Each exact retained ancestor HANDLE checked in order; no reopened paths')
        executable = output / 'factory-case-policy.exe'
        command('policy-sg-build', [seed, '--runtime-mode', 'source-development', 'build', probe,
            '--force-rebuild', '-O', '0', '-o', executable], env)
        artifact(executable)
        stdout, stderr = command('policy-sg-source-facts-and-resources', [executable], pure, expected=42)
        expected = sum(([str(i), 'exact/case-policy source controls restored six resources'] for i in range(3)), [])
        assert not stderr and stdout.decode('utf8').splitlines() == expected
        report.update(result='passed-factory-case-policy-response-controls', sg_resource_scopes=3,
            resource_counters=['String', 'Buffer', 'opaque', 'FileSnapshot handles', 'FileSnapshot bytes', 'Win32 handles'],
            injected_controls=['all ancestors flags0', 'flags1 at every ancestor', 'unknown flags2 at every ancestor', 'ERROR_NOT_SUPPORTED at every ancestor'],
            source_facts='Exact paths accepted without querying policy; case variants accepted only with positive insensitive proof; rejections profile0/schema-1',
            limitations=['Injected responses do not claim execution on a real case-sensitive directory',
                'Current host control requires real queried ancestors to report insensitive',
                'Caller owns complete build/runtime/toolchain provenance; no final SDK or native lowering claim'])
    except BaseException as error:
        report.update(result='failed', error=repr(error))
        raise
    finally:
        report.update(changed_sources=[p for p, h in sources.items() if not Path(p).is_file() or sha(p) != h],
            changed_origins=[p for p, h in origins.items() if not Path(p).is_file() or sha(p) != h],
            changed_artifacts=[p for p, h in artifacts.items() if not (output / p).is_file() or sha(output / p) != h],
            completed_at=dt.datetime.now(dt.timezone.utc).isoformat())
        if any(report[key] for key in ['changed_sources', 'changed_origins', 'changed_artifacts']):
            report['result'] = 'failed'
        save()
    assert report['result'] == 'passed-factory-case-policy-response-controls'
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['snapshot', 'output', 'seed', 'clang']:
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    env = {k: v for k, v in os.environ.items() if not k.upper().startswith(('SENGOO_', 'PYTHON'))}
    env['PATH'] = str(args.clang.resolve().parent) + os.pathsep + env.get('PATH', '')
    def run(name, command, environment, expected=0, timeout=900):
        process = subprocess.run(command, cwd=args.output, env=environment, capture_output=True, timeout=timeout)
        print(name, process.returncode, flush=True)
        # On failure retain the streams before throwing, too.
        for stream, data in [('stdout', process.stdout), ('stderr', process.stderr)]:
            (args.output / (name + '.' + stream + '.log')).write_bytes(data)
        assert process.returncode == expected, (name, process.stdout[-2000:], process.stderr[-3000:])
        return process.stdout, process.stderr
    report = run_policy_checks(run, env, dict(env, PATH=''), args.snapshot, args.output, args.seed, args.clang)
    print(report['result'], sha(args.output / 'evidence.json'))


if __name__ == '__main__':
    main()
