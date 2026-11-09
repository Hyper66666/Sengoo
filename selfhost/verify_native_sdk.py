"""Qualify pure SG native SDK inventory admission, not a final runtime SDK.

Python prepares adversarial inputs and invokes the SG implementation. It does
not implement admission or substitute for native execution of the SG parser.
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
import struct
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
MODULES = ['core', 'lexer', 'parser', 'types', 'native_sdk_json',
           'native_sdk_contracts', 'native_sdk', 'native_sdk_artifact', 'native_sdk_bind']


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def revision() -> str:
    return subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()


def prepare(out: Path) -> tuple[Path, dict]:
    catalog = ROOT / 'selfhost/sdk/native_x64_abi1/manifest.json'
    original = json.loads(catalog.read_text(encoding='utf-8'))
    folder = out / 'fixtures'
    folder.mkdir(parents=True, exist_ok=True)
    cases = []

    def case(name: str, value: dict | bytes, expected: int, *, root: str = '.',
             pin: str | None = None, target: str = 'x86_64-pc-windows-msvc',
             abi: int = 1, artifact: dict | None = None) -> None:
        path = folder / (name + '.json')
        path.write_bytes(value if isinstance(value, bytes) else
                         (json.dumps(value, ensure_ascii=False, indent=2) + '\n').encode())
        row = {'name': name, 'root': root, 'path': str(path), 'pin': pin or sha(path),
               'target': target, 'abi': abi, 'expected': expected}
        if artifact is not None:
            row['artifact'] = artifact
        cases.append(row)

    case('valid', original, 0)
    case('caller-wrong-pin', original, 5402, pin='0' * 64)
    case('caller-invalid-pin', original, 5402, pin='not-a-sha256')
    case('caller-wrong-target', original, 5409, target='aarch64-pc-windows-msvc')
    case('caller-wrong-abi', original, 5409, abi=2)
    mutations = [
        ('portable-family', lambda x: x.update(family='portable-logical-layout'), 5409),
        ('wrong-target', lambda x: x.update(target='aarch64-pc-windows-msvc'), 5409),
        ('wrong-width', lambda x: x.update(pointer_bits=32), 5409),
        ('wrong-endian', lambda x: x.update(endianness='big'), 5409),
        ('wrong-abi', lambda x: x.update(abi_version=2), 5409),
        ('false-port-claim', lambda x: x.update(runtime_algorithms_ported=True), 5409),
        ('outcome-offset', lambda x: x['layouts'][0]['fields'][2].update(offset=4), 5407),
        ('outcome-size', lambda x: x['layouts'][0].update(size=12), 5407),
        ('outcome-physical-type', lambda x: x['layouts'][0]['fields'][0].update(type=1), 5407),
        ('missing-lifecycle', lambda x: x['language_contracts'][1]['operations'].pop(), 5407),
        ('wrong-future-origin', lambda x: x['language_contracts'][2].update(origin='same_spelling'), 5407),
        ('wrong-language-output', lambda x: x['language_contracts'][2].update(output='i64'), 5407),
        ('wrong-c-ret', lambda x: x['exports'][11].update(result=2), 5405),
        ('wrong-out-pointer', lambda x: x['exports'][13]['parameters'].__setitem__(0, 1), 5405),
        ('wrong-convention', lambda x: x['exports'][2].update(convention='SG'), 5405),
        ('wrong-consume', lambda x: x['exports'][8].update(consumes_handle=False), 5405),
        ('wrong-unsafe', lambda x: x['exports'][7].update(unsafe=False), 5405),
        ('wrong-prototype', lambda x: x['exports'][2].update(prototype='long long fake(void)'), 5405),
        ('wrong-prototype-span', lambda x: x['exports'][2].update(prototype_begin=0), 5405),
        ('overflow-prototype-span', lambda x: x['exports'][2].update(prototype_begin=9223372036854775807), 5405),
        ('negative-source-id', lambda x: x['exports'][2].update(source=-1), 5405),
        ('overflow-source-id', lambda x: x['exports'][2].update(source=9223372036854775807), 5405),
        ('missing-export', lambda x: x['exports'].pop(), 5405),
        ('duplicate-export', lambda x: x['exports'][4].update(id=3), 5405),
        ('source-hash', lambda x: x['sources'][0].update(sha256='0' * 64), 5402),
        ('source-size', lambda x: x['sources'][0].update(bytes=1), 5402),
        ('source-path-traversal', lambda x: x['sources'][0].update(path='../tools/stdlib/runtime.c'), 5403),
        ('source-absolute', lambda x: x['sources'][0].update(path='D:/Sengoo/tools/stdlib/runtime.c'), 5403),
        ('source-missing', lambda x: x['sources'][0].update(path='missing-native-provider.c'), 5401),
        ('false-component-class', lambda x: x['components'][1].update(**{'class': 'final-shim'}), 5408),
        ('extra-header-field', lambda x: x.update(unreviewed=True), 5409),
    ]
    for name, mutate, expected in mutations:
        value = copy.deepcopy(original)
        mutate(value)
        case(name, value, expected)
    raw = json.dumps(original).encode()
    case('duplicate-json-key', b'{"schema":"bad",' + raw[1:], 5403)
    case('malformed-json', b'{"schema":', 5403)
    case('invalid-utf8', b'{"schema":"\xff"}', 5403)
    case('trailing-json', raw + b' false', 5403)

    # Isolated physical roots allow malformed declarations without touching the
    # checked-in prelude or the frozen provider source tree.
    for name, before, after, expected in [
        ('prelude-result', 'fn sengoo_string_len(handle: i64) -> i64;', 'fn sengoo_string_len(handle: i64) -> bool;', 5405),
        ('prelude-output-pointer', 'output: *mut ShNativeFileReadinessOutcome', 'output: i64', 5405),
        ('prelude-c-convention', 'extern "C"', 'extern "SG"', 5405),
        ('prelude-own-body', 'fn sengoo_string_len(handle: i64) -> i64;', 'fn sengoo_string_len(handle: i64) -> i64 {42}', 5405),
    ]:
        clone = folder / name
        for item in original['sources']:
            path = clone / item['path']
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes((ROOT / item['path']).read_bytes())
        prelude = clone / original['prelude']['path']
        prelude.parent.mkdir(parents=True, exist_ok=True)
        data = (ROOT / original['prelude']['path']).read_text(encoding='utf-8')
        if data.count(before) != 1:
            raise ValueError('prelude fixture replacement is not unique')
        prelude.write_bytes(data.replace(before, after).encode())
        value = copy.deepcopy(original)
        value['prelude']['sha256'] = sha(prelude)
        case(name, value, expected, root=str(clone))

    # Use the actual separately verified runtime_string.c COFF object. This is
    # content/symbol admission in development mode, not final SDK qualification.
    dependency_path = ROOT / 'target/selfhost/m3-string-runtime/checkpoint/evidence.json'
    dependency = json.loads(dependency_path.read_text(encoding='utf-8'))
    obj = dependency_path.parent / 'runtime_string.obj'
    if dependency.get('result') != 'passed' or dependency.get('source_changes') or dependency.get('tool_changes'):
        raise ValueError('registered String runtime dependency is not a clean pass')
    if sha(obj) != dependency['artifacts']['runtime_string.obj']:
        raise ValueError('registered String object identity changed')
    provider_path = str(ROOT / 'tools/stdlib/runtime_string.c')
    if sha(ROOT / 'tools/stdlib/runtime_string.c') != dependency['inputs_before'][provider_path]:
        raise ValueError('registered String provider source changed')
    selected = copy.deepcopy(original)
    selected['components'][0]['artifacts'] = [{'path':str(obj.relative_to(ROOT)).replace('\\','/'), 'sha256':sha(obj),
        'format':'coff-object', 'target':'x86_64-pc-windows-msvc', 'abi_version':1}]
    descriptor = {'component':0, 'ordinal':0, 'pin':sha(obj), 'expected':0,
                  'development':1, 'exports':[1, 2, 3, 4, 5], 'missing':0}
    case('actual-string-coff', selected, 0, artifact=descriptor)
    wrong = copy.deepcopy(descriptor)
    wrong.update(expected=5410, development=0)
    case('reference-not-final-sdk', selected, 0, artifact=wrong)
    wrong = copy.deepcopy(descriptor)
    wrong.update(expected=5408, pin='0' * 64)
    case('artifact-caller-pin-mismatch', selected, 0, artifact=wrong)
    absent = copy.deepcopy(descriptor)
    absent.update(expected=5408)
    case('artifact-missing-from-catalog', original, 0, artifact=absent)
    data = bytearray(obj.read_bytes())
    data[0:2] = b'\x64\xaa'
    wrong_object = folder / 'wrong-target.obj'
    wrong_object.write_bytes(data)
    wrong_catalog = copy.deepcopy(selected)
    wrong_catalog['components'][0]['artifacts'][0].update(path=str(wrong_object.relative_to(ROOT)).replace('\\','/'), sha256=sha(wrong_object))
    wrong = copy.deepcopy(descriptor)
    wrong.update(expected=5409, pin=sha(wrong_object))
    case('artifact-physical-target', wrong_catalog, 0, artifact=wrong)
    raw_object = obj.read_bytes()
    symbol_table, symbol_count = struct.unpack_from('<II', raw_object, 8)
    position = 0
    record = None
    while position < symbol_count:
        offset = symbol_table + position * 18
        section, function, storage, auxiliary = struct.unpack_from('<hHBB', raw_object, offset + 12)
        if section > 0 and storage == 2 and function & 0x30 == 0x20:
            record = offset
            break
        position += 1 + auxiliary
    if record is None:
        raise ValueError('runtime object has no actual defined function for adversarial controls')
    for name, field, replacement in [('section-out-of-range', 12, b'\xff\x7f'),
                                      ('symbol-value-out-of-range', 8, b'\xff\xff\xff\xff')]:
        data = bytearray(raw_object)
        data[record+field:record+field+len(replacement)] = replacement
        malformed = folder / (name+'.obj')
        malformed.write_bytes(data)
        value = copy.deepcopy(selected)
        value['components'][0]['artifacts'][0].update(path=str(malformed.relative_to(ROOT)).replace('\\','/'), sha256=sha(malformed))
        descriptor = copy.deepcopy(wrong)
        descriptor.update(pin=sha(malformed), expected=5409)
        case(name, value, 0, artifact=descriptor)
    cases_path = folder / 'cases.json'
    cases_path.write_bytes((json.dumps(cases, ensure_ascii=False, indent=2) + '\n').encode())
    return cases_path, {'path':str(dependency_path), 'sha256':sha(dependency_path),
                        'object':str(obj), 'object_sha256':sha(obj), 'cases':len(cases)}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=ROOT / 'target/selfhost/native-sdk/checkpoint')
    args = parser.parse_args()
    out = args.output.resolve()
    if not out.is_relative_to(ROOT / 'target'):
        raise ValueError('gate output must remain inside target/')
    out.mkdir(parents=True, exist_ok=True)
    report = {'schema':1, 'stage':'pure-sg-native-sdk-inventory-admission', 'result':'running',
              'started_at':dt.datetime.now(dt.timezone.utc).isoformat(), 'host':platform.platform(),
              'python':sys.version, 'milestone_complete':False, 'steps':[],
              'scope':'inventory, caller pins, physical declarations and development COFF identity/symbols; no final SDK or native async lifecycle'}
    before = {}
    tools = {}
    save = lambda: (out / 'evidence.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    save()
    try:
        report['source_revision'] = revision()
        lock = json.loads((ROOT / 'selfhost/bootstrap.lock').read_text(encoding='utf-8'))
        tool_paths = {'seed':ROOT/'target/agent-conditional-compiler/bin/sgc.exe',
                      'formatter':ROOT/'target/agent-formatter/debug/sgfmt.exe',
                      'clang':Path('C:/Users/admin/AppData/Local/SengooToolchain/clang+llvm-19.1.7-x86_64-pc-windows-msvc/bin/clang.exe')}
        for name, path in tool_paths.items():
            if sha(path) != lock[name]['sha256']:
                raise ValueError('unverified tool identity: ' + name)
            tools[str(path)] = sha(path)
        env = dict(os.environ, SENGOO_STDLIB=str(ROOT/'tools/stdlib'), SENGOO_RUNTIME=str(ROOT/'tools/stdlib/runtime.c'),
                   SENGOO_CLANG=str(tool_paths['clang']), SENGOO_MODULE_MAP=';'.join('selfhost_'+n+'='+str(ROOT/'selfhost/src'/f'{n}.sg') for n in MODULES))
        env['PATH'] = str(tool_paths['clang'].parent) + ';' + env.get('PATH', '')

        def run(name: str, command: list, required: str | None = None) -> subprocess.CompletedProcess:
            begin = time.monotonic()
            process = subprocess.run(list(map(str, command)), cwd=ROOT, env=env, capture_output=True,
                                     text=True, encoding='utf-8', errors='replace', timeout=180)
            (out/(name+'.stdout.log')).write_text(process.stdout, encoding='utf-8')
            (out/(name+'.stderr.log')).write_text(process.stderr, encoding='utf-8')
            report['steps'].append({'name':name, 'command':list(map(str,command)), 'exit':process.returncode,
                                    'seconds':time.monotonic()-begin})
            save()
            print(name, process.returncode, flush=True)
            if process.returncode or required is not None and required not in process.stdout:
                raise RuntimeError(name + ' failed; see retained logs')
            return process

        cases, dependency = prepare(out)
        report['registered_string_dependency'] = dependency
        inputs = [ROOT/'selfhost/src'/f'{n}.sg' for n in MODULES]
        inputs += [ROOT/'selfhost/tests/native_sdk.sg', Path(__file__), ROOT/'selfhost/bootstrap.lock']
        inputs += [p for p in (ROOT/'selfhost/sdk').rglob('*') if p.is_file()]
        # tasks.md records the resulting report hash after a run. Excluding that
        # evidence ledger avoids a self-referential report/document hash cycle.
        inputs += [p for p in (ROOT/'openspec/changes/add-selfhost-native-sdk').rglob('*.md')
                   if p.name != 'tasks.md']
        inputs += [p for p in (ROOT/'tools/stdlib').rglob('*') if p.suffix in ('.sg','.c','.h')]
        for tree in ['compiler', 'runtime', 'tools/sgc']:
            inputs += [p for p in (ROOT/tree).rglob('*')
                       if p.is_file() and (p.suffix == '.rs' or p.name in ('Cargo.toml','Cargo.lock'))]
        inputs += [ROOT/'Cargo.toml', ROOT/'Cargo.lock', ROOT/'rust-toolchain.toml']
        inventory = json.loads((ROOT/'selfhost/sdk/native_x64_abi1/manifest.json').read_text(encoding='utf-8'))
        inputs += [ROOT/x['path'] for x in inventory['sources']]
        inputs += [p for p in (out/'fixtures').rglob('*') if p.is_file()]
        inputs += [Path(dependency['path']), Path(dependency['object'])]
        before = {str(p):sha(p) for p in sorted(set(inputs))}
        report['inputs_before'] = before
        report['tools'] = {name:{'path':str(path),'sha256':sha(path)} for name,path in tool_paths.items()}
        run('deterministic-inventory', [sys.executable, ROOT/'selfhost/sdk/update_native_inventory.py', '--output', out/'regenerated-manifest.json'])
        if (out/'regenerated-manifest.json').read_bytes() != (ROOT/'selfhost/sdk/native_x64_abi1/manifest.json').read_bytes():
            raise ValueError('catalog fingerprints/prototypes require an explicit development refresh')
        for name in MODULES[4:]:
            run('format-'+name, [tool_paths['formatter'], '--check', ROOT/'selfhost/src'/f'{name}.sg'])
        run('format-tests', [tool_paths['formatter'], '--check', ROOT/'selfhost/tests/native_sdk.sg'])
        run('format-prelude', [tool_paths['formatter'], '--check', ROOT/'selfhost/sdk/native_x64_abi1/prelude.sg'])
        exe = out/'native-sdk.exe'
        run('build-native-sdk', [tool_paths['seed'], '--runtime-mode', 'source-development', 'build',
            ROOT/'selfhost/tests/native_sdk.sg', '--force-rebuild', '-O', '0', '-o', exe])
        native = run('native-sdk-admission', [exe, cases], 'selfhost native sdk: ok')
        expected_marker = 'SDK external fixtures passed\n' + str(dependency['cases']) + '\n'
        if expected_marker not in native.stdout.replace('\r\n', '\n'):
            raise ValueError('external fixture dispatch/count was not observed')
        observed = re.findall(r'^SDK_CASE\n([^\n]+)\n(-?\d+)\n(-?\d+)\n',
                              native.stdout.replace('\r\n','\n'), re.MULTILINE)
        expected = json.loads(cases.read_text(encoding='utf-8'))
        wanted = [(row['name'], str(row['expected']), str(row.get('artifact',{}).get('expected',-1))) for row in expected]
        if observed != wanted or len({row[0] for row in observed}) != len(wanted):
            raise ValueError('per-fixture ID/expected diagnostic execution differs from complete prepared corpus')
        report['executed_cases'] = [{'id':name,'expected_error':int(code),'expected_artifact_error':int(artifact)} for name,code,artifact in observed]
        report['artifact_sha256'] = sha(exe)
        report['resource_rounds'] = 20
        report['resource_kinds'] = ['String handles', 'Buffer handles', 'opaque handles',
                                    'FileSnapshot handles', 'FileSnapshot retained bytes']
        report['additional_native_checks'] = ['strict JSON duplicate/escape/Unicode/integer/limit cases',
            'SDK entry record nine-field poisoning', 'C calling convention poisoning',
            'SDK binding declaration/source-identity corruption', 'readonly type-arena stability']
        report['result'] = 'passed-inventory-development-admission-not-final-sdk'
    except Exception as error:
        report['result'] = 'failed'
        report['failure'] = str(error)
    finally:
        report['source_revision_after'] = revision()
        report['inputs_after'] = {p:sha(Path(p)) for p in before if Path(p).is_file()}
        report['source_changes'] = [p for p,h in before.items() if not Path(p).is_file() or sha(Path(p)) != h]
        report['tool_changes'] = [p for p,h in tools.items() if sha(Path(p)) != h]
        if report['source_changes'] or report['tool_changes']:
            report['result'] = 'failed'
        report['completed_at'] = dt.datetime.now(dt.timezone.utc).isoformat()
        save()
    print(report['result'], flush=True)
    return 0 if report['result'].startswith('passed-') else 1


if __name__ == '__main__':
    raise SystemExit(main())
