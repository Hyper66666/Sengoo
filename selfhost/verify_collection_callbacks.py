"""Verify generated concrete MIR callbacks against a pinned development C provider."""
from __future__ import annotations

import argparse
import datetime as dt
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tomllib

from verify_collection_provider import ROOT, sha, closure, headers
from compiler_inputs import module_map


def revision():
    return subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['seed', 'clang', 'formatter', 'provider-report', 'output']:
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--provider-pin', required=True)
    args = parser.parse_args()
    base = args.output.resolve()
    assert base.is_relative_to(ROOT / 'target')
    base.mkdir(parents=True, exist_ok=False)
    parent_path = args.provider_report.resolve()
    assert sha(parent_path) == args.provider_pin
    parent = json.loads(parent_path.read_text('utf8'))
    assert parent['schema'] == 1 and parent['stage'] == 'collections-namespace-two-provider-mapping' and parent['result'] == 'passed'
    for name in ['changed_sources', 'checkout_changes', 'changed_tools', 'changed_configs', 'changed_artifacts']:
        assert parent[name] == []
    sources = {str(parent_path): args.provider_pin, str(Path(__file__).resolve()): sha(__file__),
               str(ROOT / 'selfhost/verify_collection_provider.py'): sha(ROOT / 'selfhost/verify_collection_provider.py')}
    sources.update(parent['sources'])
    for item in parent['tools'].values():
        assert sha(item['path']) == item['sha256']
        sources[item['path']] = item['sha256']
    for path, digest in sources.items():
        assert sha(path) == digest, path
    for name, digest in parent['artifacts'].items():
        path = parent_path.parent / name
        assert sha(path) == digest
        sources[str(path)] = digest
    for row in parent['steps']:
        assert row['exit_code'] == row.get('expected_exit_code', 0)
        for stream in ['stdout', 'stderr']:
            path = parent_path.parent / (row['name'] + '.' + stream + '.log')
            assert sha(path) == row[stream + '_sha256']
            sources[str(path)] = sha(path)
    provider_objects = [parent_path.parent / 'implementation/target/provider' / (name + '.obj')
                        for name in ['runtime', 'runtime_string', 'runtime_collections']]
    assert sha(provider_objects[-1]) == parent['provider_object_sha256']
    for path in provider_objects:
        assert str(path.relative_to(parent_path.parent)) in parent['artifacts']
    originals = closure(ROOT / 'selfhost/tests/collection_callbacks.sg')
    for directory in ['tools/stdlib', 'runtime']:
        originals.update({path: sha(path) for path in (ROOT / directory).rglob('*')
                          if path.is_file() and not set(path.relative_to(ROOT / directory).parts) & {'target', 'build', '__pycache__'}})
    for name in ['selfhost/tests/collection_callbacks_probe.c', 'selfhost/bootstrap.lock', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
                 'openspec/changes/add-selfhost-collection-intrinsics/proposal.md', 'openspec/changes/add-selfhost-collection-intrinsics/design.md',
                 'openspec/changes/add-selfhost-collection-intrinsics/specs/selfhost-collection-intrinsics/spec.md']:
        originals[ROOT / name] = sha(ROOT / name)
    captured = base / 'implementation'
    for path, digest in originals.items():
        copied = captured / path.relative_to(ROOT)
        copied.parent.mkdir(parents=True, exist_ok=True)
        raw = path.read_bytes()
        assert sha(path) == digest
        copied.write_bytes(raw)
        sources[str(copied)] = digest
    tools = {name: getattr(args, name).resolve() for name in ['seed', 'clang', 'formatter']}
    tools['python'] = Path(sys.executable).resolve()
    for name in ['lld-link', 'llvm-readobj', 'llvm-objdump']:
        tools[name] = tools['clang'].parent / (name + '.exe')
    lock = json.loads((ROOT / 'selfhost/bootstrap.lock').read_text('utf8'))
    for name in ['seed', 'clang', 'formatter']:
        assert sha(tools[name]) == lock[name]['sha256']
    env = {key: value for key, value in os.environ.items() if not key.upper().startswith(('RUSTC', 'RUSTDOC', 'RUSTUP_TOOLCHAIN', 'CARGO_', 'SENGOO_', 'SGPLATFORM_', 'SDL2_', 'PYTHON', 'LLDB_')) and key.upper() not in {'CARGO', 'RUSTFLAGS', 'RUSTDOCFLAGS', 'CL', '_CL_', 'LINK', '_LINK_'}}
    env['PATH'] = str(tools['clang'].parent) + os.pathsep + env.get('PATH', '')
    tools['rustup'] = Path(shutil.which('rustup', path=env['PATH'])).resolve()
    for name in ['cargo', 'rustc']:
        tools[name] = Path(subprocess.check_output([tools['rustup'], 'which', name], env=env, text=True).strip()).resolve()
        assert sha(tools[name]) == lock[name + '_sha256']
    env.update(CARGO=str(tools['cargo']), RUSTC=str(tools['rustc']), CARGO_BUILD_RUSTC=str(tools['rustc']), RUSTUP_TOOLCHAIN=tools['rustc'].parent.parent.name, RUSTC_WRAPPER='', RUSTC_WORKSPACE_WRAPPER='', CARGO_BUILD_RUSTC_WRAPPER='', CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER='')
    env['PATH'] = str(tools['cargo'].parent) + os.pathsep + env['PATH']
    env.update(SENGOO_STDLIB=str(captured / 'tools/stdlib'), SENGOO_RUNTIME=str(captured / 'tools/stdlib/runtime.c'), SENGOO_CLANG=str(tools['clang']), SENGOO_MODULE_MAP=module_map(ROOT, originals, captured))
    pure = {key: value for key, value in env.items() if not key.upper().startswith('SENGOO_')}
    pure['PATH'] = ''
    configs = {folder / '.cargo' / name for folder in [ROOT, *ROOT.parents, captured, *captured.parents] for name in ['config', 'config.toml']}
    configs.update(Path(os.environ.get('CARGO_HOME', str(Path.home() / '.cargo'))) / name for name in ['config', 'config.toml'])
    config_pins = {str(path): sha(path) if path.is_file() else None for path in configs}
    for path in configs:
        if path.is_file():
            assert 'include' not in tomllib.loads(path.read_text('utf8'))
    report = dict(schema=1, stage='generated-concrete-collection-callbacks', result='running', source_revision=revision(),
                  host=platform.platform(), started_at=dt.datetime.now(dt.timezone.utc).isoformat(), sources=sources,
                  qualified_checkout_sources={str(path): digest for path, digest in originals.items()},
                  provider_qualification=dict(path=str(parent_path), sha256=args.provider_pin, object_sha256=parent['provider_object_sha256']),
                  tools={name: dict(path=str(path), sha256=sha(path)) for name, path in tools.items()}, cargo_config_candidates=config_pins,
                  steps=[], artifacts={}, limits=['raw MIR concrete byte-move/source-clear and typed Drop generation only',
                  'no source intrinsic authority, HIR/provider/loan/ownership integration or optional-borrow permission',
                  'nonzero elements up to 1 MiB, alignment at most eight; complete over-aligned/zero-sized support remains open'])
    def save():
        (base / 'evidence.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf8')
    def run(name, command, expected=0, empty=False):
        done = subprocess.run(list(map(str, command)), cwd=ROOT, env=pure if empty else env, capture_output=True, timeout=900)
        for stream, data in [('stdout', done.stdout), ('stderr', done.stderr)]:
            (base / (name + '.' + stream + '.log')).write_bytes(data)
        report['steps'].append(dict(name=name, command=list(map(str, command)), exit_code=done.returncode, expected_exit_code=expected,
                                    empty_path=empty, stdout_sha256=sha(base / (name + '.stdout.log')), stderr_sha256=sha(base / (name + '.stderr.log'))))
        save(); print(name, done.returncode, flush=True)
        assert done.returncode == expected, (name, done.returncode, done.stdout[-1200:], done.stderr[-3000:])
        return done
    try:
        probe = base / 'cargo-probe'; (probe / 'src').mkdir(parents=True)
        (probe / 'Cargo.toml').write_text('[package]\nname="generated_callbacks_rustc_probe"\nversion="0.0.0"\nedition="2021"\n[workspace]\n')
        (probe / 'src/lib.rs').write_text('pub fn answer()->i64{42}\n')
        (probe / 'build.rs').write_text('fn main(){println!("cargo:warning=callbacks_rustc={}",std::env::var("RUSTC").unwrap());}\n')
        for generated in [probe / 'Cargo.toml', probe / 'src/lib.rs', probe / 'build.rs']:
            sources[str(generated)] = sha(generated)
        observed = run('cargo-actual-rustc', [tools['cargo'], 'build', '--offline', '--verbose', '--manifest-path', probe / 'Cargo.toml', '--target-dir', probe / 'target'])
        output = (observed.stdout + observed.stderr).decode('utf8')
        actual_runs = [line for line in output.splitlines() if line.lstrip().startswith('Running ') and str(tools['rustc']).casefold() in line.casefold()]
        assert len(actual_runs) >= 2 and 'callbacks_rustc=' + str(tools['rustc']) in output
        report['cargo_actual_rustc_invocations'] = actual_runs
        for path in originals:
            if path.suffix == '.sg' and path.name in ['collection_callbacks.sg']:
                run('format-' + path.parent.name, [tools['formatter'], '--check', captured / path.relative_to(ROOT)])
        binary = base / 'generator.exe'
        run('build-generator', [tools['seed'], '--runtime-mode', 'source-development', 'build', captured / 'selfhost/tests/collection_callbacks.sg', '--force-rebuild', '-O', '0', '-o', binary])
        for number in range(5):
            r = run('first-resource-' + str(number), [binary], 42, True)
            assert not r.stdout and not r.stderr
        objects = []
        for name in ['fresh-a', 'fresh-b']:
            directory = base / name; directory.mkdir()
            obj = directory / 'generated.obj'
            r = run(name + '-emit', [binary, obj], 42, True)
            assert not r.stdout and not r.stderr
            objects.append(obj); report['artifacts'][str(obj.relative_to(base))] = sha(obj)
        assert objects[0].read_bytes() == objects[1].read_bytes()
        run('object-inspection', [tools['llvm-readobj'], '--file-headers', '--sections', '--symbols', '--relocations', '--unwind', objects[0]])
        run('object-disassembly', [tools['llvm-objdump'], '-d', objects[0]])
        csource = captured / 'selfhost/tests/collection_callbacks_probe.c'
        flags = ['-std=c11', '-Wall', '-Wextra', '-Werror', '-Werror=date-time', '-O0', '-I', captured / 'tools/stdlib']
        before, after = base / 'before.d', base / 'after.d'
        run('c-headers', [tools['clang'], *flags, '-M', '-MT', 'provider_dependency', '-MF', before, csource])
        selected_headers = headers(before); sources.update(selected_headers)
        cobject = base / 'probe.obj'
        run('c-compile', [tools['clang'], *flags, '-MD', '-MT', 'provider_dependency', '-MF', after, '-c', csource, '-o', cobject])
        assert selected_headers == headers(after)
        report['c_headers'] = selected_headers
        executable = base / 'probe.exe'
        command = [tools['clang'], '-fuse-ld=lld', cobject, objects[0], *provider_objects, '-Wl,/verbose', '-o', executable]
        selected = run('actual-linker', [*command, '-###'])
        paths = re.findall(r'^\s*"([^"\r\n]*[\\/]lld-link(?:\.exe)?)"', selected.stderr.decode('utf8'), re.M | re.I)
        assert len(paths) == 1
        selected_path = Path(paths[0].replace('\\\\', '\\'))
        if not selected_path.suffix:
            selected_path = selected_path.with_suffix('.exe')
        assert selected_path.resolve() == tools['lld-link']
        linked = run('c-link', command)
        libraries = {}
        for match in re.finditer(rb'Reading ([^\r\n]+?\.lib)(?:\(|\r?$)', linked.stdout + linked.stderr, re.M):
            path = Path(match[1].decode('utf8')).resolve()
            if path.is_file():
                libraries[str(path)] = sha(path)
        assert libraries
        sources.update(libraries); report['c_link_libraries'] = libraries
        result = run('c-execute', [executable], 42, True)
        assert not result.stdout and not result.stderr
        for path in [binary, cobject, executable]:
            report['artifacts'][str(path.relative_to(base))] = sha(path)
        report.update(result='passed', first_call_resource_scopes=7, callback_resource_rounds=100, owning_values_released=10000, raw_objects_byte_equal=True)
    except BaseException as error:
        report.update(result='failed', error=repr(error))
        raise
    finally:
        report.update(changed_sources=[path for path, digest in sources.items() if not Path(path).is_file() or sha(path) != digest],
                      checkout_changes=[str(path) for path, digest in originals.items() if not path.is_file() or sha(path) != digest],
                      changed_tools=[name for name, item in report['tools'].items() if not Path(item['path']).is_file() or sha(item['path']) != item['sha256']],
                      changed_configs=[path for path, digest in config_pins.items() if (sha(path) if Path(path).is_file() else None) != digest],
                      changed_artifacts=[name for name, digest in report['artifacts'].items() if not (base / name).is_file() or sha(base / name) != digest],
                      source_revision_end=revision(), completed_at=dt.datetime.now(dt.timezone.utc).isoformat())
        if any(report[name] for name in ['changed_sources', 'checkout_changes', 'changed_tools', 'changed_configs', 'changed_artifacts']):
            report.update(result='failed', error='qualified input or artifact changed')
        save()
    return 0 if report['result'] == 'passed' else 1


if __name__ == '__main__':
    raise SystemExit(main())
