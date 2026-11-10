use std::{env,fs,io::{Cursor,Read}};
fn number(raw:&[u8],at:&mut usize,width:usize)->u64 {
    assert!(width<=8 && *at+width<=raw.len());let mut result=0;
    for index in 0..width{result|=(raw[*at+index]as u64)<<(index*8);}*at+=width;result
}
fn hex(raw:&[u8])->String{raw.iter().map(|b|format!("{b:02x}")).collect()}
fn observe(input:&[u8])->Result<String,String>{
    let mut archive=tar::Archive::new(Cursor::new(input));let mut rows=Vec::new();
    for entry in archive.entries().map_err(|e|e.to_string())? {
        let mut entry=entry.map_err(|e|e.to_string())?;
        let _windows_path=entry.path().map_err(|e|e.to_string())?;
        let name=hex(&entry.path_bytes());let kind=entry.header().entry_type().as_byte();
        let declared=entry.header().size().map_err(|e|e.to_string())?;let size=entry.size();
        let header=entry.raw_header_position();let data=entry.raw_file_position();
        let mut contents=Vec::new();entry.read_to_end(&mut contents).map_err(|e|e.to_string())?;
        rows.push(format!("{name},{kind},{declared},{size},{header},{data},{}",hex(&contents)));
    }
    Ok(rows.join(";"))
}
fn main(){
    let raw=fs::read(env::args().nth(1).expect("corpus")).expect("read corpus");
    assert_eq!(&raw[..8],b"SGTAR1\0\0");let mut at=8;let count=number(&raw,&mut at,4);
    for id in 0..count {
        let length=number(&raw,&mut at,4)as usize;let _tc=number(&raw,&mut at,4);let _pc=number(&raw,&mut at,4);
        let _expected=number(&raw,&mut at,8);let table=number(&raw,&mut at,4)as usize;let paths=number(&raw,&mut at,4)as usize;
        let input=&raw[at..at+length];at+=length+table+paths;
        match observe(input){Ok(value)=>println!("{id}|true|{value}"),Err(error)=>println!("{id}|false|{}",hex(error.as_bytes()))};
    }
    assert_eq!(at,raw.len());
}
