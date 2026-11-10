"""Qualify six C collection exports and their actual checked SG program bindings."""
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def revision():
    return subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()


def closure(entry):
    found, pending = {}, [entry]
    while pending:
        path = pending.pop()
        if path in found:
            continue
        found[path] = sha(path)
        pending.extend(ROOT / 'selfhost/src' / (name + '.sg') for name in
                       re.findall(r'import\s+selfhost_(\w+)\s*;', path.read_text('utf8')))
    return found


def headers(path):
    payload = path.read_text('utf8').replace('\\\n', ' ').split(':', 1)[1]
    words, word, index = [], [], 0
    while index < len(payload):
        char = payload[index]
        if char == '\\' and index + 1 < len(payload) and payload[index + 1] in ' \t#':
            word.append(payload[index + 1]); index += 2
        elif char == '$' and index + 1 < len(payload) and payload[index + 1] == '$':
            word.append('$'); index += 2
        elif char.isspace():
            if word:
                words.append(''.join(word)); word = []
            index += 1
        else:
            word.append(char); index += 1
    if word:
        words.append(''.join(word))
    result = {str(Path(word).resolve()): sha(Path(word)) for word in words}
    assert result
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['seed', 'clang', 'formatter', 'output']:
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    base = args.output.resolve()
    assert base.is_relative_to(ROOT / 'target')
    base.mkdir(parents=True, exist_ok=False)
    original = closure(ROOT / 'selfhost/tests/collection_provider.sg')
    for directory in ['tools/stdlib', 'runtime']:
        original.update({path: sha(path) for path in (ROOT / directory).rglob('*')
                         if path.is_file() and not set(path.relative_to(ROOT / directory).parts) & {'target', 'build', '__pycache__'}})
    for name in ['selfhost/sdk/collections_x64_abi1/prelude.sg', 'selfhost/sdk/collections_x64_abi1/manifest.json',
                 'selfhost/sdk/update_collections_inventory.py', 'selfhost/tests/collection_provider_probe.c',
                 'selfhost/bootstrap.lock', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
                 'openspec/changes/add-selfhost-collection-intrinsics/proposal.md',
                 'openspec/changes/add-selfhost-collection-intrinsics/design.md',
                 'openspec/changes/add-selfhost-collection-intrinsics/specs/selfhost-collection-intrinsics/spec.md']:
        original[ROOT / name] = sha(ROOT / name)
    inputs = {str(Path(__file__).resolve()): sha(__file__)}
    captured = base / 'implementation'
    for path, digest in original.items():
        destination = captured / path.relative_to(ROOT)
        destination.parent.mkdir(parents=True, exist_ok=True)
        data = path.read_bytes()
        assert hashlib.sha256(data).hexdigest() == digest
        destination.write_bytes(data)
        if path.name != 'manifest.json' or path.parent.name != 'collections_x64_abi1':
            inputs[str(destination)] = digest
    tools = {name: getattr(args, name).resolve() for name in ['seed', 'clang', 'formatter']}
    tools['python'] = Path(sys.executable).resolve()
    for name in ['lld-link', 'llvm-readobj']:
        tools[name] = tools['clang'].parent / (name + '.exe')
    lock = json.loads((ROOT / 'selfhost/bootstrap.lock').read_text('utf8'))
    for name in ['seed', 'clang', 'formatter']:
        assert sha(tools[name]) == lock[name]['sha256']
    def overridden(key):
        key = key.upper()
        return key.startswith(('RUSTC', 'RUSTDOC', 'RUSTUP_TOOLCHAIN', 'CARGO_BUILD_RUSTC', 'CARGO_BUILD_RUSTDOC', 'CARGO_PROFILE_', 'SENGOO_', 'SGPLATFORM_', 'SDL2_', 'PYTHON', 'LLDB_')) or key in {'CARGO', 'RUSTFLAGS', 'RUSTDOCFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CARGO_ENCODED_RUSTDOCFLAGS', 'CARGO_BUILD_RUSTFLAGS', 'CARGO_BUILD_RUSTDOCFLAGS', 'CARGO_BUILD_TARGET', 'CARGO_TARGET_DIR', 'CARGO_INCREMENTAL', 'CL', '_CL_', 'LINK', '_LINK_'} or key.startswith('CARGO_TARGET_') and key.endswith(('_RUSTFLAGS', '_RUSTDOCFLAGS', '_RUSTC'))
    env = {key: value for key, value in os.environ.items() if not overridden(key)}
    env['PATH'] = str(tools['clang'].parent) + os.pathsep + env.get('PATH', '')
    tools['rustup'] = Path(shutil.which('rustup', path=env['PATH'])).resolve()
    for name in ['cargo', 'rustc']:
        tools[name] = Path(subprocess.check_output([tools['rustup'], 'which', name], env=env, text=True).strip()).resolve()
        assert sha(tools[name]) == lock[name + '_sha256']
    assert tools['cargo'].parent == tools['rustc'].parent
    env.update(CARGO=str(tools['cargo']), RUSTC=str(tools['rustc']), CARGO_BUILD_RUSTC=str(tools['rustc']), RUSTUP_TOOLCHAIN=tools['rustc'].parent.parent.name, RUSTC_WRAPPER='', RUSTC_WORKSPACE_WRAPPER='', CARGO_BUILD_RUSTC_WRAPPER='', CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER='')
    env['PATH'] = str(tools['cargo'].parent) + os.pathsep + env['PATH']
    env.update(SENGOO_STDLIB=str(captured / 'tools/stdlib'), SENGOO_RUNTIME=str(captured / 'tools/stdlib/runtime.c'), SENGOO_CLANG=str(tools['clang']), SENGOO_MODULE_MAP=';'.join('selfhost_' + path.stem + '=' + str(captured / path.relative_to(ROOT)) for path in original if path.parent == ROOT / 'selfhost/src'))
    pure = {key: value for key, value in env.items() if not key.upper().startswith('SENGOO_')}
    pure['PATH'] = ''
    configs = {folder / '.cargo' / name for folder in [ROOT, *ROOT.parents] for name in ['config', 'config.toml']}
    configs.update(Path(os.environ.get('CARGO_HOME', str(Path.home() / '.cargo'))) / name for name in ['config', 'config.toml'])
    config_pins = {str(path): sha(path) if path.is_file() else None for path in configs}
    for path in configs:
        if path.is_file():
            assert 'include' not in tomllib.loads(path.read_text('utf8'))
    report = dict(schema=1, stage='collections-namespace-two-provider-mapping', result='running',
                  source_revision=revision(), host=platform.platform(), started_at=dt.datetime.now(dt.timezone.utc).isoformat(),
                  sources=inputs, qualified_checkout_sources={str(path): digest for path, digest in original.items()},
                  tools={name: dict(path=str(path), sha256=sha(path)) for name, path in tools.items()},
                  cargo_config_candidates=config_pins, steps=[], artifacts={}, c_header_closures={},
                  limits=['six development C exports and physical SG declaration mapping only',
                          'no origin/loan/ownership/constructor/optional-borrow lowering permission',
                          'C runtime algorithms and external C verification linker remain; no M3/M4/M6/M7 completion'])
    def save():
        (base / 'evidence.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf8')
    def run(name, command, expected=0, empty=False):
        done = subprocess.run(list(map(str, command)), cwd=ROOT, env=pure if empty else env, capture_output=True, timeout=900)
        stdout, stderr = base / (name + '.stdout.log'), base / (name + '.stderr.log')
        stdout.write_bytes(done.stdout); stderr.write_bytes(done.stderr)
        report['steps'].append(dict(name=name, command=list(map(str, command)), exit_code=done.returncode,
                                    expected_exit_code=expected, empty_path=empty, stdout_sha256=sha(stdout), stderr_sha256=sha(stderr)))
        save(); print(name, done.returncode, flush=True)
        assert done.returncode == expected, (name, done.returncode, done.stdout[-1200:], done.stderr[-3000:])
        return done
    try:
        probe = base / 'cargo-probe'; (probe / 'src').mkdir(parents=True)
        (probe / 'Cargo.toml').write_text('[package]\nname="collection_provider_rustc_probe"\nversion="0.0.0"\nedition="2021"\n[workspace]\n')
        (probe / 'src/lib.rs').write_text('pub fn answer()->i64{42}\n')
        (probe / 'build.rs').write_text('fn main(){println!("cargo:warning=provider_rustc={}",std::env::var("RUSTC").unwrap());}\n')
        observed = run('cargo-actual-rustc', [tools['cargo'], 'build', '--offline', '--verbose', '--manifest-path', probe / 'Cargo.toml', '--target-dir', probe / 'target'])
        output = (observed.stdout + observed.stderr).decode('utf8')
        actual_runs = [line for line in output.splitlines() if line.lstrip().startswith('Running ') and str(tools['rustc']).casefold() in line.casefold()]
        assert len(actual_runs) >= 2 and 'provider_rustc=' + str(tools['rustc']) in output
        report['cargo_actual_rustc_invocations'] = actual_runs
        for name in ['selfhost/src/collection_provider.sg', 'selfhost/tests/collection_provider.sg', 'selfhost/sdk/collections_x64_abi1/prelude.sg']:
            run('format-' + Path(name).parent.name, [tools['formatter'], '--check', captured / name])
        objects = []
        artifact_dir = captured / 'target/provider'; artifact_dir.mkdir(parents=True)
        for name in ['runtime', 'runtime_string', 'runtime_collections']:
            source = captured / 'tools/stdlib' / (name + '.c')
            dep_before, dep_after = base / (name + '-before.d'), base / (name + '-after.d')
            flags = ['-O0', '-Werror=date-time']
            run(name + '-headers', [tools['clang'], *flags, '-M', '-MT', 'provider_dependency', '-MF', dep_before, source])
            selected_headers = headers(dep_before); inputs.update(selected_headers)
            obj = artifact_dir / (name + '.obj')
            run(name + '-compile', [tools['clang'], *flags, '-MD', '-MT', 'provider_dependency', '-MF', dep_after, '-c', source, '-o', obj])
            assert selected_headers == headers(dep_after)
            report['c_header_closures'][name] = selected_headers
            objects.append(obj); report['artifacts'][str(obj.relative_to(base))] = sha(obj)
        generator = captured / 'selfhost/sdk/update_collections_inventory.py'
        run('provider-inventory', [tools['python'], '-B', '-X', 'utf8', generator, '--root', captured, '--artifact', objects[-1]])
        manifest = captured / 'selfhost/sdk/collections_x64_abi1/manifest.json'
        inputs[str(manifest)] = sha(manifest)
        run('provider-coff', [tools['llvm-readobj'], '--file-headers', '--sections', '--symbols', '--relocations', '--unwind', objects[-1]])
        cprobe = captured / 'selfhost/tests/collection_provider_probe.c'
        probe_before, probe_after = base / 'probe-before.d', base / 'probe-after.d'
        probe_flags = ['-std=c11', '-Wall', '-Wextra', '-Werror', '-Werror=date-time', '-O0', '-I', captured / 'tools/stdlib']
        run('c-probe-headers', [tools['clang'], *probe_flags, '-M', '-MT', 'provider_dependency', '-MF', probe_before, cprobe])
        probe_headers = headers(probe_before); inputs.update(probe_headers)
        probe_object = base / 'c-provider-probe.obj'
        run('c-probe-compile', [tools['clang'], *probe_flags, '-MD', '-MT', 'provider_dependency', '-MF', probe_after, '-c', cprobe, '-o', probe_object])
        assert probe_headers == headers(probe_after)
        report['c_header_closures']['c_probe'] = probe_headers
        report['artifacts'][str(probe_object.relative_to(base))] = sha(probe_object)
        executable = base / 'c-provider.exe'
        link = [tools['clang'], '-fuse-ld=lld', probe_object, *objects, '-Wl,/verbose', '-o', executable]
        driver = run('selected-c-linker', [*link, '-###'])
        selected = re.findall(r'^\s*"([^"\r\n]*[\\/]lld-link(?:\.exe)?)"', driver.stderr.decode('utf8'), re.M | re.I)
        assert len(selected) == 1
        selected_path = Path(selected[0].replace('\\\\', '\\'))
        if not selected_path.suffix:
            selected_path = selected_path.with_suffix('.exe')
        assert selected_path.resolve() == tools['lld-link'] and sha(selected_path) == report['tools']['lld-link']['sha256']
        report['selected_c_linker'] = dict(path=str(selected_path.resolve()), sha256=sha(selected_path))
        linked = run('c-provider-link', link)
        libraries = {}
        for match in re.finditer(rb'Reading ([^\r\n]+?\.lib)(?:\(|\r?$)', linked.stdout + linked.stderr, re.M):
            path = Path(match[1].decode('utf8')).resolve()
            if path.is_file():
                libraries[str(path)] = sha(path)
        assert libraries
        inputs.update(libraries); report['c_link_libraries'] = libraries
        cresult = run('c-provider-execute', [executable], 42, True)
        assert not cresult.stdout and not cresult.stderr
        report['artifacts'][str(executable.relative_to(base))] = sha(executable)
        fixture = captured / 'program.sg'
        fixture.write_text('import std::collections;\nimport collection_runtime;\ndef main() -> i64 { 42; }\n', encoding='utf8')
        inputs[str(fixture)] = sha(fixture)
        binary = base / 'provider-tests.exe'
        run('sg-build', [tools['seed'], '--runtime-mode', 'source-development', 'build', captured / 'selfhost/tests/collection_provider.sg', '--force-rebuild', '-O', '0', '-o', binary])
        report['artifacts'][str(binary.relative_to(base))] = sha(binary)
        result = run('sg-provider-mapping-resources', [binary, captured, sha(manifest), sha(objects[-1]), fixture], 42, True)
        assert result.stdout.strip() == b'collection provider: six physical program bindings and five exact resource scopes passed' and not result.stderr
        report.update(result='passed', provider_manifest_sha256=sha(manifest), provider_object_sha256=sha(objects[-1]), first_call_resource_scopes=5)
    except BaseException as error:
        report.update(result='failed', error=repr(error))
        raise
    finally:
        report.update(changed_sources=[name for name, digest in inputs.items() if not Path(name).is_file() or sha(name) != digest],
                      checkout_changes=[str(path) for path, digest in original.items() if not path.is_file() or sha(path) != digest],
                      changed_tools=[name for name, item in report['tools'].items() if not Path(item['path']).is_file() or sha(item['path']) != item['sha256']],
                      changed_configs=[name for name, digest in config_pins.items() if (sha(name) if Path(name).is_file() else None) != digest],
                      changed_artifacts=[name for name, digest in report['artifacts'].items() if not (base / name).is_file() or sha(base / name) != digest],
                      source_revision_end=revision(), completed_at=dt.datetime.now(dt.timezone.utc).isoformat())
        if any(report[name] for name in ['changed_sources', 'checkout_changes', 'changed_tools', 'changed_configs', 'changed_artifacts']):
            report.update(result='failed', error='qualified inputs or artifacts changed')
        save()
    return 0 if report['result'] == 'passed' else 1


if __name__ == '__main__':
    raise SystemExit(main())
