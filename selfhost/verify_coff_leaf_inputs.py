"""Qualify real AMD64 leaf COFF, BSS and the existing SG linker/debug gates."""
from __future__ import annotations

import argparse
import ast
import concurrent.futures
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import struct
import subprocess
import sys
import threading
import time
import tomllib

from link_multi_debug_gate import closure, rust_override
from verify_flow_lower_regressions import dependencies

ROOT = Path(__file__).resolve().parents[1]


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def coff(raw):
    count = struct.unpack_from('<H', raw, 2)[0]
    first, total = struct.unpack_from('<II', raw, 8)
    strings = raw[first + total * 18:]

    def name(field, section=False):
        if section and field[:1] == b'/':
            return strings[int(field[1:].rstrip(b'\0')):].split(b'\0')[0].decode()
        if field[:4] == bytes(4):
            return strings[struct.unpack_from('<I', field, 4)[0]:].split(b'\0')[0].decode()
        return field.rstrip(b'\0').decode()

    sections = {name(raw[20 + 40 * i:28 + 40 * i], True): 20 + 40 * i for i in range(count)}
    symbols, i = {}, 0
    while i < total:
        at = first + i * 18
        symbols[name(raw[at:at + 8])] = (i, at)
        i += 1 + raw[at + 17]
    return sections, symbols, first, total


def mutations(raw):
    sections, symbols, first, total = coff(raw)
    bss, meta, text = [sections[n] for n in ('.bss', '.llvm_addrsig', '.text')]
    rel = struct.unpack_from('<I', raw, text + 24)[0]
    metadata = struct.unpack_from('<I', raw, meta + 20)[0]
    file_index, file_row = symbols['.file']
    result = []

    def change(label, at, value, width, code=7201):
        changed = bytearray(raw)
        changed[at:at + width] = value.to_bytes(width, 'little')
        result.append((label, bytes(changed), code))

    change('bss-raw', bss + 20, 1, 4, 7203)
    change('bss-relocs', bss + 32, 1, 2, 7203)
    change('bss-exec', bss + 36, struct.unpack_from('<I', raw, bss + 36)[0] | 0x20000000, 4, 7203)
    change('bss-too-large', bss + 16, 67108865, 4, 7202)
    change('aux-count', first + 17, 255, 1)
    change('aux-length', first + 18, 164, 4)
    change('aux-reloc-count', first + 22, 9, 2)
    change('aux-section-number', first + 30, 2, 2)
    change('aux-selection', first + 32, 2, 1, 7203)
    change('aux-reserved', first + 33, 1, 1)
    change('file-no-aux', file_row + 17, 0, 1, 7203)
    change('file-wrong-owner', file_row + 12, 65535, 2, 7203)
    change('relocation-to-aux', rel + 4, 1, 4)
    change('relocation-to-file', rel + 4, file_index, 4)
    change('relocation-to-metadata', rel + 4, symbols['.llvm_addrsig'][0], 4)
    change('relocation-outside-symbols', rel + 4, total, 4)
    change('addrsig-aux', metadata, 1, 1)
    change('addrsig-truncated', metadata, 0x808080, 3)
    change('addrsig-nonminimal', metadata, 0x00008e, 3)
    change('addrsig-runtime-flags', meta + 36, 0x40000800, 4, 7203)
    change('addrsig-relocation', meta + 32, 1, 2, 7203)
    change('addrsig-unknown-flags', meta + 36, 1050625, 4, 7203)
    change('addrsig-byte-cap', meta + 16, total * 3 + 1, 4, 7202)
    change('comdat-section', text + 36, struct.unpack_from('<I', raw, text + 36)[0] | 0x1000, 4, 7203)
    change('weak-symbol', symbols['main'][1] + 16, 105, 1, 7203)
    common = bytearray(raw)
    struct.pack_into('<IH', common, symbols['main'][1] + 8, 8, 0)
    result.append(('common-symbol', bytes(common), 7203))
    # The old prototype skipped relocation validation for discarded empty sections.
    change('empty-section-with-relocations', text + 16, 0, 4)
    return result


def image_facts(raw):
    nt = struct.unpack_from('<I', raw, 60)[0]
    assert raw[:2] == b'MZ' and raw[nt:nt + 4] == b'PE\0\0'
    optional = nt + 24
    count = struct.unpack_from('<H', raw, nt + 6)[0]
    rows = []
    for i in range(count):
        at = optional + 240 + i * 40
        size, rva, padded, position = struct.unpack_from('<IIII', raw, at + 8)
        rows.append(dict(name=raw[at:at + 8].rstrip(b'\0').decode(), virtual_size=size, rva=rva,
                         raw_size=padded, raw=position, flags=struct.unpack_from('<I', raw, at + 36)[0]))
        assert rva % 4096 == 0 and position % 512 == 0 and padded % 512 == 0
        assert position + padded <= len(raw)
    preferred = struct.unpack_from('<Q', raw, optional + 24)[0]
    data = next(row for row in rows if row['name'] == '.data')
    storage = struct.unpack_from('<Q', raw, data['raw'])[0] - preferred
    entry = next(row['rva'] for row in rows if row['name'] == '.text')
    return dict(sections=rows, preferred_base=preferred, bss_storage_rva=storage, main_rva=entry,
                uninitialized=struct.unpack_from('<I', raw, optional + 12)[0])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--focused-only', action='store_true')
    parser.add_argument('--seed', type=Path, default=ROOT / 'target/agent-conditional-compiler/bin/sgc.exe')
    parser.add_argument('--formatter', type=Path, default=ROOT / 'target/agent-formatter/debug/sgfmt.exe')
    parser.add_argument('--clang', type=Path, default=Path('C:/Users/admin/AppData/Local/SengooToolchain/clang+llvm-19.1.7-x86_64-pc-windows-msvc/bin/clang.exe'))
    parser.add_argument('--lldb-python-home', type=Path, default=ROOT / 'target/selfhost/m5-linker-dev/lldb-python310')
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to(ROOT / 'target')
    out.mkdir(parents=True, exist_ok=False)
    inputs, origins, snapshots, tools, configs, artifacts = {}, {}, {}, {}, {}, {}
    revision = lambda: subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    report = dict(schema=1, stage='source-qualified-coff-leaf-inputs', result='running', m5_complete=False,
                  source_revision=revision(), host=platform.platform(), started_at=dt.datetime.now(dt.timezone.utc).isoformat(),
                  sources=inputs, producer_origins=origins, producer_snapshots=snapshots, tools=tools,
                  cargo_config_candidates=configs, artifacts=artifacts, steps=[], regressions={},
                  limits=['bounded 128-object leaf profile; COMDAT/directives/CRT startup remain unsupported',
                          'BSS is temporarily materialized in the bounded parser arena; this is not an RSS/performance qualification',
                          'legacy child gates that use live source paths are guarded and archived without claiming they consumed the archive'])
    mutex = threading.Lock()

    def save():
        with mutex:
            temporary = out / 'evidence.partial.json'
            temporary.write_text(json.dumps(report, indent=2) + '\n', encoding='utf8')
            # Windows readers may briefly hold a handle without FILE_SHARE_DELETE.
            # Preserve the complete previous report until an atomic replace succeeds.
            for attempt in range(100):
                try:
                    temporary.replace(out / 'evidence.json')
                    break
                except PermissionError:
                    if attempt == 99:
                        raise
                    time.sleep(0.02)

    def bind(path, digest=None):
        path = Path(path).resolve()
        raw = path.read_bytes()
        actual = hashlib.sha256(raw).hexdigest()
        assert digest is None or digest == actual, str(path)
        inputs[str(path)] = actual
        return raw

    def run(name, command, code=0, pure=False, timeout=2400):
        start = time.monotonic()
        done = subprocess.run(list(map(str, command)), cwd=ROOT, env=empty if pure else env, capture_output=True, timeout=timeout)
        stdout, stderr = out / (name + '.stdout.log'), out / (name + '.stderr.log')
        stdout.write_bytes(done.stdout)
        stderr.write_bytes(done.stderr)
        report['steps'].append(dict(name=name, command=list(map(str, command)), exit_code=done.returncode,
                                    expected_exit_code=code, seconds=time.monotonic() - start, empty_path=pure,
                                    stdout_sha256=sha(stdout), stderr_sha256=sha(stderr)))
        save()
        print(name, done.returncode, flush=True)
        assert done.returncode == code, (name, done.stdout[-1000:], done.stderr[-2000:])
        return done

    save()
    try:
        lock = json.loads(bind(ROOT / 'selfhost/bootstrap.lock'))
        rustbin = Path.home() / '.rustup/toolchains' / (lock['rust_toolchain'] + '-x86_64-pc-windows-msvc') / 'bin'
        selected = dict(seed=args.seed.resolve(), formatter=args.formatter.resolve(), clang=args.clang.resolve(), python=Path(sys.executable),
                        cargo=rustbin / 'cargo.exe', rustc=rustbin / 'rustc.exe')
        selected.update({name: args.clang.resolve().parent / name for name in ['lld-link.exe', 'llvm-readobj.exe', 'llvm-lib.exe']})
        selected.update({name: Path(os.environ['SystemRoot']) / 'System32' / name for name in ['ntdll.dll', 'kernel32.dll', 'KernelBase.dll', 'dbghelp.dll', 'ucrtbase.dll']})
        for name, path in selected.items():
            expected = lock[name]['sha256'] if name in ('seed', 'formatter', 'clang') else lock[name + '_sha256'] if name in ('cargo', 'rustc') else None
            bind(path, expected)
            tools[name] = dict(path=str(path), sha256=sha(path))
        foundational = ROOT / 'target/selfhost/m5-linker-sdk-checked/evidence.json'
        foundation = json.loads(bind(foundational))
        sdk = Path(foundation['tools']['sdk-kernel32']['path'])
        bind(sdk, foundation['tools']['sdk-kernel32']['sha256'])
        tools['sdk-kernel32'] = dict(path=str(sdk), sha256=sha(sdk))
        paths = set()
        entries = ['linker.sg', 'coff_leaf_resources.sg', 'link_debug.sg', 'link_codeview.sg', 'link_native_debug.sg',
                   'link_native_debug_modules.sg', 'link_debug_units.sg', 'link_native_debug_units.sg', 'mixeddebug.sg', 'pdb_type_hashes.sg',
                   'link_debug_ranges.sg', 'link_cv_types.sg', 'link_cv_types_debug.sg', 'link_cv_aggregate.sg']
        for name in entries:
            paths.update(closure(ROOT / 'selfhost/tests' / name))
        paths.update(p for p in (ROOT / 'selfhost/tests').glob('link*') if p.suffix in ('.sg', '.c', '.cpp'))
        for folder in ['tools/stdlib', 'runtime']:
            paths.update(p for p in (ROOT / folder).rglob('*') if p.is_file() and not any(x in {'target', 'build', '__pycache__'} for x in p.relative_to(ROOT / folder).parts))
        for name in ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'selfhost/bootstrap.lock', 'selfhost/runtime/memory_read.c',
                     'selfhost/tests/coff_leaf_source.c', 'selfhost/tests/coff_leaf_memory_source.c', 'selfhost/tests/coff_leaf_rebase_probe.c']:
            paths.add(ROOT / name)
        pending = [Path(__file__).resolve()] + [ROOT / 'selfhost' / n for n in ['linker_gate.py', 'verify_pdb_type_hashes.py', 'link_codeview_gate.py', 'link_multi_debug_gate.py', 'mixeddebug_gate.py', 'link_units_gate.py', 'link_native_debug_gate.py']]
        while pending:
            path = pending.pop()
            if path in paths:
                continue
            paths.add(path)
            for node in ast.walk(ast.parse(path.read_bytes())):
                names = [node.module] if isinstance(node, ast.ImportFrom) and node.module else [v.name for v in node.names] if isinstance(node, ast.Import) else []
                pending.extend(ROOT / 'selfhost' / (name + '.py') for name in names if (ROOT / 'selfhost' / (name + '.py')).is_file())
        # Child scripts loaded by name are not Python imports.
        paths.update(ROOT / 'selfhost' / n for n in ['mixeddebug_gate.py', 'link_units_gate.py', 'link_native_debug_gate.py'])
        source = out / 'implementation'
        for path in sorted(paths):
            raw = bind(path)
            origins[str(path)] = sha(path)
            saved = source / path.relative_to(ROOT)
            saved.parent.mkdir(parents=True, exist_ok=True)
            saved.write_bytes(raw)
            snapshots[str(saved)] = sha(saved)
        assert all(sha(p) == h for p, h in origins.items())
        env = {k: v for k, v in os.environ.items() if not rust_override(k) and not k.upper().startswith(('SENGOO_', 'PYTHON', 'LLDB_', 'SGPLATFORM_', 'SDL2_')) and k.upper() not in {'CL', '_CL_', 'LINK', '_LINK_'}}
        env.update(SENGOO_STDLIB=str(source / 'tools/stdlib'), SENGOO_RUNTIME=str(source / 'tools/stdlib/runtime.c'), SENGOO_CLANG=str(args.clang.resolve()),
                   SENGOO_MODULE_MAP=';'.join('selfhost_' + p.stem + '=' + str(source / p.relative_to(ROOT)) for p in paths if p.parent == ROOT / 'selfhost/src'),
                   RUSTC=str(rustbin / 'rustc.exe'), CARGO=str(rustbin / 'cargo.exe'), CARGO_BUILD_RUSTC=str(rustbin / 'rustc.exe'),
                   RUSTUP_TOOLCHAIN=rustbin.parent.name, RUSTC_WRAPPER='', RUSTC_WORKSPACE_WRAPPER='', CARGO_BUILD_RUSTC_WRAPPER='', CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER='')
        env['PATH'] = str(rustbin) + os.pathsep + str(args.clang.resolve().parent) + os.pathsep + env.get('PATH', '')
        empty = {k: v for k, v in env.items() if not k.upper().startswith('SENGOO_')}
        empty['PATH'] = ''
        candidates = {p / '.cargo' / n for p in [ROOT, *ROOT.parents, source, *source.parents] for n in ['config', 'config.toml']}
        candidates.update(Path(env.get('CARGO_HOME', str(Path.home() / '.cargo'))) / n for n in ['config', 'config.toml'])
        for path in candidates:
            raw = path.read_bytes() if path.is_file() else None
            configs[str(path)] = hashlib.sha256(raw).hexdigest() if raw is not None else None
            if raw is not None:
                assert 'include' not in tomllib.loads(raw.decode('utf8'))
                bind(path)
        for name in ['seed', 'formatter', 'clang', 'cargo', 'rustc']:
            tools[name]['version'] = run(name + '-version', [selected[name], '-vV' if name == 'rustc' else '--version']).stdout.decode('utf8').strip()
        cargo = out / 'cargo-probe'
        (cargo / 'src').mkdir(parents=True)
        (cargo / 'Cargo.toml').write_text('[package]\nname="coff_leaf_rustc_probe"\nversion="0.0.0"\nedition="2021"\n[workspace]\n', encoding='utf8')
        (cargo / 'src/lib.rs').write_text('pub fn answer()->i64{42}\n', encoding='utf8')
        (cargo / 'build.rs').write_text('fn main(){println!("cargo:warning=coff_leaf_rustc={}",std::env::var("RUSTC").unwrap());}\n', encoding='utf8')
        observed = run('cargo-actual-compiler', [selected['cargo'], 'build', '--offline', '--verbose', '--manifest-path', cargo / 'Cargo.toml', '--target-dir', cargo / 'target'])
        diagnostic = (observed.stdout + observed.stderr).decode('utf8')
        assert 'coff_leaf_rustc=' + str(selected['rustc']) in diagnostic
        assert sum(line.lstrip().startswith('Running ') and str(selected['rustc']).casefold() in line.casefold() for line in diagnostic.splitlines()) >= 2
        for name in ['Cargo.toml', 'Cargo.lock', 'build.rs', 'src/lib.rs']:
            bind(cargo / name)
        for name in ['selfhost/src/link_coff.sg', 'selfhost/src/link_pe.sg', 'selfhost/tests/coff_leaf_resources.sg']:
            run('format-' + Path(name).stem, [selected['formatter'], '--check', source / name])
        binary, resources = out / 'sg-linker.exe', out / 'resources.exe'
        for name, entry, product in [('linker', 'linker.sg', binary), ('resources', 'coff_leaf_resources.sg', resources)]:
            run('build-' + name, [selected['seed'], '--runtime-mode', 'source-development', 'build', source / 'selfhost/tests' / entry, '--force-rebuild', '-O', '0', '-o', product])
            artifacts[product.name] = sha(product)
        fixtures = out / 'c-fixtures'
        fixtures.mkdir()
        leaf = source / 'selfhost/tests/coff_leaf_source.c'
        longfile, exact = fixtures / 'filename-longer-than-eighteen-bytes-actual.c', fixtures / 'abcdefghijklmnop.c'
        longfile.write_bytes(leaf.read_bytes())
        exact.write_bytes(leaf.read_bytes())
        jobs = [('leaf', leaf, []), ('noaddrsig', leaf, ['-fno-addrsig']), ('longfile', longfile, []), ('exact18', exact, []),
                ('memory-read', source / 'selfhost/runtime/memory_read.c', []), ('memory-consumer', source / 'selfhost/tests/coff_leaf_memory_source.c', []),
                ('rebase-probe', source / 'selfhost/tests/coff_leaf_rebase_probe.c', [])]
        jobs += [(Path(name).stem, source / 'selfhost/tests' / name, []) for name in ['linker_probe.c', 'link_debug_ranges_probe.c', 'link_cv_pointer_probe.cpp', 'link_cv_aggregate_probe.cpp']]
        headers = {}
        for name, csource, flags in jobs:
            bind(csource)
            before, after = out / (name + '-before.d'), out / (name + '-after.d')
            run(name + '-headers', [selected['clang'], '-M', '-MT', 'optional_dependency', '-MF', before, csource])
            captured = {str(p): sha(p) for p in dependencies(before)}
            for path, digest in captured.items():
                bind(path, digest)
            obj = out / (name + '.obj')
            run('compile-' + name, [selected['clang'], '-O0', '-Werror', *flags, '-MD', '-MT', 'optional_dependency', '-MF', after, '-c', csource, '-o', obj])
            assert captured == {str(p): sha(p) for p in dependencies(after)}
            headers[name] = captured
            artifacts[obj.name] = sha(obj)
        report['c_headers'] = headers
        selection = run('rebase-link-selection', [selected['clang'], '-fuse-ld=lld', '-###', out / 'rebase-probe.obj', '-o', out / 'rebase-probe.exe'])
        trace = (selection.stdout + selection.stderr).decode('utf8')
        paths = [p.replace('\\\\', '\\') for p in re.findall(r'"([^"\r\n]+)"', trace)]
        linkers = [Path(p if p.lower().endswith('.exe') else p + '.exe') for p in paths if p.lower().endswith(('lld-link.exe', 'lld-link'))]
        assert len(linkers) == 1 and sha(linkers[0]) == tools['lld-link.exe']['sha256']
        report['actual_driver_linker'] = dict(path=str(linkers[0]), sha256=sha(linkers[0]))
        library_dirs = [Path(p.split(':', 1)[1]) for p in paths if p.lower().startswith('-libpath:')]
        assert library_dirs
        library_manifest = {str(p): sha(p) for folder in library_dirs for p in folder.glob('*.lib')}
        for path, digest in library_manifest.items():
            bind(path, digest)
        report['conservative_link_library_manifest'] = library_manifest
        system = run('legacy-system-link-selection', [selected['clang'], '-###', out / 'linker_probe.obj', '-o', out / 'legacy-selection.exe'])
        system_words = [p.replace('\\\\', '\\') for p in re.findall(r'"([^"\r\n]+)"', (system.stdout + system.stderr).decode('utf8'))]
        system_linkers = [Path(p) for p in system_words if p.lower().endswith('link.exe') and Path(p).is_file()]
        assert len(system_linkers) == 1
        bind(system_linkers[0])
        tools['legacy-system-linker'] = dict(path=str(system_linkers[0]), sha256=sha(system_linkers[0]))
        for word in system_words:
            if word.lower().startswith('-libpath:'):
                for path in Path(word.split(':', 1)[1]).glob('*.lib'):
                    bind(path)
                    library_manifest[str(path)] = sha(path)
        run('build-rebase-probe', [selected['clang'], '-fuse-ld=lld', out / 'rebase-probe.obj', '-Wl,/verbose', '-o', out / 'rebase-probe.exe'])
        artifacts['rebase-probe.exe'] = sha(out / 'rebase-probe.exe')
        positive = []
        for label, objects in [('leaf', ['leaf']), ('noaddrsig', ['noaddrsig']), ('longfile', ['longfile']), ('exact18', ['exact18']), ('actual-memory-read', ['memory-consumer', 'memory-read'])]:
            objects = [out / (n + '.obj') for n in objects]
            outputs = []
            for fresh in ['a', 'b']:
                image = out / (label + '-' + fresh + '.exe')
                run(label + '-sg-link-' + fresh, [binary, 'link', image, 'main', *objects, sdk], pure=True)
                run(label + '-sg-run-' + fresh, [image], 42, pure=True)
                artifacts[image.name] = sha(image)
                outputs.append(image.read_bytes())
            assert outputs[0] == outputs[1]
            independent = out / (label + '-lld.exe')
            run(label + '-lld-link', [selected['lld-link.exe'], '/entry:main', '/subsystem:console', '/nodefaultlib', '/out:' + str(independent), *objects])
            run(label + '-lld-run', [independent], 42, pure=True)
            artifacts[independent.name] = sha(independent)
            positive.append(dict(name=label, sg_exit=42, lld_exit=42, two_fresh_pe_bytes_equal=True))
        report['positives'] = positive
        for label in ['leaf-a', 'leaf-lld']:
            image = out / (label + '.exe')
            facts = image_facts(image.read_bytes())
            if label == 'leaf-a':
                bss = next(row for row in facts['sections'] if row['name'] == '.bss')
                assert bss['virtual_size'] == 8193 and bss['raw_size'] == 0 and bss['raw'] == 0
                assert bss['rva'] == facts['bss_storage_rva'] and facts['uninitialized'] == 8193
                assert not any(row['name'] == '.llvm_addrsig' for row in facts['sections'])
            done = run('actual-rebase-' + label, [out / 'rebase-probe.exe', image, hex(facts['main_rva']), hex(facts['bss_storage_rva'])])
            assert done.stdout.strip() == b'actual loader rebased BSS zero/write/address: ok' and not done.stderr
            report.setdefault('actual_loader_rebase', {})[label] = facts
        negative = []
        resource_args = [out / 'leaf.obj', sdk]
        for label, raw, code in mutations((out / 'leaf.obj').read_bytes()):
            obj, image = out / (label + '.obj'), out / 'rejected.exe'
            obj.write_bytes(raw)
            artifacts[obj.name] = sha(obj)
            image.write_bytes(b'previous-output')
            done = run('reject-' + label, [binary, 'link', image, 'main', obj, sdk], 65, pure=True)
            assert done.stdout.strip() == str(code).encode() and image.read_bytes() == b'previous-output'
            resource_args.extend([obj, 'M' if code == 7201 else 'C' if code == 7202 else 'U'])
            negative.append(dict(name=label, code=code, previous_output_preserved=True))
        done = run('five-first-resource-scopes', [resources, *resource_args], pure=True)
        assert done.stdout.strip() == b'leaf profile: five first-call resource scopes passed' and not done.stderr
        report.update(negative_cases=negative, first_call_resource_scopes=5)
        # Actual producer forms, not only byte-poisoned stand-ins.
        for name, content, flags in [('directives', '#pragma comment(lib,"kernel32.lib")\nint main(void){return 42;}\n', []),
                                     ('common', 'int state; int main(void){return state+42;}\n', ['-fcommon']),
                                     ('comdat', '__declspec(selectany) int state=42; int main(void){return state;}\n', [])]:
            csource, obj, image = fixtures / (name + '.c'), out / (name + '.obj'), out / 'rejected.exe'
            csource.write_text(content, encoding='utf8')
            bind(csource)
            run('compile-actual-' + name, [selected['clang'], '-O0', '-Werror', *flags, '-c', csource, '-o', obj])
            artifacts[obj.name] = sha(obj)
            image.write_bytes(b'previous-output')
            done = run('reject-actual-' + name, [binary, 'link', image, 'main', obj, sdk], 65, pure=True)
            assert done.stdout.strip() == b'7203' and image.read_bytes() == b'previous-output'
        if not args.focused_only:
            common = ['--seed', args.seed.resolve(), '--clang', args.clang.resolve(), '--formatter', args.formatter.resolve()]

            def child(name, script, extra):
                dest = out / name
                run('regression-' + name, [sys.executable, '-B', '-X', 'utf8', ROOT / 'selfhost' / script, *common, '--output', dest, *extra], timeout=7200)
                evidence = dest / 'evidence.json'
                data = json.loads(bind(evidence))
                assert data['result'].startswith('passed')
                for key, value in data.items():
                    if key.startswith('changed_') or key.endswith('_changes') or key == 'checkout_changes':
                        assert value == [], (name, key, value)
                report['regressions'][name] = dict(report=str(evidence), sha256=sha(evidence), result=data['result'], steps=len(data['steps']))
                save()
            debug = ['--lldb-python-home', args.lldb_python_home.resolve()]
            with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
                futures = [pool.submit(child, 'linker-archive-sdk', 'linker_gate.py', ['--sdk-kernel32', sdk]),
                           pool.submit(child, 'pdb-all', 'verify_pdb_type_hashes.py', debug),
                           pool.submit(child, 'codeview', 'link_codeview_gate.py', [*debug, '--ranges', '--primitives', '--references', '--aggregates'])]
                for future in concurrent.futures.as_completed(futures):
                    future.result()
            child('multi-source', 'link_multi_debug_gate.py', debug)
            raw_sdk = out / 'pdb-all/mixed/sdk/historical.raw.obj'
            raw = bind(raw_sdk)
            _, symbols, _, _ = coff(raw)
            actual = symbols['@feat.00'][1]
            assert struct.unpack_from('<IhHBB', raw, actual + 8) == (0, -1, 0, 3, 0)
            image = out / 'raw-aux-plain.exe'
            run('raw-aux-plain-pe', [binary, 'link', image, 'sengoo_deflate_fixture_length', raw_sdk, sdk], pure=True)
            artifacts[image.name] = sha(image)
            report['raw_aux_diagnostic_boundary'] = dict(raw_object=str(raw_sdk), sha256=sha(raw_sdk),
                plain_coff_pe_link_exit=0, exact_absolute_record=[0, -1, 0, 3, 0],
                mixed_pdb_error=7333, reason='COFF now accepts standard auxiliaries; unchanged @feat.00 reaches the separately unsupported absolute-symbol public catalog')
        report['result'] = 'passed-focused-coff-leaf' if args.focused_only else 'passed-coff-leaf-inputs'
    except BaseException as error:
        report.update(result='failed', error=repr(error))
        raise
    finally:
        report.update(changed_sources=[p for p, h in inputs.items() if not Path(p).is_file() or sha(p) != h],
                      checkout_changes=[p for p, h in origins.items() if not Path(p).is_file() or sha(p) != h],
                      changed_snapshots=[p for p, h in snapshots.items() if not Path(p).is_file() or sha(p) != h],
                      changed_tools=[n for n, item in tools.items() if sha(item['path']) != item['sha256']],
                      changed_configs=[p for p, h in configs.items() if (sha(p) if Path(p).is_file() else None) != h],
                      changed_artifacts=[p for p, h in artifacts.items() if sha(out / p) != h],
                      source_revision_end=revision(), completed_at=dt.datetime.now(dt.timezone.utc).isoformat())
        if any(report[k] for k in ['changed_sources', 'checkout_changes', 'changed_snapshots', 'changed_tools', 'changed_configs', 'changed_artifacts']):
            report['result'] = 'failed'
        save()
    print(report['result'], sha(out / 'evidence.json'), flush=True)
    return 0 if report['result'].startswith('passed') else 1


if __name__ == '__main__':
    raise SystemExit(main())
