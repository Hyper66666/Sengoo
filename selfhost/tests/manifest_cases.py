"""Canonical manifest syntax/schema cases; Rust oracle supplies actual policy."""
from __future__ import annotations
import argparse,hashlib,json
from pathlib import Path
BASE='[package]\nname="demo"\nversion="1.2.3"\n'
def main():
    p=argparse.ArgumentParser();p.add_argument('--output',type=Path,required=True);a=p.parse_args();a.output.mkdir(parents=True,exist_ok=False);rows=[]
    def add(name,text):
        file=a.output/(str(len(rows)).zfill(4)+'-'+name+'.toml');file.write_bytes(text.encode('utf8'));rows.append({'id':len(rows),'name':name,'path':file.name,'sha256':hashlib.sha256(file.read_bytes()).hexdigest()})
    add('minimal',BASE)
    add('all-fields','''sengoo-schema=1
[package]
name='demo'
version='1.2.3-alpha.1+build.000'
authors=['A','B']
description="""first
second"""
edition='2026'
[bin]
name='app'
path='src/run.sg'
[lib]
path='src/library.sg'
[[test]]
path='tests/a.sg'
[[test]]
path='tests/b.sg'
[registries.local]
path='../registry'
[registries.remote]
url='https://example.invalid'
token_env='TOKEN'
[dependencies]
a='^1.0'
b={package='other',path='../other'}
c={git='file:///repo',rev='main'}
d={version='>=1, <2',registry='remote'}
''')
    add('inline-root',"package={name='demo',version='1.2.3',authors=['a','b']}\nbin={}\nlib={}\ntest=[{path='a.sg'},{path='b.sg'}]\nregistries={local={path=''}}\ndependencies={a={version='1'}}\n")
    add('dotted-root',"package.name='demo'\npackage.version='1.2.3'\npackage.authors=['a','b']\ndependencies.a.path='../a'\n")
    add('quoted-keys',"['package']\n\"na\\u006de\"='demo'\n'version'='1.2.3'\n['registries'.'local-reg']\npath=''\n")
    add('basic-escapes',BASE+'authors=["a\\t\\u4e2d\\U0001f642", "quote\\\"", "slash\\\\", "null\\u0000"]\n')
    add('multiline-literal',BASE+"description='''\nline1\n'quoted'\n''body'''''\n")
    add('multiline-basic',BASE+'description="""\nThe quick brown \\\n  fox jumps over \\\n\n  the lazy dog.\n"""\n')
    add('trailing-array-comma',BASE+"authors=[\n 'a',# comment\n 'b',\n]\n")
    add('nested-table-order',BASE+"[dependencies.a]\npath='../a'\n[registries.default]\nurl='https://example.invalid'\n")
    add('crlf',BASE.replace('\n','\r\n')+"description=\"\"\"\r\nline\r\nnext\"\"\"\r\n")
    add('bom','\ufeff'+BASE)
    add('package-sequence',"package=['demo','1.2.3']\n")
    add('all-struct-sequences',"package=['demo','1.2.3',['A','B'],'desc','2026']\nbin=['app','p.sg']\nlib=['lib.sg']\ntest=[['a.sg'],['b.sg']]\nregistries={local=['../registry']}\n")
    add('default-struct-sequences',"package=['demo','1.2.3']\nbin=[]\nlib=[]\n")
    add('long-package-sequence',"package=['demo','1.2.3',[],'desc','2026','extra']\n")
    add('short-package-sequence',"package=['demo']\n")
    for value in ['1','+1','0x1','0o1','0b1','0_1','01','1.0','true','2','-1','99']:
        add('schema-'+str(len(rows)),"sengoo-schema="+value+'\n'+BASE)
    for value in ['1.2.3','0.0.0','18446744073709551615.0.0','18446744073709551616.0.0','1.2.3-a.1+b.00','1.2.3-01','1.2.3+01','01.2.3','1.02.3','1.2','1.2.3.4','1.2.3-','1.2.3+','1.2.3--','1.2.3-0','1.2.3-00a','1.2.3-0.01','1.2.3-ä',' 1.2.3','1.2.3 ']:
        add('version-'+str(len(rows)),BASE.replace('1.2.3',value))
    for value in ['*','x','X','1','1.2','1.2.3','1.*','1.x.*','1.*.2','^1.2.3','~1.2','>=1, <2','=1.*','1.2.3-alpha+build','* ,1','1 || 2','1 - 2','1.2.3,','', '1\t', '1.2.3-01','1.2.3+01','\u20031\u2003']:
        add('requirement-'+str(len(rows)),BASE+'[dependencies]\na='+json.dumps(value,ensure_ascii=False)+'\n')
    invalid=[('missing-package',"[bin]\npath='a'\n"),('missing-name',"[package]\nversion='1.0.0'"),('missing-version',"[package]\nname='demo'"),('empty-name',BASE.replace('demo','')),('bad-name',BASE.replace('demo','Demo')),('edition',BASE+"edition='2027'\n"),('unknown-package',BASE+"unknown='x'\n"),('authors-type',BASE+"authors=['a',1]\n"),('description-type',BASE+"description=1\n"),('unknown-root',BASE+'[workspace]\n'),('duplicate-field',BASE+"name='other'\n"),('duplicate-table',BASE+'[package]\n'),('dotted-redefinition',"package.name='demo'\n[package]\nversion='1.2.3'\n"),('sealed-inline',"package={name='demo',version='1.2.3'}\npackage.edition='2026'\n"),('literal-newline',BASE+"description='a\nb'\n"),('bad-escape',BASE+'description="\\q"\n'),('surrogate-escape',BASE+'description="\\uD800"\n'),('single-cr',BASE+'\r'),('inline-trailing',BASE+"[dependencies]\na={path='a',}\n"),('bin-name',BASE+"[bin]\nname='App'\n"),('test-missing-path',BASE+'[[test]]\n'),('registry-double',BASE+"[registries.a]\npath='a'\nurl='b'\n"),('registry-empty',BASE+"[registries.a]\n"),('registry-token',BASE+"[registries.a]\npath='a'\ntoken_env='TOKEN'\n"),('registry-boundary',BASE+"[registries.'.a']\npath='a'\n"),('dep-double',BASE+"[dependencies]\na={path='a',version='1'}\n"),('dep-empty-path',BASE+"[dependencies]\na={path=' '}\n"),('dep-rev',BASE+"[dependencies]\na={path='a',rev='x'}\n"),('dep-registry',BASE+"[dependencies]\na={path='a',registry='local'}\n"),('dep-unknown',BASE+"[dependencies]\na={git='a',features=['x']}\n"),('dep-wrong-field',BASE+"[dependencies]\na={git='a',rev=3}\n")]
    for name,text in invalid:add(name,text)
    threshold=(1<<1024)-(1<<970)
    ignored=['true','false','garbage','1.0','1e3','+1.2e3','1e309','-1e309','0e99999','1e-99999','inf','-inf','+nan','NaN','1.','01.0','1_0.0_1','0_0.1','1.0_','1e_3','1e+3',str(threshold-1)+'.0',str(threshold)+'.0',str(threshold+1)+'.0','-'+str(threshold)+'.0','9223372036854775807','9223372036854775808','-9223372036854775808','-9223372036854775809','0x7fffffffffffffff','0x8000000000000000','1979-05-27','0000-02-29','2024-02-29','2023-02-29','2000-02-29','1900-02-29','07:32:00','07:32:60','07:32:61','07:32:00.123456789123','07:32:00Z','1979-05-27T07:32:00Z','1979-05-27t07:32:00z','1979-05-27 07:32:00-07:00','1979-05-27T07:32:00+23:59','1979-05-27T07:32:00+24:00','1979-05-27T07:32:00.','[]','{}','[1,true,2.5]']
    for index,value in enumerate(ignored):add('ignored-sequence-tail-'+str(index),"package=['demo','1.2.3',[],'desc','2026',"+value+']\n')
    (a.output/'manifest.json').write_text(json.dumps({'schema':1,'cases':rows},indent=2)+'\n');print(json.dumps({'schema':1,'cases':len(rows)}))
if __name__=='__main__':main()
