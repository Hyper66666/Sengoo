use std::io::{Read, Write};

use super::{
    classify_io_error, copy_bytes_to_buffer, decode_chunked_body, fail_bool, fail_handle, fail_i64,
    net_runtime, open_stream, parse_http_headers, parse_url, reset_last_error,
    split_http_headers_and_body, tls, HttpResponseEntry, NetErrorCode, NetRuntime, ParsedUrl,
    TlsStream,
};

impl NetRuntime {
    pub(crate) fn http_store(&self, response: HttpResponseEntry) -> Result<u64, NetErrorCode> {
        let handle = self.alloc_handle();
        self.http_responses
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?
            .insert(handle, response);
        Ok(handle)
    }

    pub(crate) fn http_status(&self, handle: u64) -> Result<i64, NetErrorCode> {
        let table = self
            .http_responses
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?;
        table
            .get(&handle)
            .map(|resp| resp.status_code)
            .ok_or(NetErrorCode::HandleNotFound)
    }

    pub(crate) fn http_body_len(&self, handle: u64) -> Result<i64, NetErrorCode> {
        let table = self
            .http_responses
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?;
        table
            .get(&handle)
            .map(|resp| resp.body.len() as i64)
            .ok_or(NetErrorCode::HandleNotFound)
    }

    pub(crate) fn http_body_copy(
        &self,
        handle: u64,
        buffer: *mut u8,
        capacity: usize,
    ) -> Result<i64, NetErrorCode> {
        let table = self
            .http_responses
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?;
        let response = table.get(&handle).ok_or(NetErrorCode::HandleNotFound)?;
        Ok(copy_bytes_to_buffer(&response.body, buffer, capacity))
    }

    pub(crate) fn http_close(&self, handle: u64) -> Result<i64, NetErrorCode> {
        let removed = self
            .http_responses
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?
            .remove(&handle)
            .is_some();
        if removed {
            Ok(1)
        } else {
            Err(NetErrorCode::HandleNotFound)
        }
    }
}

fn send_http_request(
    method: &str,
    url: &ParsedUrl,
    body: &[u8],
    timeout_ms: u32,
) -> Result<HttpResponseEntry, NetErrorCode> {
    if url.scheme != "http" && url.scheme != "https" {
        return Err(NetErrorCode::UnsupportedScheme);
    }

    let tcp = open_stream(&url.host, url.port, timeout_ms)?;
    let mut stream: TlsStream = if url.scheme == "https" {
        tls::connect_tls(tcp, &url.host)?
    } else {
        TlsStream::Plain(tcp)
    };
    let mut req = format!(
        "{} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nUser-Agent: sengoo-runtime/0.1\r\n",
        method, url.path, url.host
    );
    if !body.is_empty() {
        req.push_str("Content-Type: application/octet-stream\r\n");
        req.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    req.push_str("\r\n");

    stream
        .write_all(req.as_bytes())
        .map_err(|err| classify_io_error(&err))?;
    if !body.is_empty() {
        stream
            .write_all(body)
            .map_err(|err| classify_io_error(&err))?;
    }
    stream.flush().map_err(|err| classify_io_error(&err))?;

    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|err| classify_io_error(&err))?;
    let (header, body_bytes) = split_http_headers_and_body(&raw)?;
    let (status_code, headers) = parse_http_headers(header)?;
    let body = if headers
        .get("transfer-encoding")
        .map(|v| v.to_ascii_lowercase().contains("chunked"))
        .unwrap_or(false)
    {
        decode_chunked_body(body_bytes)?
    } else {
        body_bytes.to_vec()
    };
    Ok(HttpResponseEntry { status_code, body })
}

#[no_mangle]
pub extern "C" fn sengoo_http_get(url: *const u8, timeout_ms: u32) -> u64 {
    reset_last_error();
    let parsed = match parse_url(url) {
        Ok(parsed) => parsed,
        Err(code) => return fail_handle(code),
    };
    let response = match send_http_request("GET", &parsed, &[], timeout_ms) {
        Ok(response) => response,
        Err(code) => return fail_handle(code),
    };
    net_runtime()
        .http_store(response)
        .unwrap_or_else(fail_handle)
}

#[no_mangle]
/// # Safety
/// If `body` is non-null, it must point to `len` readable bytes.
pub unsafe extern "C" fn sengoo_http_post(
    url: *const u8,
    body: *const u8,
    len: usize,
    timeout_ms: u32,
) -> u64 {
    reset_last_error();
    if body.is_null() && len > 0 {
        return fail_handle(NetErrorCode::InvalidArgument);
    }
    let parsed = match parse_url(url) {
        Ok(parsed) => parsed,
        Err(code) => return fail_handle(code),
    };
    let payload = if len == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(body, len) }
    };
    let response = match send_http_request("POST", &parsed, payload, timeout_ms) {
        Ok(response) => response,
        Err(code) => return fail_handle(code),
    };
    net_runtime()
        .http_store(response)
        .unwrap_or_else(fail_handle)
}

#[no_mangle]
pub extern "C" fn sengoo_http_status(handle: u64) -> i64 {
    reset_last_error();
    net_runtime().http_status(handle).unwrap_or_else(fail_i64)
}

#[no_mangle]
pub extern "C" fn sengoo_http_body_len(handle: u64) -> i64 {
    reset_last_error();
    net_runtime().http_body_len(handle).unwrap_or_else(fail_i64)
}

#[no_mangle]
pub extern "C" fn sengoo_http_body_copy(handle: u64, buffer: *mut u8, capacity: usize) -> i64 {
    reset_last_error();
    if buffer.is_null() {
        return fail_i64(NetErrorCode::InvalidArgument);
    }
    net_runtime()
        .http_body_copy(handle, buffer, capacity)
        .unwrap_or_else(fail_i64)
}

#[no_mangle]
pub extern "C" fn sengoo_http_close(handle: u64) -> i64 {
    reset_last_error();
    net_runtime().http_close(handle).unwrap_or_else(fail_bool)
}
