use serde::Serialize;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Barrier, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

pub const SENGOO_NET_BENCH_OK: i32 = 0;
pub const SENGOO_NET_BENCH_ERR_INVALID_ARGUMENT: i32 = -2601;
pub const SENGOO_NET_BENCH_ERR_IO: i32 = -2602;
pub const SENGOO_NET_BENCH_ERR_INTERNAL: i32 = -2699;

#[derive(Clone, Debug)]
struct NetBenchErrorState {
    code: i32,
    message: String,
}

impl Default for NetBenchErrorState {
    fn default() -> Self {
        Self {
            code: SENGOO_NET_BENCH_OK,
            message: String::new(),
        }
    }
}

#[derive(Serialize, serde::Deserialize, Clone, Debug)]
pub struct NetBenchReport {
    pub connections: u32,
    pub rtt_messages_per_connection: u32,
    pub broadcast_rounds: u32,
    pub payload_bytes: u32,
    pub rtt_samples: usize,
    pub rtt_p50_us: u64,
    pub rtt_p95_us: u64,
    pub rtt_p99_us: u64,
    pub broadcast_samples: usize,
    pub broadcast_p50_us: u64,
    pub broadcast_p95_us: u64,
    pub broadcast_p99_us: u64,
}

static NET_BENCH_LAST_ERROR: OnceLock<Mutex<NetBenchErrorState>> = OnceLock::new();
static NET_BENCH_LAST_RUN_ID: AtomicU64 = AtomicU64::new(0);

fn net_bench_last_error() -> &'static Mutex<NetBenchErrorState> {
    NET_BENCH_LAST_ERROR.get_or_init(|| Mutex::new(NetBenchErrorState::default()))
}

fn clear_error() {
    if let Ok(mut state) = net_bench_last_error().lock() {
        state.code = SENGOO_NET_BENCH_OK;
        state.message.clear();
    }
}

fn set_error(code: i32, message: impl Into<String>) -> i32 {
    if let Ok(mut state) = net_bench_last_error().lock() {
        state.code = code;
        state.message = message.into();
    }
    code
}

fn copy_bytes_to_buffer(bytes: &[u8], buffer: *mut u8, capacity: usize) -> i64 {
    if buffer.is_null() {
        return set_error(SENGOO_NET_BENCH_ERR_INVALID_ARGUMENT, "null output buffer") as i64;
    }
    let copy_len = bytes.len().min(capacity);
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer, copy_len);
    }
    copy_len as i64
}

fn percentile_us(mut samples: Vec<u64>, p: f64) -> u64 {
    if samples.is_empty() {
        return 0;
    }
    samples.sort_unstable();
    let rank = ((samples.len() as f64) * p).ceil() as usize;
    let index = rank.saturating_sub(1).min(samples.len() - 1);
    samples[index]
}

fn run_roundtrip_bench(
    connections: usize,
    messages_per_connection: usize,
    payload_bytes: usize,
) -> Result<Vec<u64>, i32> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|err| set_error(SENGOO_NET_BENCH_ERR_IO, format!("bind failed: {err}")))?;
    let addr = listener
        .local_addr()
        .map_err(|err| set_error(SENGOO_NET_BENCH_ERR_IO, format!("local_addr failed: {err}")))?;
    let payload = vec![0x5Au8; payload_bytes.max(1)];
    let payload_len = payload.len();

    let (server_done_tx, server_done_rx) = mpsc::channel::<Result<(), i32>>();
    let server = thread::spawn(move || {
        let mut workers = Vec::with_capacity(connections);
        for _ in 0..connections {
            let (mut stream, _) = match listener.accept() {
                Ok(v) => v,
                Err(err) => {
                    let _ = server_done_tx.send(Err(set_error(
                        SENGOO_NET_BENCH_ERR_IO,
                        format!("accept failed: {err}"),
                    )));
                    return;
                }
            };
            workers.push(thread::spawn(move || {
                let mut recv = vec![0u8; payload_len];
                while let Ok(()) = stream.read_exact(&mut recv) {
                    if stream.write_all(&recv).is_err() {
                        break;
                    }
                }
            }));
        }
        for worker in workers {
            let _ = worker.join();
        }
        let _ = server_done_tx.send(Ok(()));
    });

    let barrier = Arc::new(Barrier::new(connections.max(1)));
    let latencies = Arc::new(Mutex::new(Vec::<u64>::with_capacity(
        connections * messages_per_connection,
    )));
    let mut clients = Vec::with_capacity(connections);
    for _ in 0..connections {
        let mut stream = TcpStream::connect(addr)
            .map_err(|err| set_error(SENGOO_NET_BENCH_ERR_IO, format!("connect failed: {err}")))?;
        stream.set_nodelay(true).map_err(|err| {
            set_error(
                SENGOO_NET_BENCH_ERR_IO,
                format!("set_nodelay failed: {err}"),
            )
        })?;
        let barrier = Arc::clone(&barrier);
        let latencies = Arc::clone(&latencies);
        let payload = payload.clone();
        clients.push(thread::spawn(move || -> Result<(), i32> {
            barrier.wait();
            let mut recv = vec![0u8; payload.len()];
            for _ in 0..messages_per_connection {
                let start = Instant::now();
                stream.write_all(&payload).map_err(|err| {
                    set_error(
                        SENGOO_NET_BENCH_ERR_IO,
                        format!("client write failed: {err}"),
                    )
                })?;
                stream.read_exact(&mut recv).map_err(|err| {
                    set_error(
                        SENGOO_NET_BENCH_ERR_IO,
                        format!("client read failed: {err}"),
                    )
                })?;
                let elapsed_us = start.elapsed().as_micros() as u64;
                let mut sink = latencies.lock().map_err(|_| {
                    set_error(SENGOO_NET_BENCH_ERR_INTERNAL, "latency sink poisoned")
                })?;
                sink.push(elapsed_us);
            }
            Ok(())
        }));
    }

    for client in clients {
        match client.join() {
            Ok(Ok(())) => {}
            Ok(Err(code)) => return Err(code),
            Err(_) => {
                return Err(set_error(
                    SENGOO_NET_BENCH_ERR_INTERNAL,
                    "client thread panicked",
                ))
            }
        }
    }
    match server_done_rx.recv() {
        Ok(Ok(())) => {}
        Ok(Err(code)) => return Err(code),
        Err(_) => {
            return Err(set_error(
                SENGOO_NET_BENCH_ERR_INTERNAL,
                "server completion channel failed",
            ))
        }
    }
    let _ = server.join();

    let collected = latencies
        .lock()
        .map_err(|_| set_error(SENGOO_NET_BENCH_ERR_INTERNAL, "latency sink poisoned"))?
        .clone();
    Ok(collected)
}

fn run_broadcast_bench(
    connections: usize,
    rounds: usize,
    payload_bytes: usize,
) -> Result<Vec<u64>, i32> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|err| set_error(SENGOO_NET_BENCH_ERR_IO, format!("bind failed: {err}")))?;
    let addr = listener
        .local_addr()
        .map_err(|err| set_error(SENGOO_NET_BENCH_ERR_IO, format!("local_addr failed: {err}")))?;
    let (server_tx, server_rx) = mpsc::channel::<Result<Vec<TcpStream>, i32>>();

    let acceptor = thread::spawn(move || {
        let mut streams = Vec::with_capacity(connections);
        for _ in 0..connections {
            match listener.accept() {
                Ok((stream, _)) => streams.push(stream),
                Err(err) => {
                    let _ = server_tx.send(Err(set_error(
                        SENGOO_NET_BENCH_ERR_IO,
                        format!("accept failed: {err}"),
                    )));
                    return;
                }
            }
        }
        let _ = server_tx.send(Ok(streams));
    });

    let mut clients = Vec::with_capacity(connections);
    for _ in 0..connections {
        let stream = TcpStream::connect(addr)
            .map_err(|err| set_error(SENGOO_NET_BENCH_ERR_IO, format!("connect failed: {err}")))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .map_err(|err| {
                set_error(
                    SENGOO_NET_BENCH_ERR_IO,
                    format!("set_read_timeout failed: {err}"),
                )
            })?;
        clients.push(stream);
    }

    let mut server_streams = match server_rx.recv() {
        Ok(Ok(streams)) => streams,
        Ok(Err(code)) => return Err(code),
        Err(_) => {
            return Err(set_error(
                SENGOO_NET_BENCH_ERR_INTERNAL,
                "acceptor channel failed",
            ))
        }
    };
    let _ = acceptor.join();

    let mut latencies = Vec::with_capacity(connections * rounds);
    let payload = vec![0xA5u8; payload_bytes.max(1)];
    for _ in 0..rounds {
        let start = Instant::now();
        for stream in &mut server_streams {
            stream.write_all(&payload).map_err(|err| {
                set_error(
                    SENGOO_NET_BENCH_ERR_IO,
                    format!("broadcast write failed: {err}"),
                )
            })?;
        }
        for stream in &mut clients {
            let mut recv = vec![0u8; payload.len()];
            stream.read_exact(&mut recv).map_err(|err| {
                set_error(
                    SENGOO_NET_BENCH_ERR_IO,
                    format!("broadcast read failed: {err}"),
                )
            })?;
            latencies.push(start.elapsed().as_micros() as u64);
        }
    }
    Ok(latencies)
}

pub fn run_network_benchmark_report(
    connections: u32,
    rtt_messages_per_connection: u32,
    broadcast_rounds: u32,
    payload_bytes: u32,
) -> Result<NetBenchReport, i32> {
    if connections == 0 {
        return Err(set_error(
            SENGOO_NET_BENCH_ERR_INVALID_ARGUMENT,
            "connections must be > 0",
        ));
    }
    if rtt_messages_per_connection == 0 {
        return Err(set_error(
            SENGOO_NET_BENCH_ERR_INVALID_ARGUMENT,
            "rtt_messages_per_connection must be > 0",
        ));
    }
    if broadcast_rounds == 0 {
        return Err(set_error(
            SENGOO_NET_BENCH_ERR_INVALID_ARGUMENT,
            "broadcast_rounds must be > 0",
        ));
    }

    let rtt = run_roundtrip_bench(
        connections as usize,
        rtt_messages_per_connection as usize,
        payload_bytes as usize,
    )?;
    let broadcast = run_broadcast_bench(
        connections as usize,
        broadcast_rounds as usize,
        payload_bytes as usize,
    )?;

    Ok(NetBenchReport {
        connections,
        rtt_messages_per_connection,
        broadcast_rounds,
        payload_bytes,
        rtt_samples: rtt.len(),
        rtt_p50_us: percentile_us(rtt.clone(), 0.50),
        rtt_p95_us: percentile_us(rtt.clone(), 0.95),
        rtt_p99_us: percentile_us(rtt, 0.99),
        broadcast_samples: broadcast.len(),
        broadcast_p50_us: percentile_us(broadcast.clone(), 0.50),
        broadcast_p95_us: percentile_us(broadcast.clone(), 0.95),
        broadcast_p99_us: percentile_us(broadcast, 0.99),
    })
}

/// Returns the error code widened to `i64` so negative bench codes survive
/// the stdlib extern ABI (`-> i64`) without zero-extension corruption.
#[no_mangle]
pub extern "C" fn sengoo_net_bench_last_error_code() -> i64 {
    i64::from(
        net_bench_last_error()
            .lock()
            .map(|state| state.code)
            .unwrap_or(SENGOO_NET_BENCH_ERR_INTERNAL),
    )
}

#[no_mangle]
pub extern "C" fn sengoo_net_bench_last_error_len() -> i64 {
    net_bench_last_error()
        .lock()
        .map(|state| state.message.len() as i64)
        .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn sengoo_net_bench_last_error_copy(buffer: *mut u8, capacity: usize) -> i64 {
    let message = net_bench_last_error()
        .lock()
        .map(|state| state.message.clone())
        .unwrap_or_default();
    copy_bytes_to_buffer(message.as_bytes(), buffer, capacity)
}

#[no_mangle]
pub extern "C" fn sengoo_net_bench_last_error_clear() -> i64 {
    clear_error();
    i64::from(SENGOO_NET_BENCH_OK)
}

#[no_mangle]
pub extern "C" fn sengoo_net_bench_run(
    connections: u32,
    rtt_messages_per_connection: u32,
    broadcast_rounds: u32,
    payload_bytes: u32,
    report_buffer: *mut u8,
    report_capacity: usize,
) -> i64 {
    clear_error();
    let report = match run_network_benchmark_report(
        connections,
        rtt_messages_per_connection,
        broadcast_rounds,
        payload_bytes,
    ) {
        Ok(report) => report,
        Err(code) => return code as i64,
    };

    let report_json = match serde_json::to_vec(&report) {
        Ok(bytes) => bytes,
        Err(err) => {
            return set_error(
                SENGOO_NET_BENCH_ERR_INTERNAL,
                format!("failed to serialize benchmark report: {err}"),
            ) as i64
        }
    };
    let copy_len = copy_bytes_to_buffer(&report_json, report_buffer, report_capacity);
    if copy_len >= 0 {
        NET_BENCH_LAST_RUN_ID.fetch_add(1, Ordering::Relaxed);
    }
    copy_len
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_bench_generates_percentiles() {
        let report = run_network_benchmark_report(6, 8, 4, 32).expect("network benchmark report");
        assert_eq!(report.connections, 6);
        assert_eq!(report.rtt_samples, 48);
        assert_eq!(report.broadcast_samples, 24);
        assert!(report.rtt_p95_us >= report.rtt_p50_us);
        assert!(report.rtt_p99_us >= report.rtt_p95_us);
        assert!(report.broadcast_p95_us >= report.broadcast_p50_us);
        assert!(report.broadcast_p99_us >= report.broadcast_p95_us);
    }

    #[test]
    fn network_bench_c_api_copies_json_report() {
        let mut buf = vec![0u8; 4096];
        let copied = sengoo_net_bench_run(4, 6, 3, 24, buf.as_mut_ptr(), buf.len());
        assert!(copied > 0);
        let json = std::str::from_utf8(&buf[..copied as usize]).expect("utf8");
        let report: NetBenchReport = serde_json::from_str(json).expect("json");
        assert_eq!(report.connections, 4);
        assert!(report.rtt_p99_us >= report.rtt_p95_us);
    }
}
