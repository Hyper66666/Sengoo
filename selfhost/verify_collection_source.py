"""Qualify the bounded fresh-source Vec constructor entry and ordinary lowering regressions."""
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
import time
import tomllib

from verify_collection_provider import ROOT, closure, headers, sha


def revision():
    return subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()


def rust_override(key):
    key = key.upper()
    return key.startswith(('RUSTC', 'RUSTDOC', 'RUSTUP_TOOLCHAIN', 'CARGO_BUILD_RUSTC', 'CARGO_BUILD_RUSTDOC', 'CARGO_PROFILE_')) or key in {'CARGO', 'RUSTFLAGS', 'RUSTDOCFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CARGO_ENCODED_RUSTDOCFLAGS', 'CARGO_BUILD_RUSTFLAGS', 'CARGO_BUILD_RUSTDOCFLAGS', 'CARGO_BUILD_TARGET', 'CARGO_TARGET_DIR', 'CARGO_INCREMENTAL'} or key.startswith('CARGO_TARGET_') and key.endswith(('_RUSTFLAGS', '_RUSTDOCFLAGS', '_RUSTC'))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['seed', 'clang', 'formatter', 'reference', 'output']:
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--skip-regressions', action='store_true', help='Development only; cannot produce a qualified result')
    parser.add_argument('--regression-parent', type=Path)
    parser.add_argument('--regression-parent-pin')
    parser.add_argument('--profile', choices=['constructor', 'flow'], default='constructor')
    args = parser.parse_args()
    base = args.output.resolve()
    assert base.is_relative_to(ROOT / 'target')
    base.mkdir(parents=True, exist_ok=False)
    report = dict(schema=1, stage='fresh-source-collection-constructor', result='running',
                  source_revision=revision(), host=platform.platform(), started_at=dt.datetime.now(dt.timezone.utc).isoformat(),
                  m3_complete=False, m4_complete=False, runtime_replaced=False, steps=[], artifacts={},
                  limits=['Concrete parameterless canonical constructor functions and their concrete owned-field Drop jobs only',
                          'Nonzero T up to one MiB, alignment at most eight; no source optional-borrow permission',
                          'Actual independently qualified development C collection algorithms; no compiler fixed point'])
    sources, origins, snapshots, tools, configs = {}, {}, {}, {}, {}

    def save():
        (base / 'evidence.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf8')

    save()
    try:
        flow = args.profile == 'flow'
        test_name = 'collection_flow' if flow else 'collection_source'
        fixture_name = 'collection_flow_source' if flow else 'collection_source_fixture'
        selected_name = 'source_some_i64' if flow else 'source_i64'
        if flow:
            report.update(stage='fresh-source-scalar-collection-flow',
                          limits=['Parameterless concrete SG roots returning i64; Vec elements i64/bool only',
                                  'Actual source/provider/loan/effect/ownership proofs and strict optional physical MIR; owning HIR6',
                                  'Saved MIR is not accepted; full owned new-authority wire and final runtime/self-build remain open'],
                          ordinary_regressions_separately_required=True)
        parent_path = ROOT / 'target/selfhost/m3-collection-provider-ast-checkpoint/evidence.json'
        assert sha(parent_path) == 'a63152e01208dff39e1d5cbcbd326187901808c8f76fa0d3075a08e0a9fa1cd3'
        parent = json.loads(parent_path.read_bytes())
        assert parent['result'] == 'passed'
        assert all(parent[k] == [] for k in ['changed_sources', 'checkout_changes', 'changed_tools', 'changed_configs', 'changed_artifacts'])
        sources[str(parent_path)] = sha(parent_path)
        for path, digest in parent['sources'].items():
            assert sha(path) == digest, path
            sources[path] = digest
        for relative, digest in parent['artifacts'].items():
            path = parent_path.parent / relative
            assert sha(path) == digest, path
            sources[str(path)] = digest
        for item in parent['tools'].values():
            assert sha(item['path']) == item['sha256']
            sources[item['path']] = item['sha256']
        for row in parent['steps']:
            assert row['exit_code'] == row.get('expected_exit_code', 0)
            for stream in ['stdout', 'stderr']:
                path = parent_path.parent / (row['name'] + '.' + stream + '.log')
                assert sha(path) == row[stream + '_sha256']
                sources[str(path)] = sha(path)
        provider_root = parent_path.parent / 'implementation'
        report['provider'] = dict(report=str(parent_path), sha256=sha(parent_path), root=str(provider_root),
                                  manifest_sha256=parent['provider_manifest_sha256'], object_sha256=parent['provider_object_sha256'])
        entries = ['selfhost/tests/' + test_name + '.sg', 'selfhost/tests/borrow.sg', 'selfhost/tests/mir.sg',
                   'selfhost/tests/handle_disarm_hir.sg', 'selfhost/tools/native.sg', 'selfhost/tools/semantic.sg']
        if flow:
            entries.append('selfhost/tests/collection_cleanup.sg')
        imported = {}
        for entry in entries:
            imported.update(closure(ROOT / entry))
        paths = set(imported)
        paths.update(ROOT / name for name in [
            'selfhost/verify_collection_source.py', 'selfhost/verify_collection_provider.py', 'selfhost/verify_mir.py',
            'selfhost/verify_projection.py', 'selfhost/verify_handle_disarm.py', 'selfhost/source_codegen_gate.py',
            'selfhost/tests/collection_source_fixture.sg', 'selfhost/tests/collection_source_shapes.sg', 'selfhost/tests/collection_source_probe.c',
            'selfhost/tests/projection_probe.c', 'selfhost/tests/handle_disarm.sg', 'selfhost/tests/handle_disarm_probe.c',
            'selfhost/tests/native_source_probe.c', 'selfhost/REFERENCE_GAPS.md', 'selfhost/bootstrap.lock',
            'selfhost/reference-baseline.lock', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
            'openspec/changes/add-selfhost-collection-intrinsics/source-constructor-checkpoint.md',
            'openspec/changes/add-selfhost-collection-intrinsics/specs/selfhost-source-collection-constructor/spec.md'])
        if flow:
            paths.update(ROOT / name for name in ['selfhost/tests/collection_flow_source.sg', 'selfhost/tests/collection_flow_probe.c',
                'openspec/changes/add-selfhost-collection-intrinsics/source-flow-checkpoint.md',
                'openspec/changes/add-selfhost-collection-intrinsics/specs/selfhost-source-collection-flow/spec.md'])
        for directory in ['tools/stdlib', 'runtime']:
            paths.update(p for p in (ROOT / directory).rglob('*') if p.is_file() and not set(p.relative_to(ROOT / directory).parts) & {'target', 'build', '__pycache__'})
        paths.update(p for p in (ROOT / 'openspec/changes/add-selfhost-handle-disarm').rglob('*.md') if p.name != 'tasks.md')
        captured = base / 'implementation'
        for path in sorted(paths):
            digest = sha(path)
            raw = path.read_bytes()
            assert sha(path) == digest
            copied = captured / path.relative_to(ROOT)
            copied.parent.mkdir(parents=True, exist_ok=True)
            copied.write_bytes(raw)
            origins[str(path)] = digest
            snapshots[str(copied)] = digest
        assert all(sha(path) == digest for path, digest in origins.items())
        sources.update(origins)
        sources.update(snapshots)
        report.update(sources=sources, producer_origins=origins, producer_snapshots=snapshots)
        lock = json.loads((captured / 'selfhost/bootstrap.lock').read_bytes())
        reference_lock = json.loads((captured / 'selfhost/reference-baseline.lock').read_bytes())
        tools = {name: dict(path=str(getattr(args, name).resolve()), sha256=sha(getattr(args, name))) for name in ['seed', 'clang', 'formatter', 'reference']}
        for name in ['seed', 'clang', 'formatter']:
            assert tools[name]['sha256'] == lock[name]['sha256']
        assert tools['reference']['sha256'] == reference_lock['tools']['sgc']['sha256']
        tools['python'] = dict(path=sys.executable, sha256=sha(sys.executable))
        for name in ['lld-link', 'llvm-readobj', 'llvm-objdump']:
            path = args.clang.resolve().parent / (name + '.exe')
            tools[name] = dict(path=str(path), sha256=sha(path))
        env = {k: v for k, v in os.environ.items() if not k.upper().startswith(('SENGOO_', 'SGPLATFORM_', 'SDL2_', 'PYTHON', 'LLDB_')) and not rust_override(k) and k.upper() not in {'CL', '_CL_', 'LINK', '_LINK_'}}
        env['PATH'] = str(args.clang.resolve().parent) + os.pathsep + env.get('PATH', '')
        rustup = Path(shutil.which('rustup', path=env['PATH'])).resolve()
        tools['rustup'] = dict(path=str(rustup), sha256=sha(rustup))
        for name in ['rustc', 'cargo']:
            path = Path(subprocess.check_output([rustup, 'which', name], env=env, text=True).strip()).resolve()
            assert sha(path) == lock[name + '_sha256']
            tools[name] = dict(path=str(path), sha256=sha(path))
        env.update(CARGO=tools['cargo']['path'], RUSTC=tools['rustc']['path'], CARGO_BUILD_RUSTC=tools['rustc']['path'],
                   RUSTUP_TOOLCHAIN=Path(tools['rustc']['path']).parent.parent.name, RUSTC_WRAPPER='', RUSTC_WORKSPACE_WRAPPER='', CARGO_BUILD_RUSTC_WRAPPER='', CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER='',
                   SENGOO_STDLIB=str(captured / 'tools/stdlib'), SENGOO_RUNTIME=str(captured / 'tools/stdlib/runtime.c'), SENGOO_CLANG=str(args.clang.resolve()),
                   SENGOO_MODULE_MAP=';'.join('selfhost_' + p.stem + '=' + str(captured / p.relative_to(ROOT)) for p in imported if p.parent == ROOT / 'selfhost/src'))
        env['PATH'] = str(Path(tools['cargo']['path']).parent) + os.pathsep + env['PATH']
        pure = {k: v for k, v in env.items() if not k.upper().startswith('SENGOO_')}
        pure['PATH'] = ''
        candidates = {folder / '.cargo' / name for folder in {ROOT, *ROOT.parents, captured, *captured.parents} for name in ['config', 'config.toml']}
        candidates.update(Path(os.environ.get('CARGO_HOME', str(Path.home() / '.cargo'))) / name for name in ['config', 'config.toml'])
        for path in candidates:
            configs[str(path)] = sha(path) if path.is_file() else None
            if path.is_file():
                assert 'include' not in tomllib.loads(path.read_text('utf8'))
        report.update(tools=tools, cargo_config_candidates=configs)

        def run(name, command, expected=0, empty=False, cwd=ROOT, timeout=900):
            started = time.monotonic()
            stdout, stderr = base / (name + '.stdout.log'), base / (name + '.stderr.log')
            with stdout.open('wb') as output, stderr.open('wb') as errors:
                done = subprocess.run(list(map(str, command)), cwd=cwd, env=pure if empty else env, stdout=output, stderr=errors, timeout=timeout)
            report['steps'].append(dict(name=name, command=list(map(str, command)), cwd=str(cwd), exit_code=done.returncode, expected_exit_code=expected,
                                        elapsed_s=time.monotonic() - started, empty_path=empty, stdout_sha256=sha(stdout), stderr_sha256=sha(stderr)))
            save()
            print(name, done.returncode, flush=True)
            assert done.returncode == expected, (name, stdout.read_bytes()[-1200:], stderr.read_bytes()[-3000:])
            return subprocess.CompletedProcess(command, done.returncode, stdout.read_bytes(), stderr.read_bytes())

        probe = base / 'cargo-probe'
        (probe / 'src').mkdir(parents=True)
        (probe / 'Cargo.toml').write_text('[package]\nname="source_constructor_rustc_probe"\nversion="0.0.0"\nedition="2021"\n[workspace]\n')
        (probe / 'src/lib.rs').write_text('pub fn answer()->i64{42}\n')
        (probe / 'build.rs').write_text('fn main(){println!("cargo:warning=source_constructor_rustc={}",std::env::var("RUSTC").unwrap());}\n')
        for path in [probe / 'Cargo.toml', probe / 'src/lib.rs', probe / 'build.rs']:
            sources[str(path)] = sha(path)
        actual = run('cargo-actual-rustc', [tools['cargo']['path'], 'build', '--offline', '--verbose', '--manifest-path', probe / 'Cargo.toml', '--target-dir', probe / 'target'])
        cargo_output = (actual.stdout + actual.stderr).decode('utf8')
        invocations = [line for line in cargo_output.splitlines() if line.lstrip().startswith('Running ') and tools['rustc']['path'].casefold() in line.casefold()]
        assert len(invocations) >= 2 and 'source_constructor_rustc=' + tools['rustc']['path'] in cargo_output
        report['cargo_actual_rustc_invocations'] = invocations
        for name in [test_name + '.sg', fixture_name + '.sg']:
            run('format-' + name, [args.formatter.resolve(), '--check', captured / 'selfhost/tests' / name])
        run('format-source-module', [args.formatter.resolve(), '--check', captured / 'selfhost/src/collection_source.sg'])
        if flow:
            for name in ['collection_flow', 'collection_cleanup']:
                run('format-' + name, [args.formatter.resolve(), '--check', captured / 'selfhost/src' / (name + '.sg')])
            cleanup = base / 'cleanup.exe'
            run('cleanup-tests-build', [args.seed.resolve(), '--runtime-mode', 'source-development', 'build', captured / 'selfhost/tests/collection_cleanup.sg', '--force-rebuild', '-O', '0', '-o', cleanup])
            tested = run('cleanup-three-first-scopes', [cleanup], 42, True)
            assert not tested.stdout and not tested.stderr
            report['artifacts'][cleanup.name] = sha(cleanup)
        binary = base / 'constructor.exe'
        run('source-tests-build', [args.seed.resolve(), '--runtime-mode', 'source-development', 'build', captured / 'selfhost/tests' / (test_name + '.sg'), '--force-rebuild', '-O', '0', '-o', binary])
        report['artifacts']['constructor.exe'] = sha(binary)
        fixture = captured / 'selfhost/tests' / (fixture_name + '.sg')

        def invoke(name, mode, selected, obj):
            source = captured / 'selfhost/tests/collection_source_shapes.sg' if mode == 'shape-adt' else fixture
            result = run(name, [binary, provider_root, parent['provider_manifest_sha256'], parent['provider_object_sha256'], source, obj, mode, selected], 42, True)
            assert not result.stdout and not result.stderr, name

        objects = []
        for number in range(3):
            obj = base / ('source-' + str(number) + '.obj')
            invoke('first-success-' + str(number), 'positive', selected_name, obj)
            objects.append(obj)
            report['artifacts'][obj.name] = sha(obj)
        assert len({obj.read_bytes() for obj in objects}) == 1
        cases = [(mode, selected_name) for mode in ['installed', 'manifest', 'artifact', 'collection', 'option', 'bad-limit', 'bad-object-limit', 'empty-names', 'duplicate', 'nul-name']]
        if flow:
            cases += [('shape', name) for name in ['missing', 'source_pure', 'source_wrapper', 'negative_escape']]
            cases += [('expect6120', name) for name in ['negative_mutable', 'negative_invalidated', 'negative_fake', 'negative_cross_t', 'negative_family']]
            cases += [('expect6124', 'negative_unguarded'), ('expect6121', 'negative_unknown'), ('expect6101', 'negative_push_reuse')]
        else:
            cases += [('shape', name) for name in ['missing', 'source_manual', 'source_wrapper', 'source_parameter', 'source_generic', 'source_other', 'source_local', 'source_branch']]
            cases += [('empty-element', 'source_empty')]
            cases += [('shape-adt', name) for name in ['source_class', 'source_nested_class', 'source_array_class', 'source_enum']]
        for mode, selected in cases:
            obj = base / ('rejected-' + mode + '-' + selected + '.obj')
            invoke('reject-' + mode + '-' + selected, mode, selected, obj)
            assert not obj.exists()
        run('object-inspection', [tools['llvm-readobj']['path'], '--file-headers', '--sections', '--symbols', '--relocations', '--unwind', objects[0]])
        run('object-disassembly', [tools['llvm-objdump']['path'], '-d', objects[0]])
        csource = captured / 'selfhost/tests' / (test_name + '_probe.c')
        flags = ['-std=c11', '-Wall', '-Wextra', '-Werror', '-Werror=date-time', '-O0', '-I', captured / 'tools/stdlib']
        before, after = base / 'before.d', base / 'after.d'
        run('c-headers', [args.clang.resolve(), *flags, '-M', '-MT', 'provider_dependency', '-MF', before, csource])
        selected_headers = headers(before)
        sources.update(selected_headers)
        cobject = base / 'probe.obj'
        run('c-compile', [args.clang.resolve(), *flags, '-MD', '-MT', 'provider_dependency', '-MF', after, '-c', csource, '-o', cobject])
        assert selected_headers == headers(after)
        report['c_headers'] = selected_headers
        provider_objects = [provider_root / 'target/provider' / (name + '.obj') for name in ['runtime', 'runtime_string', 'runtime_collections']]
        executable = base / 'probe.exe'
        command = [args.clang.resolve(), '-fuse-ld=lld', cobject, objects[0], *provider_objects, '-Wl,/verbose', '-o', executable]
        selected = run('actual-linker', [*command, '-###'])
        linked_paths = re.findall(r'^\s*"([^"\r\n]*[\\/]lld-link(?:\.exe)?)"', selected.stderr.decode('utf8'), re.M | re.I)
        assert len(linked_paths) == 1
        actual_linker = Path(linked_paths[0].replace('\\\\', '\\'))
        if not actual_linker.suffix:
            actual_linker = actual_linker.with_suffix('.exe')
        assert actual_linker.resolve() == Path(tools['lld-link']['path'])
        linked = run('c-link', command)
        libraries = {}
        for match in re.finditer(rb'Reading ([^\r\n]+?\.lib)(?:\(|\r?$)', linked.stdout + linked.stderr, re.M):
            path = Path(match[1].decode('utf8')).resolve()
            if path.is_file():
                libraries[str(path)] = sha(path)
        assert libraries
        report['c_link_libraries'] = libraries
        sources.update(libraries)
        executed = run('c-thousand-owning-rounds', [executable], 42, True)
        assert not executed.stdout and not executed.stderr
        for path in [cobject, executable]:
            report['artifacts'][path.name] = sha(path)
        if args.regression_parent:
            assert not args.skip_regressions and args.regression_parent_pin
            previous_path = args.regression_parent.resolve()
            assert sha(previous_path) == args.regression_parent_pin
            previous = json.loads(previous_path.read_bytes())
            assert previous['result'] == 'passed' and set(previous['regressions']) == {'mir', 'projection', 'disarm', 'source-native'}
            assert all(not previous.get(k) for k in ['changed_sources', 'checkout_changes', 'changed_snapshots', 'changed_tools', 'changed_configs', 'changed_artifacts'])
            previous_root = previous_path.parent / 'implementation'
            for p, h in previous['producer_snapshots'].items():
                assert sha(p) == h, p
                sources[p] = h
            excluded = {ROOT / p for p in ['selfhost/verify_collection_source.py', 'selfhost/src/collection_source.sg',
                        'selfhost/tests/collection_source.sg', 'selfhost/tests/collection_source_fixture.sg',
                        'selfhost/tests/collection_source_shapes.sg', 'selfhost/tests/collection_source_probe.c',
                        'openspec/changes/add-selfhost-collection-intrinsics/source-constructor-checkpoint.md',
                        'openspec/changes/add-selfhost-collection-intrinsics/specs/selfhost-source-collection-constructor/spec.md']}
            unchanged = {}
            for p in sorted(paths - excluded):
                assert str(p) in previous['producer_origins'], p
                h = previous['producer_origins'][str(p)]
                saved = captured / p.relative_to(ROOT)
                assert sha(saved) == h == sha(previous_root / p.relative_to(ROOT)), p
                unchanged[p.relative_to(ROOT).as_posix()] = h
            for n, item in previous['tools'].items():
                assert n in tools and item['sha256'] == tools[n]['sha256'] and sha(item['path']) == item['sha256'], n
            report['regressions'] = previous['regressions']
            sources[str(previous_path)] = args.regression_parent_pin
            legacy_logs = {}
            for name, item in previous['regressions'].items():
                evidence = Path(item['report'])
                assert sha(evidence) == item['sha256']
                child = json.loads(evidence.read_bytes())
                assert child['result'].startswith('passed')
                for k, value in child.items():
                    if k.startswith(('source_changes', 'snapshot_changes', 'tool_changes', 'checkout_changes', 'changed_')):
                        assert not value, (name, k)
                for key in ['sources', 'inputs_before', 'producer_origins', 'producer_snapshots']:
                    for p, h in child.get(key, {}).items():
                        p = Path(p); p = p if p.is_absolute() else previous_root / p
                        assert sha(p) == h, p
                        sources[str(p)] = h
                for p, h in child.get('artifacts', {}).items():
                    p = Path(p); p = p if p.is_absolute() else evidence.parent / p
                    assert sha(p) == h, p
                    sources[str(p)] = h
                for step in child['steps']:
                    assert step.get('exit', step.get('exit_code')) == step.get('expected_exit', step.get('expected_exit_code', 0)), step['name']
                    if 'log_sha256' in step:
                        p = previous_root / step['log']; assert sha(p) == step['log_sha256']; sources[str(p)] = sha(p)
                    else:
                        for stream in ['stdout', 'stderr']:
                            p = evidence.parent / (step['name'] + '.' + stream + '.log')
                            if stream + '_sha256' in step:
                                assert sha(p) == step[stream + '_sha256']
                            else:
                                legacy_logs[str(p)] = sha(p)
                            sources[str(p)] = sha(p)
                sources[str(evidence)] = sha(evidence)
            report['regression_reuse'] = dict(report=str(previous_path), sha256=args.regression_parent_pin,
                                            unchanged_inputs=unchanged, observed_legacy_log_hashes=legacy_logs,
                                            scope='Ordinary compiler producer, harnesses, runtime and stdlib are byte-identical; only the separate fresh-source API/profile and its test/ledger changed.',
                                            receipt_limit='The nested source-native gate lacks per-stream producer hashes; its retained logs are observations after its terminal result.')
        elif not args.skip_regressions and not flow:
            report['regressions'] = {}
            for name, script, extra, wanted in [
                    ('mir', 'verify_mir.py', [], 'passed'),
                    ('projection', 'verify_projection.py', ['--reference', args.reference.resolve()], 'passed'),
                    ('disarm', 'verify_handle_disarm.py', ['--reference', args.reference.resolve()], 'passed-with-reference-zero-drop-policy-difference'),
                    ('source-native', 'source_codegen_gate.py', ['--reference', args.reference.resolve(), '--formatter', args.formatter.resolve()], 'passed')]:
                dest = captured / 'target' / name
                run('existing-' + name, [sys.executable, captured / 'selfhost' / script, '--seed', args.seed.resolve(), '--clang', args.clang.resolve(), '--output', dest, *extra], cwd=captured, timeout=3600)
                evidence = dest / 'evidence.json'
                child = json.loads(evidence.read_bytes())
                assert child['result'] == wanted
                for key, value in child.items():
                    if key.startswith(('source_changes', 'snapshot_changes', 'tool_changes', 'cargo_config_changes', 'checkout_changes', 'changed_')):
                        assert not value, (name, key)
                sources[str(evidence)] = sha(evidence)
                report['regressions'][name] = dict(report=str(evidence), sha256=sha(evidence), result=child['result'], commands=len(child['steps']))
        report.update(result='passed-fresh-scalar-flow-boundary' if flow and not args.skip_regressions else 'development-only' if args.skip_regressions else 'passed', first_call_resource_scopes=3 + len(cases), owning_drops=0 if flow else 50000,
                      raw_objects_byte_equal=True, rejected_source_cases=len(cases))
        if flow:
            report.update(machine_source_calls=7000, cleanup_template_first_scopes=3, source_functions=7,
                          ordinary_regressions_scope='Required separate ordinary-lower report must bind this exact lowerer/closure before the integration checkpoint is closed.')
    except BaseException as error:
        report.update(result='failed', failure=repr(error))
        raise
    finally:
        report.update(sources=sources, producer_origins=origins, producer_snapshots=snapshots, tools=tools,
                      changed_sources=[p for p, h in sources.items() if not Path(p).is_file() or sha(p) != h],
                      checkout_changes=[p for p, h in origins.items() if not Path(p).is_file() or sha(p) != h],
                      changed_snapshots=[p for p, h in snapshots.items() if not Path(p).is_file() or sha(p) != h],
                      changed_tools=[n for n, item in tools.items() if not Path(item['path']).is_file() or sha(item['path']) != item['sha256']],
                      changed_configs=[p for p, h in configs.items() if (sha(p) if Path(p).is_file() else None) != h],
                      changed_artifacts=[n for n, h in report['artifacts'].items() if not (base / n).is_file() or sha(base / n) != h],
                      source_revision_end=revision(), completed_at=dt.datetime.now(dt.timezone.utc).isoformat())
        if any(report[k] for k in ['changed_sources', 'checkout_changes', 'changed_snapshots', 'changed_tools', 'changed_configs', 'changed_artifacts']):
            report.update(result='failed', guard_failure='qualified input, tool or output changed')
        save()
    return 0 if report['result'] in ['passed', 'development-only', 'passed-fresh-scalar-flow-boundary'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
