"""Execute the admitted development Rust readiness provider through independent C.

This gate retains unchanged runtime sources and an explicit test-workspace
derivation. It does not materialize a final SDK or prove SG Future lowering.
"""
from __future__ import annotations

import argparse
import copy
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import time
import tomllib

from verify_compiler_source_snapshots import dependencies, rust_override

ROOT = Path(__file__).resolve().parents[1]
ADMISSION_PIN = 'c9ddf42b4ceacbc838e6b291c5f3348ef0d893f10cb27a8a1a53fae00fd9b8ee'
CATALOG_PIN = 'f044410918f66b38c6059b5e4b29f94aa35ea08c748fcdd3d795a7ac4c055bbb'
PREFIX = 'sengoo_async_file_wait_readable__'
OPERATIONS = [
    ('start', [1, 1], 1, False, False),
    ('poll', [1], 1, True, False),
    ('result', [3, 1], 0, True, True),
    ('cancel', [1], 2, True, True),
    ('drop', [1], 0, True, True),
]
CASES = [
    'disk-ready-terminal-repoll-result-read5', 'source-close-before-disk-poll',
    'cancel-before-poll', 'drop-before-poll', 'pipe-pending-write-ready-read5',
    'source-close-pending-cancel', 'drop-pending', 'empty-pipe-timeout',
    'negative-timeout', 'invalid-file-registry-id', 'pending-result-consumes',
    'null-output-result-consumes', 'cancel-ready', 'drop-ready',
    'source-close-pending-then-data-ready', 'pipe-peer-close-ready',
]


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def revision():
    return subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()


def verify_catalog(catalog, captured):
    """Correlate the already admitted catalog with this execution's exact inputs."""
    assert catalog['schema'] == 'sengoo.native-sdk.catalog.v1'
    assert (catalog['target'], catalog['pointer_bits'], catalog['endianness'], catalog['abi_version']) == (
        'x86_64-pc-windows-msvc', 64, 'little', 1)
    assert catalog['runtime_algorithms_ported'] is False
    assert catalog['components'][1] == dict(id=1, name='rust-native-bridge-reference-runtime',
                                           **{'class': 'reference-runtime'}, artifacts=[])
    sources = catalog['sources']
    paths = [row['path'] for row in sources]
    assert len(paths) == len(set(paths))
    required = {'runtime/src/async_runtime/async_file.rs', 'runtime/src/async_runtime.rs',
                'runtime/src/async_runtime/futures.rs', 'runtime/Cargo.toml', 'tools/stdlib/file.sg'}
    assert required <= set(paths)
    for row in sources:
        path = (captured / row['path']).resolve()
        assert path.is_relative_to(captured) and sha(path) == row['sha256']
        assert path.stat().st_size == row['bytes']
    assert sha(captured / catalog['prelude']['path']) == catalog['prelude']['sha256']
    selected = [row for row in catalog['exports'] if row['family'] == 3]
    assert len(selected) == 5
    for index, (row, (operation, parameters, result, unsafe, consumes)) in enumerate(zip(selected, OPERATIONS)):
        assert {k: row[k] for k in ['id', 'symbol', 'component', 'family', 'convention',
                                    'parameters', 'result', 'unsafe', 'consumes_handle']} == dict(
            id=11 + index, symbol=PREFIX + operation, component=1, family=3, convention='C',
            parameters=parameters, result=result, unsafe=unsafe, consumes_handle=consumes)
        source = sources[row['source']]
        assert source['path'] == 'runtime/src/async_runtime/async_file.rs'
        raw = (captured / source['path']).read_bytes()
        begin, end = row['prototype_begin'], row['prototype_end']
        assert 0 <= begin < end <= len(raw)
        assert ' '.join(raw[begin:end].decode('utf8').split()) == row['prototype']
    assert catalog['layouts'] == [dict(id=0, name='ShNativeFileReadinessOutcome', size=16, alignment=8,
        fields=[dict(name='is_ok', type=2, offset=0), dict(name='value', type=2, offset=1),
                dict(name='error', type=1, offset=8)])]
    assert catalog['language_contracts'][2] == dict(kind=3,
        language_function='fn(&AsyncFile,i64)->Future<FileReadinessOutcome>',
        origin='sengoo_async_file_wait_readable', producer=11, output='FileReadinessOutcome',
        operations=[11, 12, 13, 14, 15], runtime_result='owned-handle')
    return selected


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--clang', type=Path, required=True)
    parser.add_argument('--admission-report', type=Path, required=True)
    parser.add_argument('--admission-pin', required=True)
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to(ROOT / 'target')
    out.mkdir(parents=True, exist_ok=False)
    captured = out / 'implementation'
    sources, origins, artifacts, tools, configs = {}, {}, {}, {}, {}
    report = dict(schema=1, stage='native-readiness-development-provider-lifecycle', result='running',
        host=platform.platform(), source_revision=revision(), started_at=dt.datetime.now(dt.timezone.utc).isoformat(),
        sources=sources, producer_origins=origins, artifacts=artifacts, tools=tools,
        cargo_config_candidates=configs, steps=[], final_sdk=False, runtime_algorithms_ported=False,
        sg_lowering_qualified=False)

    def save():
        (out / 'evidence.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf8')

    def bind(path, expected=None):
        path = Path(path).resolve()
        digest = sha(path)
        assert expected is None or digest == expected, str(path)
        assert str(path) not in sources or sources[str(path)] == digest, 'input changed: ' + str(path)
        sources[str(path)] = digest
        return digest

    def artifact(path):
        path = Path(path).resolve()
        artifacts[str(path.relative_to(out))] = sha(path)

    def run(name, command, env, *, cwd=out, expected=0, timeout=900):
        command = list(map(str, command))
        start = time.monotonic()
        stdout, stderr = out / (name + '.stdout.log'), out / (name + '.stderr.log')
        with stdout.open('wb') as o, stderr.open('wb') as e:
            done = subprocess.run(command, cwd=cwd, env=env, stdout=o, stderr=e, timeout=timeout)
        artifact(stdout); artifact(stderr)
        report['steps'].append(dict(name=name, command=command, cwd=str(cwd), exit_code=done.returncode,
            expected_exit_code=expected, elapsed_s=time.monotonic() - start,
            stdout_sha256=sha(stdout), stderr_sha256=sha(stderr)))
        save(); print(name, done.returncode, flush=True)
        assert done.returncode == expected, (name, stdout.read_bytes()[-2000:], stderr.read_bytes()[-3000:])
        return stdout.read_bytes(), stderr.read_bytes()

    save()
    try:
        assert os.name == 'nt' and platform.machine().lower() in {'amd64', 'x86_64'}, 'Windows x64 gate'
        parent_path = args.admission_report.resolve()
        assert args.admission_pin == ADMISSION_PIN, 'unreviewed admission report pin'
        bind(parent_path, ADMISSION_PIN)
        parent = json.loads(parent_path.read_bytes())
        assert parent['result'] == 'passed-inventory-development-admission-not-final-sdk'
        assert not parent['source_changes'] and not parent['tool_changes']
        assert parent['source_revision'] == parent['source_revision_after']
        assert len(parent['executed_cases']) == 51 and parent['executed_cases'][0]['id'] == 'valid'
        parent_exe = parent_path.parent / 'native-sdk.exe'
        bind(parent_exe, parent['artifact_sha256'])
        catalog_path = ROOT / 'selfhost/sdk/native_x64_abi1/manifest.json'
        assert sha(catalog_path) == CATALOG_PIN
        catalog = json.loads(catalog_path.read_bytes())
        admitted = [catalog_path, ROOT / catalog['prelude']['path']]
        admitted.extend(ROOT / row['path'] for row in catalog['sources'])
        for path in admitted:
            digest = sha(path)
            assert parent['inputs_before'][str(path)] == digest == parent['inputs_after'][str(path)]
        report['admission_dependency'] = dict(path=str(parent_path), sha256=ADMISSION_PIN,
            executable=str(parent_exe), executable_sha256=sha(parent_exe), catalog_sha256=CATALOG_PIN,
            scope='Reused SG declaration admission for the identical catalog, prelude and all catalog source bytes; no inherited lifecycle claim')

        paths = set(admitted)
        paths.update(p for p in (ROOT / 'runtime').rglob('*') if p.is_file()
                     and not set(p.relative_to(ROOT / 'runtime').parts) & {'target', 'build', '__pycache__'})
        paths.update(ROOT / name for name in [
            'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'selfhost/bootstrap.lock',
            'selfhost/compiler_inputs.py', 'selfhost/verify_compiler_source_snapshots.py',
            'selfhost/verify_native_readiness_provider.py',
            'selfhost/tests/native_readiness_provider_probe.c', 'selfhost/tests/native_readiness_provider.rs'])
        paths.update(p for p in (ROOT / 'openspec/changes/add-selfhost-native-sdk').rglob('*.md')
                     if p.name not in {'tasks.md', 'checkpoint.md', 'readiness-checkpoint.md'})
        for path in sorted(paths):
            raw = path.read_bytes(); digest = hashlib.sha256(raw).hexdigest()
            destination = captured / path.relative_to(ROOT)
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(raw); origins[str(path)] = digest; bind(destination, digest)
        report['selected_exports'] = verify_catalog(catalog, captured)
        # These are qualification-oracle negative controls, not a replacement
        # for the pinned SG admission implementation's adversarial catalog gate.
        bad_catalogs = []
        missing = copy.deepcopy(catalog); missing['exports'].pop(); bad_catalogs.append(missing)
        changed = copy.deepcopy(catalog); changed['exports'][13]['parameters'][0] = 1; bad_catalogs.append(changed)
        changed = copy.deepcopy(catalog); changed['exports'][13]['consumes_handle'] = False; bad_catalogs.append(changed)
        rejected = 0
        for bad in bad_catalogs:
            try:
                verify_catalog(bad, captured)
            except AssertionError:
                rejected += 1
        assert rejected == 3
        report['catalog_correlation_negative_controls'] = ['missing-drop', 'result-integer-output', 'result-not-consuming']

        # Keep the original Cargo inputs immutable; use a separate workspace
        # with exactly one reviewed manifest-field transformation.
        workspace = out / 'workspace'
        shutil.copytree(captured / 'runtime', workspace / 'runtime')
        for path in (workspace / 'runtime').rglob('*'):
            if path.is_file():
                bind(path, sha(captured / path.relative_to(workspace)))
        original_text = (captured / 'Cargo.toml').read_text('utf8')
        derived_text, replacements = re.subn(r'(?ms)^members = \[.*?^\]', 'members = ["runtime", "probe"]', original_text, count=1)
        assert replacements == 1
        expected_manifest = tomllib.loads(original_text)
        expected_manifest['workspace']['members'] = ['runtime', 'probe']
        assert tomllib.loads(derived_text) == expected_manifest
        (workspace / 'Cargo.toml').write_text(derived_text, encoding='utf8'); bind(workspace / 'Cargo.toml')
        shutil.copyfile(captured / 'rust-toolchain.toml', workspace / 'rust-toolchain.toml'); bind(workspace / 'rust-toolchain.toml')
        shutil.copyfile(captured / 'Cargo.lock', workspace / 'Cargo.lock')
        probe = workspace / 'probe'
        (probe / 'src').mkdir(parents=True)
        (probe / 'Cargo.toml').write_text('[package]\nname="readiness-probe"\nversion="0.0.0"\nedition="2021"\n[dependencies]\nsengoo-runtime={path="../runtime",features=["native-bridge"]}\n', encoding='utf8')
        bind(probe / 'Cargo.toml')
        shutil.copyfile(captured / 'selfhost/tests/native_readiness_provider.rs', probe / 'src/main.rs'); bind(probe / 'src/main.rs')
        (probe / 'build.rs').write_text('fn main(){println!("cargo:warning=readiness_actual_rustc={}",std::env::var("RUSTC").unwrap());}\n', encoding='utf8'); bind(probe / 'build.rs')
        report['workspace_derivation'] = dict(original=str(captured / 'Cargo.toml'), original_sha256=sha(captured / 'Cargo.toml'),
            derived=str(workspace / 'Cargo.toml'), derived_sha256=sha(workspace / 'Cargo.toml'),
            transformation='Only workspace.members becomes [runtime, probe]; runtime source/manifests unchanged',
            original_lock=str(captured / 'Cargo.lock'), original_lock_sha256=sha(captured / 'Cargo.lock'))

        lock = json.loads((captured / 'selfhost/bootstrap.lock').read_bytes())
        env = {key: value for key, value in os.environ.items() if not rust_override(key)
               and not key.upper().startswith(('SENGOO_', 'SGPLATFORM_', 'SDL2_', 'PYTHON', 'LLDB_'))
               and key.upper() not in {'CL', '_CL_', 'LINK', '_LINK_'}}
        clang = args.clang.resolve()
        assert sha(clang) == lock['clang']['sha256']
        env['PATH'] = str(clang.parent) + os.pathsep + env.get('PATH', '')
        rustup = Path(shutil.which('rustup', path=env['PATH'])).resolve()
        selected_tools = {'clang': clang, 'llvm-nm': clang.parent / 'llvm-nm.exe', 'rustup': rustup, 'python': Path(sys.executable).resolve()}
        for name in ['cargo', 'rustc']:
            path = Path(subprocess.check_output([rustup, 'which', name], cwd=ROOT, env=env, text=True).strip()).resolve()
            assert sha(path) == lock[name + '_sha256']; selected_tools[name] = path
        assert selected_tools['cargo'].parent == selected_tools['rustc'].parent
        for name in ['kernel32.dll', 'ntdll.dll', 'KernelBase.dll']:
            selected_tools[name] = Path(os.environ['SystemRoot']) / 'System32' / name
        for name, path in selected_tools.items():
            tools[name] = dict(path=str(path), sha256=bind(path))
        env.update(CARGO=str(selected_tools['cargo']), RUSTC=str(selected_tools['rustc']),
            CARGO_BUILD_RUSTC=str(selected_tools['rustc']), RUSTUP_TOOLCHAIN=selected_tools['rustc'].parent.parent.name,
            RUSTC_WRAPPER='', RUSTC_WORKSPACE_WRAPPER='', CARGO_BUILD_RUSTC_WRAPPER='', CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER='',
            CARGO_INCREMENTAL='0', CARGO_NET_OFFLINE='true')
        env['PATH'] = str(selected_tools['cargo'].parent) + os.pathsep + env['PATH']
        candidates = {folder / '.cargo' / name for folder in {ROOT, *ROOT.parents, out, *out.parents, workspace}
                      for name in ['config', 'config.toml']}
        candidates.update(Path(env.get('CARGO_HOME', str(Path.home() / '.cargo'))) / name for name in ['config', 'config.toml'])
        for path in sorted(candidates):
            configs[str(path)] = sha(path) if path.is_file() else None
            if path.is_file():
                assert 'include' not in tomllib.loads(path.read_text('utf8')); bind(path)
        c_source = captured / 'selfhost/tests/native_readiness_provider_probe.c'
        obj = out / 'readiness-probe.obj'
        before, after = out / 'c-before.d', out / 'c-after.d'
        flags = ['-std=c11', '-O2', '-Wall', '-Wextra', '-Werror', '-Werror=date-time']
        run('c-header-closure', [clang, *flags, '-M', '-MT', 'snapshot_dependency', '-MF', before, c_source], env)
        headers = dependencies(before)
        for path, digest in headers.items():
            bind(path, digest)
        run('c-probe-compile', [clang, *flags, '-MD', '-MT', 'snapshot_dependency', '-MF', after, '-c', c_source, '-o', obj], env)
        assert dependencies(after) == headers
        report['c_headers'] = headers
        artifact(before); artifact(after); artifact(obj)
        cargo = selected_tools['cargo']
        # metadata preserves compatible entries in the supplied lock; a fresh
        # generate-lockfile would choose newer locally cached dependency versions.
        run('derive-workspace-lock', [cargo, 'metadata', '--offline', '--filter-platform',
            'x86_64-pc-windows-msvc', '--format-version', '1'], env, cwd=workspace)
        bind(workspace / 'Cargo.lock')
        original_packages = {(p['name'], p['version'], p.get('source')): p.get('checksum')
                             for p in tomllib.loads((captured / 'Cargo.lock').read_text('utf8'))['package']}
        derived_packages = tomllib.loads((workspace / 'Cargo.lock').read_text('utf8'))['package']
        for package in derived_packages:
            if package['name'] == 'readiness-probe':
                assert package == dict(name='readiness-probe', version='0.0.0', dependencies=['sengoo-runtime'])
            else:
                key = (package['name'], package['version'], package.get('source'))
                assert key in original_packages and package.get('checksum') == original_packages[key], key
        report['workspace_derivation'].update(derived_lock=str(workspace / 'Cargo.lock'),
            derived_lock_sha256=sha(workspace / 'Cargo.lock'), lock_rule='Only existing package versions/checksums plus the test harness; unused workspace packages may be removed')
        stdout, stderr = run('rust-provider-build', [cargo, 'rustc', '--offline', '--locked', '--verbose',
            '--message-format=json', '-p', 'readiness-probe', '--target-dir', out / 'build', '--', '-C', 'link-arg=' + str(obj)], env, cwd=workspace, timeout=1200)
        actual_commands = [line.strip() for line in stderr.decode('utf8').splitlines() if line.lstrip().startswith('Running ')]
        provider_commands = [line for line in actual_commands if '--crate-name sengoo_runtime ' in line]
        consumer_commands = [line for line in actual_commands if '--crate-name readiness_probe ' in line]
        assert len(provider_commands) == len(consumer_commands) == 1
        assert all(str(selected_tools['rustc']).casefold() in line.casefold() for line in provider_commands + consumer_commands)
        assert 'readiness_actual_rustc=' + str(selected_tools['rustc']) in stderr.decode('utf8')
        messages = [json.loads(line) for line in stdout.splitlines() if line.strip()]
        built = [m for m in messages if m['reason'] == 'compiler-artifact']
        providers = [m for m in built if m['target']['name'] == 'sengoo_runtime']
        assert len(providers) == 1
        provider = providers[0]
        assert Path(provider['target']['src_path']).resolve() == workspace / 'runtime/src/lib.rs'
        assert 'native-bridge' in provider['features'] and not provider['fresh']
        archives = [Path(p).resolve() for p in provider['filenames'] if p.endswith('.rlib')]
        assert len(archives) == 1
        archive = archives[0]
        assert '--extern sengoo_runtime=' + str(archive) in consumer_commands[0]
        assert '-C link-arg=' + str(obj) in consumer_commands[0]
        for message in built:
            for path in message['filenames']:
                artifact(path)
        symbol_output, _ = run('provider-defined-symbols', [selected_tools['llvm-nm'], '--defined-only', '--extern-only', '--format=posix', '--no-demangle', archive], env)
        symbols = symbol_output.decode('utf8').splitlines()
        required_symbols = [PREFIX + op[0] for op in OPERATIONS] + [
            'sengoo_async_file_open_read', 'sengoo_async_file_close', 'sengoo_async_file_read_into']
        observed = {}
        for symbol in required_symbols:
            rows = [line for line in symbols if re.match(re.escape(symbol) + r'\s+T\s+', line)]
            assert len(rows) == 1, (symbol, rows)
            observed[symbol] = rows[0]
        report['actual_provider'] = dict(archive=str(archive), sha256=sha(archive), defined_exports=observed,
            cargo_artifact=provider, provider_rustc_command=provider_commands[0], consumer_rustc_command=consumer_commands[0],
            binding='Rust typed extern-C function pointers from this exact archive passed to the independent C consumer')
        consumers = [m for m in built if m['target']['name'] == 'readiness-probe']
        assert len(consumers) == 1
        exe = Path(consumers[0]['executable']).resolve()
        fixture = out / 'ready 世界.txt'; fixture.write_bytes(b'ready'); bind(fixture)
        stdout, stderr = run('readiness-1600-resource-scopes', [exe, fixture], dict(env, PATH=''), timeout=60)
        expected = ['completed readiness case ' + str(i) + ', measured rounds 100' for i in range(16)]
        expected.append('passed readiness provider: 16 cases x 100 measured rounds; exact Rust allocation count/bytes and Win32 handles restored')
        assert stdout.decode('utf8').splitlines() == expected and not stderr
        report.update(result='passed-development-readiness-provider-lifecycle', cases=CASES,
            completed_case_ids=list(range(16)), measured_scopes=1600, initialization_scopes=16,
            observations=['Win32 process handle count', 'Rust global allocation count', 'Rust global allocation bytes',
                          'Independent C layout and output canaries', 'Real named-pipe Pending to data Ready'],
            limits=['Unchanged Rust reference-runtime development execution; no final SDK materialization or SG algorithm port',
                    'No SG Future lowering, scheduler wakeup propagation or compiler fixed-point claim',
                    'Initialization permits normal process-global registry table capacity retention',
                    'No calls through fabricated or consumed nonzero unsafe state pointers',
                    'Unrelated generated-program dispatch link requirements are abort-only test traps',
                    'Rust registry source closure and system linker-library provenance are not final-SDK qualified'])
    except BaseException as error:
        report.update(result='failed', failure=repr(error))
        raise
    finally:
        report.update(changed_sources=[p for p, h in sources.items() if not Path(p).is_file() or sha(p) != h],
            changed_origins=[p for p, h in origins.items() if not Path(p).is_file() or sha(p) != h],
            changed_artifacts=[p for p, h in artifacts.items() if not (out / p).is_file() or sha(out / p) != h],
            changed_tools=[n for n, v in tools.items() if not Path(v['path']).is_file() or sha(v['path']) != v['sha256']],
            changed_configs=[p for p, h in configs.items() if (sha(p) if Path(p).is_file() else None) != h],
            source_revision_end=revision(), completed_at=dt.datetime.now(dt.timezone.utc).isoformat())
        if any(report[name] for name in ['changed_sources', 'changed_origins', 'changed_artifacts', 'changed_tools', 'changed_configs']):
            report['result'] = 'failed'
        save(); print(sha(out / 'evidence.json'), flush=True)
    return 0 if report['result'] == 'passed-development-readiness-provider-lifecycle' else 1


if __name__ == '__main__':
    raise SystemExit(main())
