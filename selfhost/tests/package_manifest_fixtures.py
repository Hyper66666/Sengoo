"""Build exact canonical manifest and independent SHA-256 fixture inventories."""
import argparse,ast,hashlib,json,random,re,struct,subprocess
from pathlib import Path

def main():
    p=argparse.ArgumentParser();p.add_argument('--root',required=True,type=Path);p.add_argument('--git',required=True,type=Path);p.add_argument('--constructed',required=True,type=Path);p.add_argument('--output',required=True,type=Path);a=p.parse_args();a.output.mkdir(parents=True,exist_ok=False);rows=[];sources={}
    def add(name,raw,source=None):
        file=a.output/(str(len(rows)).zfill(4)+'.toml');file.write_bytes(raw);row={'id':len(rows),'name':name,'path':file.name,'sha256':hashlib.sha256(raw).hexdigest()}
        if source is not None:row['source']=str(source.resolve());sources[str(source.resolve())]=row['sha256']
        rows.append(row)
    constructed=json.loads((a.constructed/'manifest.json').read_text())
    for row in constructed['cases']:
        file=a.constructed/row['path'];raw=file.read_bytes()
        if hashlib.sha256(raw).hexdigest()!=row['sha256']:raise ValueError('constructed source identity differs')
        add(row['name'],raw)
    command=[str(a.git),'ls-files','-z','--','**/Sengoo.toml','Sengoo.toml'];actual=subprocess.run(command,cwd=a.root,capture_output=True)
    (a.output/'git.stdout').write_bytes(actual.stdout);(a.output/'git.stderr').write_bytes(actual.stderr)
    if actual.returncode or actual.stderr:raise ValueError('actual Git manifest inventory failed')
    names=actual.stdout.decode().split('\0')
    repository=0
    for name in sorted(set(names)):
        if not name:continue
        source=a.root/name;add('repository:'+name,source.read_bytes(),source);repository+=1
    source=a.root/'tools/sgpm/src/manifest.rs';text=source.read_text();sources[str(source.resolve())]=hashlib.sha256(source.read_bytes()).hexdigest();embedded=0;skipped=[]
    pattern=re.compile(r'(?:Manifest|WorkspaceManifest)::parse\(\s*(r(?P<hash>#+)?".*?"(?P=hash)|"(?:\\.|[^"\\])*")',re.S)
    for match in pattern.finditer(text):
        literal=match[1]
        if literal.startswith('r'):
            quote=literal.index('"');value=literal[quote+1:len(literal)-(quote-1)-1]
        else:
            try:value=ast.literal_eval(literal)
            except (SyntaxError,ValueError):skipped.append(text[:match.start()].count('\n')+1);continue
        if not isinstance(value,str):raise ValueError('canonical fixture was not a string')
        add('canonical-embedded:'+str(text[:match.start()].count('\n')+1),value.encode('utf8'));embedded+=1
    manifest={'schema':1,'constructed':len(constructed['cases']),'repository':repository,'canonical_embedded':embedded,'skipped_rust_literal_lines':skipped,'sources':sources,'cases':rows,'git_inventory':{'command':command,'exit':actual.returncode,'stdout_sha256':hashlib.sha256(actual.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(actual.stderr).hexdigest()}}
    (a.output/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
    rng=random.Random(76131);base=['1.2.3','0.0.0','18446744073709551615.0.0','1.2.3-alpha.1+build','*','1.*','1.x.*','>=1, <2','^0.2','1.2.3-01','1.2.3+01'];values=set(base)
    for _ in range(220):
        text=rng.choice(base);at=rng.randrange(len(text)+1);values.add(text[:at]+rng.choice(['0','1','9','.','-','+','a','x','*',' ',',','\t'])+text[at:])
    (a.output/'semver.json').write_text(json.dumps({'schema':1,'values':sorted(values)},indent=2)+'\n')
    rng=random.Random(76131);cases=[b'',b'abc',b'a'*1000000]+[bytes(rng.randrange(256)for _ in range(n))for n in range(140)]
    # Different path/content decompositions must have different exact framed
    # bytes even where plain concatenation would collide structurally.
    for name,data in [(b'a',b'bc'),(b'ab',b'c'),(b'a\0',b'bc'),(b'a',b'b\0c')]:cases.append(struct.pack('<Q',len(name))+name+struct.pack('<Q',len(data))+data)
    corpus=bytearray(b'SGSHA1\0\0'+struct.pack('<I',len(cases)));runs=0
    for raw in cases:corpus+=struct.pack('<I',len(raw))+raw+hashlib.sha256(raw).digest();runs+=3+(len(raw)-1 if 0<len(raw)<=128 else 0)
    (a.output/'sha256.bin').write_bytes(corpus);(a.output/'sha256.json').write_text(json.dumps({'schema':1,'cases':len(cases),'runs':runs,'sha256':hashlib.sha256(corpus).hexdigest(),'known_vectors':['empty','abc','one million a'],'framing_cases':4},indent=2)+'\n')
    print(json.dumps({'schema':1,'manifests':len(rows),'repository':repository,'canonical_embedded':embedded,'semver_values':len(values),'sha256_cases':len(cases),'sha256_runs':runs}))
if __name__=='__main__':main()
