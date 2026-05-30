use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use super::{
    classify_io_error, copy_bytes_to_buffer, fail_bool, fail_handle, fail_i64, net_runtime,
    open_stream, parse_http_headers, parse_url, reset_last_error, set_last_error,
    split_http_headers_and_body, NetErrorCode, NetRuntime, ParsedUrl,
};

impl NetRuntime {
    pub(crate) fn ws_store(&self, stream: TcpStream) -> Result<u64, NetErrorCode> {
        let handle = self.alloc_handle();
        self.ws_streams
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?
            .insert(handle, stream);
        Ok(handle)
    }

    pub(crate) fn ws_send_text(&self, handle: u64, payload: &[u8]) -> Result<i64, NetErrorCode> {
        let mut table = self
            .ws_streams
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?;
        let stream = table.get_mut(&handle).ok_or(NetErrorCode::HandleNotFound)?;
        ws_write_frame(stream, 0x1, payload, true)
            .map(|_| payload.len() as i64)
            .map_err(|err| classify_io_error(&err))
    }

    pub(crate) fn ws_recv_text(
        &self,
        handle: u64,
        buffer: *mut u8,
        capacity: usize,
        timeout_ms: u32,
    ) -> Result<i64, NetErrorCode> {
        let mut table = self
            .ws_streams
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?;
        let stream = table.get_mut(&handle).ok_or(NetErrorCode::HandleNotFound)?;
        if timeout_ms != 0 {
            stream
                .set_read_timeout(Some(Duration::from_millis(timeout_ms as u64)))
                .map_err(|err| classify_io_error(&err))?;
        }
        loop {
            let Some((opcode, payload)) = ws_read_frame(stream) else {
                return Err(NetErrorCode::WebSocketProtocolError);
            };
            match opcode {
                0x1 => return Ok(copy_bytes_to_buffer(&payload, buffer, capacity)),
                0x9 => {
                    ws_write_frame(stream, 0xA, &payload, true)
                        .map_err(|err| classify_io_error(&err))?;
                }
                0x8 => {
                    set_last_error(NetErrorCode::RemoteClosed);
                    return Ok(0);
                }
                0xA => {}
                _ => return Err(NetErrorCode::WebSocketProtocolError),
            }
        }
    }

    pub(crate) fn ws_close(&self, handle: u64) -> Result<i64, NetErrorCode> {
        let mut stream = self
            .ws_streams
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?
            .remove(&handle)
            .ok_or(NetErrorCode::HandleNotFound)?;
        ws_write_frame(&mut stream, 0x8, &[], true).map_err(|err| classify_io_error(&err))?;
        Ok(1)
    }
}

fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::new();
    let mut idx = 0usize;
    while idx < input.len() {
        let b0 = input[idx];
        let b1 = if idx + 1 < input.len() {
            input[idx + 1]
        } else {
            0
        };
        let b2 = if idx + 2 < input.len() {
            input[idx + 2]
        } else {
            0
        };
        let triple = ((b0 as u32) << 16) | ((b1 as u32) << 8) | (b2 as u32);
        output.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
        output.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);
        if idx + 1 < input.len() {
            output.push(TABLE[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            output.push('=');
        }
        if idx + 2 < input.len() {
            output.push(TABLE[(triple & 0x3F) as usize] as char);
        } else {
            output.push('=');
        }
        idx += 3;
    }
    output
}

fn sha1_digest(input: &[u8]) -> [u8; 20] {
    fn left_rotate(value: u32, bits: u32) -> u32 {
        value.rotate_left(bits)
    }

    let mut message = input.to_vec();
    let bit_len = (message.len() as u64) * 8;
    message.push(0x80);
    while (message.len() % 64) != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_be_bytes());

    let mut h0: u32 = 0x67452301;
    let mut h1: u32 = 0xEFCDAB89;
    let mut h2: u32 = 0x98BADCFE;
    let mut h3: u32 = 0x10325476;
    let mut h4: u32 = 0xC3D2E1F0;

    for chunk in message.chunks(64) {
        let mut w = [0u32; 80];
        for (idx, word) in chunk.chunks(4).take(16).enumerate() {
            w[idx] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for idx in 16..80 {
            w[idx] = left_rotate(w[idx - 3] ^ w[idx - 8] ^ w[idx - 14] ^ w[idx - 16], 1);
        }

        let mut a = h0;
        let mut b = h1;
        let mut c = h2;
        let mut d = h3;
        let mut e = h4;

        for (idx, wi) in w.iter().enumerate() {
            let (f, k) = match idx {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let temp = left_rotate(a, 5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*wi);
            e = d;
            d = c;
            c = left_rotate(b, 30);
            b = a;
            a = temp;
        }

        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
        h4 = h4.wrapping_add(e);
    }

    let mut out = [0u8; 20];
    out[0..4].copy_from_slice(&h0.to_be_bytes());
    out[4..8].copy_from_slice(&h1.to_be_bytes());
    out[8..12].copy_from_slice(&h2.to_be_bytes());
    out[12..16].copy_from_slice(&h3.to_be_bytes());
    out[16..20].copy_from_slice(&h4.to_be_bytes());
    out
}

pub(super) fn websocket_accept_value(client_key: &str) -> String {
    let mut input = String::with_capacity(client_key.len() + 36);
    input.push_str(client_key.trim());
    input.push_str("258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
    base64_encode(&sha1_digest(input.as_bytes()))
}

pub(super) fn write_websocket_upgrade_response(
    stream: &mut TcpStream,
    accept: &str,
) -> Result<(), NetErrorCode> {
    let response = format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
        accept
    );
    stream
        .write_all(response.as_bytes())
        .map_err(|err| classify_io_error(&err))?;
    stream.flush().map_err(|err| classify_io_error(&err))
}

pub(super) fn run_ws_echo_session(stream: &mut TcpStream) -> Result<(), NetErrorCode> {
    loop {
        let Some((opcode, payload)) = ws_read_frame(stream) else {
            return Err(NetErrorCode::RemoteClosed);
        };
        match opcode {
            0x1 => ws_write_frame(stream, 0x1, &payload, false)
                .map_err(|err| classify_io_error(&err))?,
            0x9 => ws_write_frame(stream, 0xA, &payload, false)
                .map_err(|err| classify_io_error(&err))?,
            0x8 => {
                let _ = ws_write_frame(stream, 0x8, &payload, false);
                return Ok(());
            }
            0xA => {}
            _ => return Err(NetErrorCode::WebSocketProtocolError),
        }
    }
}

fn websocket_client_key() -> &'static str {
    // Base64("0123456789abcdef")
    "MDEyMzQ1Njc4OWFiY2RlZg=="
}

pub(super) fn read_http_response_headers(stream: &mut TcpStream) -> Result<Vec<u8>, NetErrorCode> {
    let mut bytes = Vec::new();
    let mut buf = [0u8; 1];
    while bytes.len() < 16 * 1024 {
        let n = stream
            .read(&mut buf)
            .map_err(|err| classify_io_error(&err))?;
        if n == 0 {
            break;
        }
        bytes.push(buf[0]);
        if bytes.ends_with(b"\r\n\r\n") {
            return Ok(bytes);
        }
    }
    Err(NetErrorCode::WebSocketHandshakeError)
}

fn websocket_connect(url: &ParsedUrl, timeout_ms: u32) -> Result<TcpStream, NetErrorCode> {
    if url.scheme != "ws" {
        return Err(NetErrorCode::UnsupportedScheme);
    }
    let mut stream = open_stream(&url.host, url.port, timeout_ms)?;
    let req = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {}\r\nSec-WebSocket-Version: 13\r\n\r\n",
        url.path,
        url.host,
        websocket_client_key()
    );
    stream
        .write_all(req.as_bytes())
        .map_err(|err| classify_io_error(&err))?;
    stream.flush().map_err(|err| classify_io_error(&err))?;

    let response = read_http_response_headers(&mut stream)?;
    let (header, _) = split_http_headers_and_body(&response)?;
    let (status_code, headers) = parse_http_headers(header)?;
    if status_code != 101 {
        return Err(NetErrorCode::WebSocketHandshakeError);
    }
    let upgrade_ok = headers
        .get("upgrade")
        .map(|v| v.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false);
    let connection_ok = headers
        .get("connection")
        .map(|v| v.to_ascii_lowercase().contains("upgrade"))
        .unwrap_or(false);
    let accept_ok = headers
        .get("sec-websocket-accept")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false);
    if !upgrade_ok || !connection_ok || !accept_ok {
        return Err(NetErrorCode::WebSocketHandshakeError);
    }
    Ok(stream)
}

pub(super) fn ws_write_frame(
    stream: &mut TcpStream,
    opcode: u8,
    payload: &[u8],
    masked: bool,
) -> std::io::Result<()> {
    let mut frame = Vec::with_capacity(payload.len() + 14);
    frame.push(0x80 | (opcode & 0x0F)); // FIN + opcode

    let mask_bit = if masked { 0x80 } else { 0x00 };
    if payload.len() <= 125 {
        frame.push(mask_bit | payload.len() as u8);
    } else if payload.len() <= 0xFFFF {
        frame.push(mask_bit | 126);
        frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    } else {
        frame.push(mask_bit | 127);
        frame.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    }

    if masked {
        let key = [0x13u8, 0x37, 0xC0, 0xDE];
        frame.extend_from_slice(&key);
        for (idx, byte) in payload.iter().enumerate() {
            frame.push(byte ^ key[idx % 4]);
        }
    } else {
        frame.extend_from_slice(payload);
    }

    stream.write_all(&frame)?;
    stream.flush()
}

pub(super) fn ws_read_frame(stream: &mut TcpStream) -> Option<(u8, Vec<u8>)> {
    let mut header = [0u8; 2];
    stream.read_exact(&mut header).ok()?;
    let opcode = header[0] & 0x0F;
    let masked = (header[1] & 0x80) != 0;

    let mut payload_len = (header[1] & 0x7F) as usize;
    if payload_len == 126 {
        let mut ext = [0u8; 2];
        stream.read_exact(&mut ext).ok()?;
        payload_len = u16::from_be_bytes(ext) as usize;
    } else if payload_len == 127 {
        let mut ext = [0u8; 8];
        stream.read_exact(&mut ext).ok()?;
        payload_len = u64::from_be_bytes(ext) as usize;
    }

    let mut mask_key = [0u8; 4];
    if masked {
        stream.read_exact(&mut mask_key).ok()?;
    }

    let mut payload = vec![0u8; payload_len];
    if payload_len > 0 {
        stream.read_exact(&mut payload).ok()?;
    }

    if masked {
        for (idx, byte) in payload.iter_mut().enumerate() {
            *byte ^= mask_key[idx % 4];
        }
    }
    Some((opcode, payload))
}

#[no_mangle]
pub extern "C" fn sengoo_ws_connect(url: *const u8, timeout_ms: u32) -> u64 {
    reset_last_error();
    let parsed = match parse_url(url) {
        Ok(parsed) => parsed,
        Err(code) => return fail_handle(code),
    };
    let stream = match websocket_connect(&parsed, timeout_ms) {
        Ok(stream) => stream,
        Err(code) => return fail_handle(code),
    };
    net_runtime().ws_store(stream).unwrap_or_else(fail_handle)
}

#[no_mangle]
/// # Safety
/// If `data` is non-null, it must point to `len` readable bytes.
pub unsafe extern "C" fn sengoo_ws_send_text(handle: u64, data: *const u8, len: usize) -> i64 {
    reset_last_error();
    if data.is_null() {
        return fail_i64(NetErrorCode::InvalidArgument);
    }
    let payload = unsafe { std::slice::from_raw_parts(data, len) };
    net_runtime()
        .ws_send_text(handle, payload)
        .unwrap_or_else(fail_i64)
}

#[no_mangle]
pub extern "C" fn sengoo_ws_recv_text(
    handle: u64,
    buffer: *mut u8,
    capacity: usize,
    timeout_ms: u32,
) -> i64 {
    reset_last_error();
    if buffer.is_null() {
        return fail_i64(NetErrorCode::InvalidArgument);
    }
    net_runtime()
        .ws_recv_text(handle, buffer, capacity, timeout_ms)
        .unwrap_or_else(fail_i64)
}

#[no_mangle]
pub extern "C" fn sengoo_ws_close(handle: u64) -> i64 {
    reset_last_error();
    net_runtime().ws_close(handle).unwrap_or_else(fail_bool)
}
