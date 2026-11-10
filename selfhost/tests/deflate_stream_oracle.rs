use std::{env,fs};
fn number(raw:&[u8],at:&mut usize,width:usize)->u64{assert!(*at+width<=raw.len());let mut value=0;for i in 0..width{value|=(raw[*at+i]as u64)<<(i*8);}*at+=width;value}
fn hex(raw:&[u8])->String{raw.iter().map(|b|format!("{b:02x}")).collect()}
fn main(){
    let raw=fs::read(env::args().nth(1).expect("corpus")).expect("read corpus");assert_eq!(&raw[..8],b"SGDFL1\0\0");let mut at=8;let count=number(&raw,&mut at,8);
    for id in 0..count{
        let length=number(&raw,&mut at,4)as usize;let _capacity=number(&raw,&mut at,4);let _expected=number(&raw,&mut at,8);let plain=number(&raw,&mut at,4)as usize;
        let source=&raw[at..at+length];at+=length+plain;
        match miniz_oxide::inflate::decompress_to_vec(source){Ok(bytes)=>println!("{id}|true|{}",hex(&bytes)),Err(error)=>println!("{id}|false|{:?}",error.status)};
    }
    assert_eq!(at,raw.len());
}
