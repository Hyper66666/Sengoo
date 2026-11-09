use std::{env,fs,io::{Cursor,Read}};
fn number(raw:&[u8],at:&mut usize,width:usize)->u64 {
    let mut result=0;for index in 0..width{result|=(raw[*at+index]as u64)<<(index*8);}*at+=width;result
}
fn hex(raw:&[u8])->String{raw.iter().map(|b|format!("{b:02x}")).collect()}
fn main(){
    let raw=fs::read(env::args().nth(1).expect("corpus")).expect("read corpus");
    assert_eq!(&raw[..8],b"SGGZP1\0\0");let mut at=8;let count=number(&raw,&mut at,8);
    for id in 0..count {
        let length=number(&raw,&mut at,4) as usize;let _capacity=number(&raw,&mut at,4);let _expected=number(&raw,&mut at,8);let plain=number(&raw,&mut at,4) as usize;let _mode=number(&raw,&mut at,4);
        let input=&raw[at..at+length];at+=length+plain;
        let mut first=Vec::new();let a=flate2::read::GzDecoder::new(Cursor::new(input)).read_to_end(&mut first);
        let mut multi=Vec::new();let b=flate2::read::MultiGzDecoder::new(Cursor::new(input)).read_to_end(&mut multi);
        println!("{id}|{}|{}|{}|{}",a.is_ok(),b.is_ok(),if a.is_ok(){hex(&first)}else{String::new()},if b.is_ok(){hex(&multi)}else{String::new()});
    }
    assert_eq!(at,raw.len());
}
