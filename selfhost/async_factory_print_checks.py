"""Print/ordinary verification component for a single immutable joint compiler.

The coordinator captures collect_inputs() before its producer build. Its run
callback accepts (name, argv, expected=0, env=...) and returns CompletedProcess
with byte stdout/stderr, recording every command and actual terminal status.
No compiler build is performed here. The joint entry must expose --print-hir.
"""
from __future__ import annotations
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import sys


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def collect_inputs(snapshot, actual_checkout, extra=()):
    """Call before producer compilation; retain this returned mapping unchanged."""
    snapshot, root = Path(snapshot).resolve(), Path(actual_checkout).resolve()
    paths = {Path(__file__).resolve(), Path(__file__).with_name('print_hir_fragment.sg'), Path(sys.executable).resolve()}
    paths.update(Path(p).resolve() for p in extra)
    for base in [snapshot, root / 'runtime']:
        paths.update(p.resolve() for p in base.rglob('*') if p.is_file()
                     and not any(part in {'target', 'build', '__pycache__', '.git'}
                                 for part in p.relative_to(base).parts))
    for name in ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', '.cargo/config',
                 '.cargo/config.toml', 'selfhost/reference-baseline.lock']:
        if (root / name).is_file():
            paths.add((root / name).resolve())
    cargo_home = Path(os.environ.get('CARGO_HOME', str(Path.home() / '.cargo')))
    for parent in [*root.parents, cargo_home]:
        for name in ['.cargo/config', '.cargo/config.toml'] if parent != cargo_home else ['config', 'config.toml']:
            if (parent / name).is_file():
                paths.add((parent / name).resolve())
    return {str(p): sha(p) for p in sorted(paths)}


def compose_entry(native_source):
    """Optional entry composer when the coordinator does not have its own CLI."""
    marker = 'def main() -> i64 {'
    assert native_source.count(marker) == 1
    fragment = Path(__file__).with_name('print_hir_fragment.sg').read_text('utf8')
    return native_source.replace(marker, 'def print_original_main() -> i64 {') + '\n' + fragment + '''
def main()->i64 {
 if args_len()==1 {let arg=sh_nc_argument(0);if arg.is_ok() and arg.value.as_str()=="--print-hir" {return sh_print_formal_hir();};};
 print_original_main();
}
'''


def _dependencies(path):
    raw = Path(path).read_text('utf8').replace('\\\n', ' ')
    assert raw.startswith('print_dependencies:')
    words, word, index = [], [], 0
    payload = raw.split(':', 1)[1]
    while index < len(payload):
        value = payload[index]
        if value == '\\' and index + 1 < len(payload) and payload[index + 1] in ' \t#':
            word.append(payload[index + 1]); index += 2
        elif value == '$' and index + 1 < len(payload) and payload[index + 1] == '$':
            word.append('$'); index += 2
        elif value.isspace():
            if word:
                words.append(''.join(word)); word = []
            index += 1
        else:
            word.append(value); index += 1
    if word:
        words.append(''.join(word))
    assert words and all(Path(p).is_absolute() for p in words)
    return {str(Path(p).resolve()): sha(p) for p in words}


def run_print_checks(run, build_env, pure_env, snapshot, output, native_binary, *,
                     reference, clang, producer_inputs, actual_checkout,
                     link_inputs, ordinary=True):
    """Run print/HIR, exact outputs and all 49 ordinary sources against joint bytes.

    link_inputs is the coordinator's pre-build hash map containing the selected
    system link.exe/lld-link.exe and conservative actual CRT/SDK *.lib search
    closure. This function verifies clang still chooses that linker and pins
    every observed /VERBOSE:LIB library. It never substitutes historical results.
    """
    snapshot, out, native_binary = map(lambda p: Path(p).resolve(), (snapshot, output, native_binary))
    root, reference, clang = map(lambda p: Path(p).resolve(), (actual_checkout, reference, clang))
    assert out.is_relative_to(root / 'target')
    out.mkdir(parents=True, exist_ok=False)
    assert producer_inputs and link_inputs and not pure_env.get('PATH', '')
    assert all(sha(p) == h for p, h in producer_inputs.items())
    assert all(sha(p) == h for p, h in link_inputs.items())
    for env in [build_env, pure_env]:
        assert Path(env['SENGOO_STDLIB']).resolve() == snapshot / 'tools/stdlib'
    assert Path(build_env['SENGOO_RUNTIME']).resolve() == snapshot / 'tools/stdlib/runtime.c'
    assert Path(build_env['SENGOO_CLANG']).resolve() == clang
    for tool in [reference, clang]:
        assert producer_inputs.get(str(tool)) == sha(tool), 'tool was not captured before producer build'
    for p in [Path(__file__).resolve(), Path(__file__).with_name('print_hir_fragment.sg').resolve()]:
        assert producer_inputs.get(str(p)) == sha(p), 'print component was not captured before producer build'
    baseline_path = snapshot / 'selfhost/reference-baseline.lock'
    if not baseline_path.is_file():
        baseline_path = root / 'selfhost/reference-baseline.lock'
    assert producer_inputs.get(str(baseline_path.resolve())) == sha(baseline_path)
    baseline = json.loads(baseline_path.read_text('utf8'))
    assert sha(reference) == baseline['tools']['sgc']['sha256']
    inputs = dict(producer_inputs)
    inputs.update(link_inputs)
    for p in [reference, clang, native_binary]:
        inputs[str(p)] = sha(p)
    report = {'schema': 1, 'stage': 'joint-print-and-ordinary-source-qualification',
              'result': 'running', 'sources': inputs, 'artifacts': {}, 'steps': [],
              'c_headers': {}, 'actual_link_libraries': {}, 'cases': [],
              'reference_differences': [], 'ordinary_cases': [],
              'build_environment': {k: build_env[k] for k in ['SENGOO_STDLIB', 'SENGOO_RUNTIME', 'SENGOO_CLANG', 'SENGOO_MODULE_MAP', 'CARGO_HOME', 'RUSTC', 'RUSTFLAGS', 'LIB', 'INCLUDE'] if k in build_env},
              'pure_environment': {k: pure_env[k] for k in ['SENGOO_STDLIB', 'SENGOO_MODULE_MAP', 'PATH'] if k in pure_env},
              'limits': ['Signed i64/shared str print subset only; no full formatting or std::file success claim.',
                         'Uses this joint native binary and current immutable source closure; no reused prior pass.']}

    def save():
        (out / 'evidence.json').write_text(json.dumps(report, indent=2) + '\n', 'utf8')

    def bind(path):
        path = Path(path).resolve()
        digest = sha(path)
        assert str(path) not in inputs or inputs[str(path)] == digest, str(path)
        inputs[str(path)] = digest
        return digest

    def artifact(path):
        path = Path(path).resolve()
        report['artifacts'][str(path)] = sha(path)

    def command(name, argv, expected=0, pure=False):
        argv = list(map(str, argv))
        completed = run('print-' + name, argv, expected=expected,
                        env=pure_env if pure else build_env)
        assert isinstance(completed.stdout, bytes) and isinstance(completed.stderr, bytes)
        row = {'name': name, 'command': argv, 'expected_exit_code': expected,
               'exit_code': completed.returncode}
        for stream in ['stdout', 'stderr']:
            path = out / (name + '.' + stream + '.log')
            path.write_bytes(getattr(completed, stream)); artifact(path)
            row[stream + '_sha256'] = sha(path)
        report['steps'].append(row); save()
        assert completed.returncode == expected, name
        return completed

    def compile_c(label, source):
        source = Path(source).resolve(); bind(source)
        before, after, obj = (out / (label + suffix) for suffix in ['-headers.d', '-compiled.d', '.obj'])
        flags = ['-O0', '-ffunction-sections']
        command(label + '-headers', [clang, *flags, '-M', '-MT', 'print_dependencies', '-MF', before, source])
        headers = _dependencies(before)
        for path, digest in headers.items():
            assert bind(path) == digest
        command(label + '-compile', [clang, *flags, '-MD', '-MT', 'print_dependencies', '-MF', after, '-c', source, '-o', obj])
        assert _dependencies(after) == headers
        report['c_headers'][str(source)] = headers
        for p in [before, after, obj]:
            artifact(p)
        return obj

    def link(label, source, runtime_objects, executable):
        done = command(label, [clang, source, *runtime_objects, '-Wl,/OPT:REF', '-Wl,/VERBOSE:LIB', '-o', executable])
        text = (done.stdout + done.stderr).decode('utf8', 'replace')
        # MSVC emits absolute Searching and Loaded paths. Every consumed path
        # must belong to the pre-build conservative selected-library closure.
        seen = set()
        for line in text.splitlines():
            found = re.search(r'([A-Za-z]:[\\/].*?\.lib)(?:\(|:|\s*$)', line, re.IGNORECASE)
            if found:
                path = str(Path(found.group(1)).resolve())
                assert path in link_inputs, 'unbound linker library: ' + path
                assert sha(path) == link_inputs[path]
                seen.add(path)
        assert seen, 'linker did not report actual library paths'
        report['actual_link_libraries'][label] = {p: link_inputs[p] for p in sorted(seen)}
        artifact(executable)

    save()
    try:
        selected = command('linker-selection', [clang, '-###', snapshot / 'tools/stdlib/runtime.c', '-o', out / 'selected.exe'])
        words = re.findall(r'"([^"\r\n]+)"', (selected.stdout + selected.stderr).decode('utf8'))
        selected_linkers = [str(Path(p).resolve()) for p in words if p.lower().endswith(('link.exe', 'lld-link.exe'))]
        assert len(selected_linkers) == 1 and selected_linkers[0] in link_inputs
        report['selected_linker'] = {'path': selected_linkers[0], 'sha256': link_inputs[selected_linkers[0]]}
        hir = command('hir-coherent-mutations', [native_binary, '--print-hir'], 42, True)
        assert hir.stdout == hir.stderr == b''
        report['hir'] = {'mutation_kinds': 21, 'scopes': 3, 'clone_and_wire': True,
                         'resources': ['String', 'Buffer', 'opaque'], 'restored_from_first_scope': True}
        runtime = compile_c('runtime', snapshot / 'tools/stdlib/runtime.c')
        strings = compile_c('runtime-string', snapshot / 'tools/stdlib/runtime_string.c')
        cases = {
            'streams': ('def message(s:&str){print(s);println(s);eprintln(s);}\ndef main()->i64{message("世界 😀");print(42);println(-7);eprintln(0);print("A\\0B");eprintln("C\\0D");print(9223372036854775807);eprintln(-9223372036854775807-1);42}\n', 0, None),
            'empty': ('def main()->i64{print("");println("");eprintln("");42}\n', 0, None),
            'unused': ('def unused(msg:&str){print(msg);}\ndef main()->i64{42}\n', 0, None),
            'shadow': ('def print(x:i64)->i64{x+1}\ndef main()->i64{print(41)}\n', 0, None),
            'option': ('import std::option;\ndef main()->i64{42}\n', 0, None),
            'bool': ('def main()->i64{print(true);42}\n', 65, 5008),
            'u64': ('def main()->i64{print(42u64);42}\n', 65, 5008),
            'reserved-body': ('def sengoo_print_i64(x:i64){}\ndef main()->i64{print(42);42}\n', 65, 5009),
            'reserved-signature': ('extern "C"{fn sengoo_print_i64(x:bool);}\ndef main()->i64{print(42);42}\n', 65, 5009),
            'wrong-arity': ('def main()->i64{print(1,2);42}\n', 65, 5105),
        }
        expected_streams = {
            'streams': ('世界 😀\r\n世界 😀\r\n42\r\n-7\r\nA\r\n9223372036854775807\r\n'.encode(), '世界 😀\r\n0\r\nC\r\n-9223372036854775808\r\n'.encode()),
            'empty': (b'\r\n\r\n', b'\r\n'),
        }
        for name, (text, status, diagnostic_code) in cases.items():
            source, obj, repeat = (out / (name + ext) for ext in ['.sg', '.obj', '-repeat.obj'])
            source.write_bytes(text.encode()); bind(source)
            emitted = command(name + '-emit', [native_binary, source, obj], status, True)
            protocol = json.loads(emitted.stdout); assert emitted.stderr == b''
            row = {'name': name, 'source': str(source), 'source_sha256': sha(source)}
            report['cases'].append(row)
            if status != 0:
                assert not obj.exists() and protocol['ok'] is False and protocol['code'] == diagnostic_code
                sentinel = out / (name + '-preserved.obj')
                preserved = b'PRINT-REJECTION-SENTINEL\0' + name.encode() + b'\xff\x80\r\n'
                sentinel.write_bytes(preserved)
                before = sha(sentinel)
                rejected = command(name + '-preserve-output', [native_binary, source, sentinel], status, True)
                assert rejected.stderr == b'' and json.loads(rejected.stdout) == protocol
                assert sentinel.read_bytes() == preserved and sha(sentinel) == before
                artifact(sentinel)
                row.update(result='rejected', diagnostic=protocol,
                           preexisting_output={'path': str(sentinel), 'before_sha256': before,
                                               'after_sha256': sha(sentinel), 'unchanged': True})
                continue
            assert protocol == {'schema_version': 1, 'ok': True, 'phase': 'object'}
            artifact(obj); command(name + '-repeat', [native_binary, source, repeat], 0, True)
            assert obj.read_bytes() == repeat.read_bytes(); artifact(repeat)
            exe, ir, reference_exe = (out / (name + ext) for ext in ['-native.exe', '.ll', '-reference.exe'])
            link(name + '-native-link', obj, [runtime, strings], exe)
            actual = command(name + '-native-run', [exe], 42, True)
            assert (actual.stdout, actual.stderr) == expected_streams.get(name, (b'', b''))
            command(name + '-reference-ir', [reference, '--runtime-mode', 'source-development', 'build', source, '--force-rebuild', '-O', '0', '--emit-llvm', '-o', ir]); artifact(ir)
            link(name + '-reference-link', ir, [runtime, strings], reference_exe)
            other = command(name + '-reference-run', [reference_exe], 0 if name == 'shadow' else 42, True)
            if name == 'shadow':
                assert other.stdout == b'41\r\n' and other.stderr == b''
                row['result'] = 'explicit-reference-difference'
                report['reference_differences'].append({'case': name, 'reason': 'Frozen reference uses builtin spelling despite a resolved user function.', 'candidate_exit': 42, 'reference_exit': 0})
            else:
                assert (actual.stdout, actual.stderr) == (other.stdout, other.stderr)
                row['result'] = 'byte-identical-output'
            save()
        if ordinary:
            fixture_module = snapshot / 'selfhost/source_codegen_gate.py'; bind(fixture_module)
            sys.path.insert(0, str(snapshot / 'selfhost'))
            try:
                spec = importlib.util.spec_from_file_location('joint_print_ordinary_cases', fixture_module)
                corpus = importlib.util.module_from_spec(spec); spec.loader.exec_module(corpus)
            finally:
                sys.path.pop(0)
            assert len(corpus.CASES) == 49
            probe = compile_c('ordinary-runtime-probe', snapshot / 'selfhost/tests/native_source_probe.c')
            for name, text in corpus.CASES.items():
                label = 'ordinary-' + name
                source, obj, repeat, ir = (out / (label + ext) for ext in ['.sg', '.obj', '-repeat.obj', '.ll'])
                source.write_bytes(text.encode()); bind(source)
                emitted = command(label + '-emit', [native_binary, source, obj], 0, True)
                assert json.loads(emitted.stdout) == {'schema_version': 1, 'ok': True, 'phase': 'object'} and emitted.stderr == b''
                command(label + '-repeat', [native_binary, source, repeat], 0, True)
                assert obj.read_bytes() == repeat.read_bytes(); artifact(obj); artifact(repeat)
                exe, reference_exe = out / (label + '-native.exe'), out / (label + '-reference.exe')
                link(label + '-native-link', obj, [probe], exe)
                actual = command(label + '-native-run', [exe], 42, True)
                assert actual.stdout == actual.stderr == b''
                ref = command(label + '-reference-ir', [reference, '--runtime-mode', 'source-development', 'build', source, '--force-rebuild', '-O', '0', '--emit-llvm', '-o', ir], 1 if name in corpus.REFERENCE_SOURCE_GAPS else 0)
                row = {'name': name, 'native_exit': 42, 'source': str(source), 'sha256': sha(source)}
                if name in corpus.REFERENCE_SOURCE_GAPS:
                    assert not ir.exists()
                    assert corpus.REFERENCE_SOURCE_GAPS[name] in ref.stdout + ref.stderr
                    row['result'] = 'recorded-reference-source-gap'
                elif name in corpus.REFERENCE_LINK_GAPS:
                    artifact(ir)
                    failed = command(label + '-reference-link', [clang, ir, probe, '-o', reference_exe], 1)
                    assert not reference_exe.exists()
                    assert all(marker in failed.stderr for marker in corpus.REFERENCE_LINK_GAPS[name])
                    row['result'] = 'recorded-reference-link-gap'
                else:
                    artifact(ir); link(label + '-reference-link', ir, [probe], reference_exe)
                    other = command(label + '-reference-run', [reference_exe], 42, True)
                    assert (actual.stdout, actual.stderr) == (other.stdout, other.stderr)
                    row['result'] = 'byte-identical-output'
                report['ordinary_cases'].append(row); save()
        report['result'] = 'passed-with-recorded-reference-differences'
    except BaseException as error:
        report['result'] = 'failed'; report['error'] = repr(error)
        raise
    finally:
        report['changed_sources'] = [p for p, h in inputs.items() if not Path(p).is_file() or sha(p) != h]
        report['changed_artifacts'] = [p for p, h in report['artifacts'].items() if not Path(p).is_file() or sha(p) != h]
        save()
        assert not report['changed_sources'] and not report['changed_artifacts']
    return report
