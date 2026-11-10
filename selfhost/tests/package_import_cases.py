"""Archive/identity/metadata inputs for the admitted SG transaction."""
import argparse,gzip,hashlib,json,tarfile,zlib
from pathlib import Path
from package_stream_cases import archive

def main():
    p=argparse.ArgumentParser();p.add_argument('--output',required=True,type=Path);p.add_argument('--large',action='store_true');a=p.parse_args();a.output.mkdir(parents=True,exist_ok=False);rows=[]
    manifest=b'[package]\nname="demo"\nversion="1.0.0"\n'
    entries=[('Sengoo.toml',manifest,tarfile.REGTYPE),('src/main.sg',b'def main()->i64{42;}\n',tarfile.REGTYPE),('assets/中.txt',b'asset\x00bytes',tarfile.REGTYPE)]
    def add(name,items=None,wire=None,expected=0,pkg='demo',version='1.0.0',badsha=False,metadata=None,metadata_text=None,attack='none',oom=-1,allow=0,cancel=-1,existing=False):
        items=entries if items is None else items
        if wire is None:wire=gzip.compress(archive(items),mtime=0)
        actual=hashlib.sha256(wire).hexdigest();pin='0'*64 if badsha else actual
        value={'checksum':actual,'yanked':False,'yank_reason':None,'features':['alpha','中','line\n']}
        if metadata is not None:value.update(metadata)
        text=json.dumps(value,ensure_ascii=False)if metadata_text is None else metadata_text
        stem=str(len(rows)).zfill(3)+'-'+name;source=a.output/(stem+'.tar.gz');meta=a.output/(stem+'.json');source.write_bytes(wire);meta.write_text(text,encoding='utf8')
        rows.append({'id':len(rows),'name':name,'archive':source.name,'archive_sha256':actual,'metadata':meta.name,'metadata_sha256':hashlib.sha256(meta.read_bytes()).hexdigest(),'expected':expected,'expected_name':pkg,'expected_version':version,'expected_archive_sha256':pin,'allow_yanked':allow,'attack':attack,'oom':oom,'cancel_after':cancel,'existing_target':existing,'source_files':{n:hashlib.sha256(b).hexdigest()for n,b,k in items if k==tarfile.REGTYPE}})
    add('normal');add('wrong-name',pkg='other',expected=-24);add('wrong-version',version='1.0.1',expected=-24);add('bad-expected-version',version='01.0.0',expected=-24);add('wrong-archive-sha',badsha=True,expected=-25)
    add('metadata-sha',metadata={'checksum':'f'*64},expected=-25);add('metadata-no-sha',metadata={'checksum':None},expected=-25);add('metadata-yanked',metadata={'yanked':True},expected=-27);add('metadata-yanked-explicit',metadata={'yanked':True},allow=1)
    add('metadata-malformed',metadata_text='{',expected=-23);add('metadata-wrong-type',metadata={'features':[1]},expected=-23)
    for i in range(4,7):add('admission-oom-'+str(i),oom=i,expected=-8)
    add('marker-write-denied',attack='marker-readonly',expected=-8);add('existing-final',existing=True,expected=-8);add('final-race',attack='target',expected=-8);add('replaced-after-admission',attack='replace',expected=-11)
    for name,source,expected in [('missing-name',b'[package]\nversion="1.0.0"\n',-21),('malformed',b'[package\n',-20),('invalid-utf8',b'\xff',-20),('identity',manifest.replace(b'demo',b'wrong'),-24),('entry-missing',manifest+b'[bin]\npath="src/missing.sg"\n',-26),('entry-absolute',manifest+b'[bin]\npath="C:/outside.sg"\n',-26),('entry-parent',manifest+b'[bin]\npath="../outside.sg"\n',-26),('entry-directory',manifest+b'[bin]\npath="src"\n',-26),('entry-normalized',manifest+b'[bin]\npath="src/../SRC/main.sg"\n',0)]:
        add(name,[(n,source if n=='Sengoo.toml'else b,k)for n,b,k in entries],expected=expected)
    add('missing-root',[x for x in entries if x[0]!='Sengoo.toml'],expected=-24)
    add('duplicate-root',entries+[('SENGOO.TOML',manifest,tarfile.REGTYPE)],expected=-6)
    add('nested-manifest',entries+[('nested/Sengoo.toml',b'[package]\nname="nested"\nversion="0.1.0"\n',tarfile.REGTYPE)])
    add('root-manifest-case',[(n.upper()if n=='Sengoo.toml'else n,b,k)for n,b,k in entries])
    for marker in ['.sgpm-archive.sha256','.sgpm-content.sha256','.sgpm-registry-metadata.json']:
        add('reserved-'+str(len(rows)),entries+[(marker.upper(),b'foreign',tarfile.REGTYPE)],expected=-4)
        add('reserved-prefix-'+str(len(rows)),entries+[(marker+'/child',b'foreign',tarfile.REGTYPE)],expected=-4)
    add('dotfiles',entries+[('.hidden',b'hidden',tarfile.REGTYPE),('.config/a',b'config',tarfile.REGTYPE)])
    add('nested-marker-names',entries+[('nested/.sgpm-archive.sha256',b'included archive spelling',tarfile.REGTYPE),('nested/.sgpm-content.sha256',b'included content spelling',tarfile.REGTYPE)])
    add('parent-case',[('Sengoo.toml',manifest,tarfile.REGTYPE),('Src/main.sg',b'main',tarfile.REGTYPE),('src/other.sg',b'other',tarfile.REGTYPE)])
    add('unicode-parent-width',entries+[('Ⱥ/a',b'a',tarfile.REGTYPE),('ⱥ/b',b'b',tarfile.REGTYPE)])
    wire=gzip.compress(archive(entries),mtime=0);bad=bytearray(wire);bad[-8]^=1;add('late-crc',wire=bytes(bad),expected=-1);add('late-crc-plus-identity',wire=bytes(bad),pkg='wrong',expected=-1)
    empty=bytearray(gzip.compress(b'',mtime=0));empty[-8]^=1;add('late-member',wire=wire+empty,expected=-1)
    cancel_items=entries+[('large.bin',b'0123456789abcdef'*65536,tarfile.REGTYPE)]
    cancel_wire=gzip.compress(archive(cancel_items),mtime=0)
    if len(cancel_wire)<=226 or len(zlib.decompressobj(31).decompress(cancel_wire[:226]))<=512+len(manifest):raise ValueError('cancellation fixture did not independently reach payload before completion')
    add('cancel-after-write',cancel_items,wire=cancel_wire,cancel=2,expected=-9)
    if a.large:
        allowance=268435456-sum(len(data)for _,data,kind in entries if kind==tarfile.REGTYPE)
        add('content-exact-256MiB',entries+[('large.bin',b'A'*allowance,tarfile.REGTYPE)])
    (a.output/'manifest.json').write_text(json.dumps({'schema':1,'scope':'target engine proof, not public sgpm replacement','cases':rows},indent=2,ensure_ascii=False)+'\n',encoding='utf8');print(json.dumps({'schema':1,'cases':len(rows)}))
if __name__=='__main__':main()
