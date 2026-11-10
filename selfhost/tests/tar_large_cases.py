"""Independent tarfile headers and expected readable extents, without giant corpus copies."""
from pathlib import Path
import argparse,hashlib,json,tarfile
def main():
    p=argparse.ArgumentParser();p.add_argument('--output',type=Path,required=True);a=p.parse_args();a.output.mkdir(parents=True,exist_ok=False)
    rows=[]
    for name,size in [('first',268435456),('second-zero',0),('second-one',1)]:
        entry=tarfile.TarInfo('a' if name=='first' else 'b');entry.size=size
        data=entry.tobuf(tarfile.GNU_FORMAT);parsed=tarfile.TarInfo.frombuf(data,'utf8','strict')
        assert parsed.size==size and parsed.name==entry.name and parsed.type==tarfile.REGTYPE
        path=a.output/(name+'.header');path.write_bytes(data);rows.append({'name':name,'path':str(path.resolve()),'sha256':hashlib.sha256(data).hexdigest(),'size':size})
    report={'schema':1,'headers':rows,'cases':[{'name':'content-exact-256MiB','length':268437504,'expected':2,'content':268435456},{'name':'content-over-256MiB','length':268438016,'expected':-7},{'name':'raw-input-exact-512MiB','length':536870912,'expected':2,'content':268435456}],'scope':'readable zero-initialized extents; payload can be skipped by scanner; no all-byte-read claim'}
    (a.output/'manifest.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'schema':1,'cases':3}))
if __name__=='__main__':main()
