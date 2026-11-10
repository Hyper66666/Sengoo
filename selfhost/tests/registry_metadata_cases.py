"""Independent serde-compatible metadata structure and JSON syntax inputs."""
import argparse,hashlib,json
from pathlib import Path
def main():
    p=argparse.ArgumentParser();p.add_argument('--output',required=True,type=Path);a=p.parse_args();a.output.mkdir(parents=True,exist_ok=False);rows=[]
    def add(name,text):
        file=a.output/(str(len(rows)).zfill(3)+'-'+name+'.json');file.write_bytes(text.encode('utf8')if isinstance(text,str)else text);rows.append({'id':len(rows),'name':name,'path':file.name,'sha256':hashlib.sha256(file.read_bytes()).hexdigest()})
    base={'checksum':'a'*64,'yanked':False,'yank_reason':None,'features':['one','two']}
    add('complete',json.dumps(base));add('minimal','{"yanked":false,"features":[]}');add('sequence','[null,false,null,["one","two"]]')
    add('sequence-extra','[null,false,null,[],1]');add('sequence-short','[null,false]')
    for key in base:
        other=dict(base);del other[key];add('missing-'+key,json.dumps(other))
    for key in base:
        for value in [None,False,1,[],{},'string']:
            other=dict(base);other[key]=value;add('type-'+key+'-'+str(len(rows)),json.dumps(other))
    add('unicode-pair','{"yanked":false,"features":["\\ud83d\\ude42"],"yank_reason":"中"}')
    add('all-string-escapes',json.dumps({'yanked':False,'features':['\b\t\n\f\r\u0001\u001f\u007f"\\/中🙂'],'yank_reason':'end\u0000'}))
    add('known-surrogate-high','{"yanked":false,"features":["\\ud800"]}')
    add('known-surrogate-low','{"yanked":false,"features":[],"yank_reason":"\\udfff"}')
    add('escaped-slash','{"yanked":false,"features":["a\\/b"]}')
    add('null-checksum','{"yanked":true,"features":[],"checksum":null}')
    add('duplicate-known','{"yanked":false,"yanked":true,"features":[]}')
    add('duplicate-unknown','{"extra":0,"extra":1,"yanked":false,"features":[]}')
    add('unknown-complex','{"yanked":false,"features":[],"extra":[null,true,false,1.2e999,{"nested":[1,-0.1]}]}')
    for name,text in [('surrogate-high','"\\ud800"'),('surrogate-low','"\\udfff"'),('surrogate-pair-bad','"\\ud800\\u0020"'),('upper-U','"\\U0001f642"'),('raw-tab','"a\tb"'),('raw-newline','"a\nb"'),('bad-escape','"\\q"'),('bad-number','01'),('bad-exponent','1e'),('bad-keyword','TRUE'),('trailing-comma','[1,]')]:
        add(name,'{"yanked":false,"features":[],"extra":'+text+'}')
    add('bom','\ufeff{"yanked":false,"features":[]}');add('trailing','{"yanked":false,"features":[]}x')
    add('ignored-raw-invalid-utf8',b'{"yanked":false,"features":[],"extra":"\xff"}')
    add('known-raw-invalid-utf8',b'{"yanked":false,"features":["\xff"]}')
    add('key-raw-invalid-utf8',b'{"yanked":false,"features":[],"\xff":0}')
    (a.output/'manifest.json').write_text(json.dumps({'schema':1,'cases':rows},indent=2)+'\n');print(json.dumps({'schema':1,'cases':len(rows)}))
if __name__=='__main__':main()
