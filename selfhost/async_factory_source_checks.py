"""Independent source-level AsyncFile factory matrix against a joint binary."""
from __future__ import annotations
import hashlib
import json
from pathlib import Path


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def run_source_checks(run, pure_env, snapshot, output, binary):
    snapshot, out, binary = map(lambda p: Path(p).resolve(), (snapshot, output, binary))
    out.mkdir(parents=True, exist_ok=False)
    assert pure_env.get('PATH', '') == ''
    library = snapshot / 'tools/stdlib'
    canonical = {p.name: p.read_bytes() for p in library.glob('*.sg')}
    sources = {str(p.resolve()): sha(p) for p in [Path(__file__), binary, *library.glob('*.sg')]}
    report = {'schema': 1, 'stage': 'joint-async-factory-independent-source-matrix',
              'result': 'running', 'sources': sources, 'artifacts': {}, 'cases': [], 'steps': [],
              'limits': ['Source hash mutations test profile rejection, not isolated typed/HIR proof fields.',
                         'Variadic and extern-with-body cases are parser-only rejections.',
                         'No native async scheduler/provider or self-build execution qualification.']}

    def save():
        (out / 'evidence.json').write_text(json.dumps(report, indent=2) + '\n', 'utf8')

    def bind(path):
        sources[str(path.resolve())] = sha(path)

    def fixture(name, text):
        path = out / name; path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(text.encode()); bind(path)
        return path

    def copied_library(name, old=None, new=None):
        target = out / name; target.mkdir()
        for filename, raw in canonical.items():
            if filename == 'file.sg' and old is not None:
                text = raw.decode(); assert text.count(old) == 1, (name, old)
                raw = text.replace(old, new).encode()
            path = target / filename; path.write_bytes(raw); bind(path)
        return target

    def case(name, text, stdlib, accepted, profile, graph=0, family=None):
        entry = fixture(name + '.sg', text)
        argv = list(map(str, [binary, '--semantic', entry, stdlib]))
        done = run('factory-source-' + name, argv, expected=42 if accepted else 65, env=pure_env)
        logs = {}
        for stream in ['stdout', 'stderr']:
            path = out / (name + '.' + stream + '.log'); path.write_bytes(getattr(done, stream))
            report['artifacts'][str(path)] = sha(path); logs[stream + '_sha256'] = sha(path)
        values = list(map(int, done.stdout.decode().splitlines()))
        assert done.stderr == b'' and len(values) >= 4 and values[0] == graph
        assert values[2] == profile and values[3] == (24 if profile == 1 else 0)
        assert done.returncode == (42 if accepted else 65)
        assert (values[1] == 0) == accepted
        if family is not None:
            assert values[1] == family or family in values[4::2], (name, values)
        row = {'name': name, 'command': argv, 'exit_code': done.returncode,
               'expected_exit_code': 42 if accepted else 65, 'observed': values,
               'expected_profile': profile, 'expected_graph_error': graph,
               'verification_layer': 'parser-only' if graph else 'source-semantic-contract',
               'result': 'accepted' if accepted else 'rejected', **logs}
        report['cases'].append(row); report['steps'].append(row.copy()); save()

    save()
    try:
        main = 'import std::file;\ndef main()->i64{42}\n'
        relocated = copied_library('relocated standard 世界')
        case('canonical-import', main, library, True, 1)
        case('relocated-import', main, relocated, True, 1)
        body = 'sengoo_async_file_wait_readable__start(self.handle, timeout_ms)'
        declaration = 'fn sengoo_async_file_wait_readable__start(handle: i64, timeout_ms: i64) -> i64;'
        mutations = {
            'literal-99': (body, '99'),
            'swapped-operands': (body, 'sengoo_async_file_wait_readable__start(timeout_ms, self.handle)'),
            'duplicate-start': (body, body + '; ' + body),
            'explicit-return': (body, 'return ' + body + ';'),
            'mut-receiver': ('def wait_readable(&self, timeout_ms: i64)', 'def wait_readable(&mut self, timeout_ms: i64)'),
            'width-i32': ('timeout_ms: i64) -> Future<FileReadinessOutcome>', 'timeout_ms: i32) -> Future<FileReadinessOutcome>'),
            'provider-result-bool': (declaration, declaration.replace('-> i64', '-> bool')),
            'provider-variadic': (declaration, declaration.replace('timeout_ms: i64)', 'timeout_ms: i64, ...)')),
            'provider-input-i32': (declaration, declaration.replace('handle: i64', 'handle: i32')),
            'provider-non-C': ('extern "C" {', 'extern "system" {'),
            'provider-extern-body': (declaration, declaration[:-1] + '{99}'),
            'receiver-extra-field': ('struct AsyncFile {\n    handle: i64,\n}', 'struct AsyncFile {\n    handle: i64,\n    extra: i64,\n}'),
            'output-field-order': ('is_ok: bool,\n    value: bool,\n    error: i64,', 'error: i64,\n    is_ok: bool,\n    value: bool,'),
            'output-width': ('struct FileReadinessOutcome {\n    is_ok: bool,\n    value: bool,\n    error: i64,', 'struct FileReadinessOutcome {\n    is_ok: bool,\n    value: bool,\n    error: i32,'),
            'output-nominal-substitute': ('-> Future<FileReadinessOutcome>', '-> Future<bool>'),
            'changed-comment': ('struct AsyncFile {', '// Changed source contract bytes\nstruct AsyncFile {'),
        }
        for name, (old, new) in mutations.items():
            case(name, main, copied_library(name + '-stdlib', old, new), False, 0,
                 graph=3001 if name in {'provider-variadic', 'provider-extern-body'} else 0)
        local_lib = copied_library('provider-local-body-stdlib', declaration, '')
        local_file = local_lib / 'file.sg'
        local_file.write_bytes(local_file.read_bytes() + b'\ndef sengoo_async_file_wait_readable__start(handle:i64,timeout_ms:i64)->i64{99}\n')
        bind(local_file)
        case('provider-local-body', main, local_lib, False, 0, family=5124)
        ordinary = 'struct FileReadinessOutcome{is_ok:bool,value:bool,error:i64}\nstruct AsyncFile{handle:i64}\nextern "C"{fn sengoo_async_file_wait_readable__start(handle:i64,timeout_ms:i64)->i64;}\nimpl AsyncFile{def wait_readable(&self,timeout_ms:i64)->Future<FileReadinessOutcome>{sengoo_async_file_wait_readable__start(self.handle,timeout_ms)}}\ndef main()->i64{42}\n'
        case('same-name-user', ordinary, relocated, False, 0, family=5124)
        fixture('shadow.sg', canonical['file.sg'].decode())
        case('same-bytes-nonstandard-module', 'import std::ffi;import std::status;import std::result;import std::string;import shadow;def main()->i64{42}', relocated, False, 0, family=5124)
        ordinary_cases = {
            'ordinary-direct-return': ('async def worker()->i64{42}\ndef escape()->Future<i64>{worker()}\ndef main()->i64{42}\n', 5124),
            'ordinary-nested-return': ('struct Wrap{value:Future<i64>}\nasync def worker()->i64{42}\ndef escape()->Wrap{Wrap{value:worker()}}\ndef main()->i64{42}\n', 5124),
            'ordinary-impl-wrong-return': ('struct Example{} impl Example{def value(&self)->bool{99}} def main()->i64{42}\n', 5104),
            'ordinary-impl-literal-future': ('struct Example{} impl Example{def value(&self)->Future<i64>{99}} def main()->i64{42}\n', 5124),
            'foreign-future-return': ('extern "C"{fn foreign_future()->Future<i64>;}\ndef main()->i64{42}\n', 5122),
            'direct-local-await': ('async def worker()->i64{42}\nasync def main()->i64{await worker()}\n', None),
        }
        for name, (text, family) in ordinary_cases.items():
            case(name, text, relocated, name == 'direct-local-await', 0, family=family)
        assert len(report['cases']) == 27
        report['result'] = 'passed-source-matrix-with-classified-parser-controls'
    except BaseException as error:
        report['result'] = 'failed'; report['error'] = repr(error)
        raise
    finally:
        report['changed_sources'] = [p for p, h in sources.items() if not Path(p).is_file() or sha(p) != h]
        report['changed_artifacts'] = [p for p, h in report['artifacts'].items() if not Path(p).is_file() or sha(p) != h]
        save()
        assert not report['changed_sources'] and not report['changed_artifacts']
    return report
