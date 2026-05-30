use std::io::ErrorKind;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use super::{
    classify_io_error, connect_timeout, fail_bool, fail_handle, fail_i64, net_runtime, parse_host,
    reset_last_error, set_last_error, NetErrorCode, NetRuntime, RecvOutcome,
};

impl NetRuntime {
    pub(crate) fn tcp_connect(
        &self,
        host: &str,
        port: u16,
        timeout_ms: u32,
    ) -> Result<u64, NetErrorCode> {
        let addr = format!("{}:{}", host, port);
        let mut addrs = addr
            .to_socket_addrs()
            .map_err(|_| NetErrorCode::ResolveFailed)?;
        let first_addr = addrs.next().ok_or(NetErrorCode::ResolveFailed)?;
        let stream = TcpStream::connect_timeout(&first_addr, connect_timeout(timeout_ms)).map_err(
            |err| {
                if err.kind() == ErrorKind::TimedOut {
                    NetErrorCode::Timeout
                } else {
                    NetErrorCode::ConnectFailed
                }
            },
        )?;
        stream
            .set_nodelay(true)
            .map_err(|err| classify_io_error(&err))?;
        stream
            .set_read_timeout(Some(connect_timeout(timeout_ms)))
            .map_err(|err| classify_io_error(&err))?;
        stream
            .set_write_timeout(Some(connect_timeout(timeout_ms)))
            .map_err(|err| classify_io_error(&err))?;

        let handle = self.alloc_handle();
        self.tcp_streams
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?
            .insert(handle, stream);
        Ok(handle)
    }

    pub(crate) fn tcp_send(&self, handle: u64, payload: &[u8]) -> Result<i64, NetErrorCode> {
        let mut table = self
            .tcp_streams
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?;
        let stream = table.get_mut(&handle).ok_or(NetErrorCode::HandleNotFound)?;
        stream
            .write(payload)
            .map(|n| n as i64)
            .map_err(|err| classify_io_error(&err))
    }

    pub(crate) fn tcp_recv(
        &self,
        handle: u64,
        buffer: &mut [u8],
        timeout_ms: u32,
    ) -> Result<RecvOutcome, NetErrorCode> {
        let mut table = self
            .tcp_streams
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?;
        let stream = table.get_mut(&handle).ok_or(NetErrorCode::HandleNotFound)?;

        if timeout_ms != 0 {
            stream
                .set_read_timeout(Some(Duration::from_millis(timeout_ms as u64)))
                .map_err(|err| classify_io_error(&err))?;
        }

        match stream.read(buffer) {
            Ok(n) => Ok(RecvOutcome::Bytes(n as i64)),
            Err(err) if err.kind() == ErrorKind::WouldBlock => Ok(RecvOutcome::Timeout),
            Err(err) if err.kind() == ErrorKind::TimedOut => Ok(RecvOutcome::Timeout),
            Err(err) => Err(classify_io_error(&err)),
        }
    }

    pub(crate) fn tcp_close(&self, handle: u64) -> Result<i64, NetErrorCode> {
        let removed = self
            .tcp_streams
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

#[no_mangle]
pub extern "C" fn sengoo_tcp_connect(host: *const u8, port: u16, timeout_ms: u32) -> u64 {
    reset_last_error();
    let host = match parse_host(host) {
        Ok(host) => host,
        Err(code) => return fail_handle(code),
    };
    net_runtime()
        .tcp_connect(&host, port, timeout_ms)
        .unwrap_or_else(fail_handle)
}

#[no_mangle]
/// # Safety
/// If `data` is non-null, it must point to `len` readable bytes.
pub unsafe extern "C" fn sengoo_tcp_send(handle: u64, data: *const u8, len: usize) -> i64 {
    reset_last_error();
    if data.is_null() {
        return fail_i64(NetErrorCode::InvalidArgument);
    }
    let payload = unsafe { std::slice::from_raw_parts(data, len) };
    net_runtime()
        .tcp_send(handle, payload)
        .unwrap_or_else(fail_i64)
}

#[no_mangle]
/// # Safety
/// If `buffer` is non-null, it must point to `capacity` writable bytes.
pub unsafe extern "C" fn sengoo_tcp_recv(
    handle: u64,
    buffer: *mut u8,
    capacity: usize,
    timeout_ms: u32,
) -> i64 {
    reset_last_error();
    if buffer.is_null() {
        return fail_i64(NetErrorCode::InvalidArgument);
    }
    let target = unsafe { std::slice::from_raw_parts_mut(buffer, capacity) };
    match net_runtime().tcp_recv(handle, target, timeout_ms) {
        Ok(RecvOutcome::Bytes(n)) => n,
        Ok(RecvOutcome::Timeout) => {
            set_last_error(NetErrorCode::Timeout);
            0
        }
        Err(code) => fail_i64(code),
    }
}

#[no_mangle]
pub extern "C" fn sengoo_tcp_close(handle: u64) -> i64 {
    reset_last_error();
    net_runtime().tcp_close(handle).unwrap_or_else(fail_bool)
}
