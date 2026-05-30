use std::io::ErrorKind;
use std::net::UdpSocket;
use std::time::Duration;

use super::{
    classify_io_error, fail_bool, fail_handle, fail_i64, net_runtime, parse_addr, parse_host,
    reset_last_error, set_last_error, NetErrorCode, NetRuntime, RecvOutcome,
};

impl NetRuntime {
    pub(crate) fn udp_bind(&self, host: &str, port: u16) -> Result<u64, NetErrorCode> {
        let addr = format!("{}:{}", host, port);
        let socket = UdpSocket::bind(addr).map_err(|err| classify_io_error(&err))?;
        socket
            .set_nonblocking(false)
            .map_err(|err| classify_io_error(&err))?;

        let handle = self.alloc_handle();
        self.udp_sockets
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?
            .insert(handle, socket);
        Ok(handle)
    }

    pub(crate) fn udp_connect(&self, handle: u64, addr: &str) -> Result<i64, NetErrorCode> {
        let mut table = self
            .udp_sockets
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?;
        let socket = table.get_mut(&handle).ok_or(NetErrorCode::HandleNotFound)?;
        socket
            .connect(addr)
            .map(|_| 1)
            .map_err(|_| NetErrorCode::ConnectFailed)
    }

    pub(crate) fn udp_send(&self, handle: u64, payload: &[u8]) -> Result<i64, NetErrorCode> {
        let mut table = self
            .udp_sockets
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?;
        let socket = table.get_mut(&handle).ok_or(NetErrorCode::HandleNotFound)?;
        socket
            .send(payload)
            .map(|n| n as i64)
            .map_err(|err| classify_io_error(&err))
    }

    pub(crate) fn udp_recv(
        &self,
        handle: u64,
        buffer: &mut [u8],
        timeout_ms: u32,
    ) -> Result<RecvOutcome, NetErrorCode> {
        let mut table = self
            .udp_sockets
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?;
        let socket = table.get_mut(&handle).ok_or(NetErrorCode::HandleNotFound)?;
        if timeout_ms != 0 {
            socket
                .set_read_timeout(Some(Duration::from_millis(timeout_ms as u64)))
                .map_err(|err| classify_io_error(&err))?;
        }
        match socket.recv(buffer) {
            Ok(n) => Ok(RecvOutcome::Bytes(n as i64)),
            Err(err) if err.kind() == ErrorKind::WouldBlock => Ok(RecvOutcome::Timeout),
            Err(err) if err.kind() == ErrorKind::TimedOut => Ok(RecvOutcome::Timeout),
            Err(err) => Err(classify_io_error(&err)),
        }
    }

    pub(crate) fn udp_close(&self, handle: u64) -> Result<i64, NetErrorCode> {
        let removed = self
            .udp_sockets
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
pub extern "C" fn sengoo_udp_bind(host: *const u8, port: u16) -> u64 {
    reset_last_error();
    let host = if host.is_null() {
        "0.0.0.0".to_string()
    } else {
        match parse_host(host) {
            Ok(host) => host,
            Err(code) => return fail_handle(code),
        }
    };
    net_runtime()
        .udp_bind(&host, port)
        .unwrap_or_else(fail_handle)
}

#[no_mangle]
pub extern "C" fn sengoo_udp_connect(handle: u64, host: *const u8, port: u16) -> i64 {
    reset_last_error();
    let addr = match parse_addr(host, port) {
        Ok(addr) => addr,
        Err(code) => return fail_bool(code),
    };
    net_runtime()
        .udp_connect(handle, &addr)
        .unwrap_or_else(fail_bool)
}

#[no_mangle]
/// # Safety
/// If `data` is non-null, it must point to `len` readable bytes.
pub unsafe extern "C" fn sengoo_udp_send(handle: u64, data: *const u8, len: usize) -> i64 {
    reset_last_error();
    if data.is_null() {
        return fail_i64(NetErrorCode::InvalidArgument);
    }
    let payload = unsafe { std::slice::from_raw_parts(data, len) };
    net_runtime()
        .udp_send(handle, payload)
        .unwrap_or_else(fail_i64)
}

#[no_mangle]
/// # Safety
/// If `buffer` is non-null, it must point to `capacity` writable bytes.
pub unsafe extern "C" fn sengoo_udp_recv(
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
    match net_runtime().udp_recv(handle, target, timeout_ms) {
        Ok(RecvOutcome::Bytes(n)) => n,
        Ok(RecvOutcome::Timeout) => {
            set_last_error(NetErrorCode::Timeout);
            0
        }
        Err(code) => fail_i64(code),
    }
}

#[no_mangle]
pub extern "C" fn sengoo_udp_close(handle: u64) -> i64 {
    reset_last_error();
    net_runtime().udp_close(handle).unwrap_or_else(fail_bool)
}
