"""Emit fixed data/expected-metadata leaf SDK fixtures, never a tar implementation."""
from pathlib import Path
import argparse,json,struct
def main():
    p=argparse.ArgumentParser();p.add_argument('--corpus',required=True,type=Path);p.add_argument('--output',required=True,type=Path);a=p.parse_args()
    raw=a.corpus.read_bytes();manifest=json.loads(a.corpus.with_suffix('.json').read_text(encoding='utf8'));assert raw[:8]==b'SGTAR1\0\0';at=12;fixtures={}
    for row in manifest['fixtures']:
        n,tc,pc,result,tlen,plen=struct.unpack_from('<IIIqII',raw,at);at+=28
        source=raw[at:at+n];at+=n;table=raw[at:at+tlen];at+=tlen;paths=raw[at:at+plen];at+=plen
        fixtures[row['name']]=(source,table,paths,result)
    assert at==len(raw)
    selected=['gnu','ustar','pax','ustar-prefix','gnu-long-4','pax-long-5','old-v7-valid','normalize','unsafe-2f6162736f6c757465','collision-34','windows-whole-path-10']
    # Long-name IDs are fixture-order labels; select their real names explicitly.
    longnames=[row['name'] for row in manifest['fixtures'] if row['name'].startswith(('gnu-long-','pax-long-'))]
    selected[4:6]=longnames[:2]
    lines=['#include <stdint.h>','static uint8_t table_buffer[4096]={1};','static uint8_t path_buffer[131072]={1};']
    for index,name in enumerate(selected):
        source,table,paths,result=fixtures[name]
        for label,value in [('input',source),('expected_table',table),('expected_paths',paths)]:
            lines.append('static const uint8_t '+label+str(index)+'[]={'+','.join(map(str,value or b'\0'))+'};')
    lines+=['uint8_t *sengoo_tar_fixture_table(void){return table_buffer;}','uint8_t *sengoo_tar_fixture_paths(void){return path_buffer;}','uint8_t sengoo_tar_fixture_table_byte(int64_t index){return table_buffer[index];}','uint8_t sengoo_tar_fixture_path_byte(int64_t index){return path_buffer[index];}']
    for label,array in [('input','fixture_inputs'),('expected_table','fixture_tables'),('expected_paths','fixture_paths')]:
        lines.append('static const uint8_t *const '+array+'[]={'+','.join(label+str(i) for i in range(len(selected)))+'};')
    for label,index in [('fixture_lengths',0),('fixture_table_lengths',1),('fixture_path_lengths',2)]:
        lines.append('static const int64_t '+label+'[]={'+','.join(str(len(fixtures[name][index])) for name in selected)+'};')
    lines.append('static const int64_t fixture_results[]={'+','.join(str(fixtures[name][3]) for name in selected)+'};')
    lines+=['const uint8_t *sengoo_tar_fixture_input(int64_t kind){return fixture_inputs[kind];}',
            'int64_t sengoo_tar_fixture_length(int64_t kind){return fixture_lengths[kind];}',
            'int64_t sengoo_tar_fixture_result(int64_t kind){return fixture_results[kind];}',
            'int64_t sengoo_tar_fixture_table_length(int64_t kind){return fixture_table_lengths[kind];}',
            'int64_t sengoo_tar_fixture_path_length(int64_t kind){return fixture_path_lengths[kind];}',
            'uint8_t sengoo_tar_fixture_expected_table(int64_t kind,int64_t index){return fixture_tables[kind][index];}',
            'uint8_t sengoo_tar_fixture_expected_path(int64_t kind,int64_t index){return fixture_paths[kind][index];}']
    a.output.write_text('\n'.join(lines)+'\n',encoding='ascii');print(json.dumps({'schema':1,'fixtures':selected,'count':len(selected)}))
if __name__=='__main__':main()
