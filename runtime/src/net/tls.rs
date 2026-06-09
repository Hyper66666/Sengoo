use std::io::{Read, Write};
use std::net::TcpStream;

use super::NetErrorCode;

#[cfg(windows)]
use native_tls::{HandshakeError, TlsConnector};

#[cfg(not(windows))]
use rustls::pki_types::{CertificateDer, ServerName};
#[cfg(not(windows))]
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};
#[cfg(all(test, not(windows)))]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(not(windows))]
use std::sync::Arc;
#[cfg(all(test, not(windows)))]
use std::sync::{Mutex, OnceLock};

pub enum TlsStream {
    Plain(TcpStream),
    #[cfg(windows)]
    Native(Box<native_tls::TlsStream<TcpStream>>),
    #[cfg(not(windows))]
    Rustls(Box<StreamOwned<ClientConnection, TcpStream>>),
}

impl Read for TlsStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            TlsStream::Plain(stream) => stream.read(buf),
            #[cfg(windows)]
            TlsStream::Native(stream) => stream.read(buf),
            #[cfg(not(windows))]
            TlsStream::Rustls(stream) => stream.read(buf),
        }
    }
}

impl Write for TlsStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            TlsStream::Plain(stream) => stream.write(buf),
            #[cfg(windows)]
            TlsStream::Native(stream) => stream.write(buf),
            #[cfg(not(windows))]
            TlsStream::Rustls(stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            TlsStream::Plain(stream) => stream.flush(),
            #[cfg(windows)]
            TlsStream::Native(stream) => stream.flush(),
            #[cfg(not(windows))]
            TlsStream::Rustls(stream) => stream.flush(),
        }
    }
}

#[cfg(all(test, not(windows)))]
static TEST_EXTRA_ROOTS: OnceLock<Mutex<Vec<CertificateDer<'static>>>> = OnceLock::new();
#[cfg(all(test, not(windows)))]
static TEST_DISABLE_NATIVE_ROOTS: AtomicBool = AtomicBool::new(false);

#[cfg(all(test, not(windows)))]
pub(crate) fn set_test_extra_roots(roots: Vec<CertificateDer<'static>>) {
    let lock = TEST_EXTRA_ROOTS.get_or_init(|| Mutex::new(Vec::new()));
    *lock.lock().expect("test extra roots lock") = roots;
}

#[cfg(all(test, not(windows)))]
pub(crate) fn clear_test_extra_roots() {
    if let Some(lock) = TEST_EXTRA_ROOTS.get() {
        lock.lock().expect("test extra roots lock").clear();
    }
}

#[cfg(all(test, not(windows)))]
pub(crate) fn set_test_disable_native_roots(disabled: bool) {
    TEST_DISABLE_NATIVE_ROOTS.store(disabled, Ordering::SeqCst);
}

pub fn connect_tls(tcp: TcpStream, host: &str) -> Result<TlsStream, NetErrorCode> {
    if host.is_empty() {
        return Err(NetErrorCode::InvalidArgument);
    }

    #[cfg(windows)]
    {
        let connector = TlsConnector::new().map_err(|_| NetErrorCode::TlsUnavailable)?;
        let tls = connector
            .connect(host, tcp)
            .map_err(classify_native_handshake_error)?;
        Ok(TlsStream::Native(Box::new(tls)))
    }

    #[cfg(not(windows))]
    {
        let config = build_rustls_config()?;
        let server_name = ServerName::try_from(host.to_ascii_lowercase())
            .map_err(|_| NetErrorCode::InvalidArgument)?;
        let conn = ClientConnection::new(Arc::new(config), server_name)
            .map_err(|err| classify_rustls_error(&err))?;
        let mut tls = StreamOwned::new(conn, tcp);
        while tls.conn.is_handshaking() {
            tls.conn
                .complete_io(&mut tls.sock)
                .map_err(classify_rustls_io_error)?;
        }
        Ok(TlsStream::Rustls(Box::new(tls)))
    }
}

#[cfg(windows)]
fn classify_native_handshake_error(err: HandshakeError<TcpStream>) -> NetErrorCode {
    match err {
        HandshakeError::Failure(error) => classify_native_tls_error(&error),
        HandshakeError::WouldBlock(_) => NetErrorCode::TlsHandshake,
    }
}

#[cfg(windows)]
fn classify_native_tls_error(err: &native_tls::Error) -> NetErrorCode {
    classify_native_tls_message(&err.to_string())
}

#[cfg(windows)]
fn classify_native_tls_message(message: &str) -> NetErrorCode {
    let lower = message.to_ascii_lowercase();
    if lower.contains("hostname") || lower.contains("cn_no_match") {
        NetErrorCode::TlsHostnameMismatch
    } else if lower.contains("cert") || lower.contains("trust") || lower.contains("chain") {
        NetErrorCode::TlsCertInvalid
    } else {
        NetErrorCode::TlsHandshake
    }
}

#[cfg(not(windows))]
fn build_rustls_config() -> Result<ClientConfig, NetErrorCode> {
    let mut root_store = RootCertStore::empty();
    let mut loaded_any = false;

    #[cfg(test)]
    let load_native_roots = !TEST_DISABLE_NATIVE_ROOTS.load(Ordering::SeqCst);
    #[cfg(not(test))]
    let load_native_roots = true;

    if load_native_roots {
        let native_roots = rustls_native_certs::load_native_certs();
        for cert in native_roots.certs {
            if root_store.add(cert).is_ok() {
                loaded_any = true;
            }
        }
    }

    #[cfg(test)]
    if let Some(extra) = TEST_EXTRA_ROOTS.get() {
        for cert in extra.lock().expect("test extra roots lock").iter() {
            if root_store.add(cert.clone()).is_ok() {
                loaded_any = true;
            }
        }
    }

    if !loaded_any {
        return Err(NetErrorCode::TlsUnavailable);
    }

    ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth()
}

#[cfg(not(windows))]
fn classify_rustls_error(err: &rustls::Error) -> NetErrorCode {
    match err {
        rustls::Error::InvalidCertificate(
            rustls::CertificateError::NotValidForName
            | rustls::CertificateError::NotValidForNameContext { .. },
        ) => NetErrorCode::TlsHostnameMismatch,
        rustls::Error::InvalidCertificate(_) => NetErrorCode::TlsCertInvalid,
        _ => NetErrorCode::TlsHandshake,
    }
}

#[cfg(not(windows))]
fn classify_rustls_io_error(err: std::io::Error) -> NetErrorCode {
    if let Some(rustls_err) = err
        .get_ref()
        .and_then(|source| source.downcast_ref::<rustls::Error>())
    {
        return classify_rustls_error(rustls_err);
    }
    NetErrorCode::TlsHandshake
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcgen::{BasicConstraints, CertificateParams, DnType, IsCa, KeyPair, SanType};
    use rustls::pki_types::{CertificateDer, PrivateKeyDer};
    use rustls::ServerConfig;
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
    use std::thread;
    use std::time::Duration;

    static TLS_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    struct TlsTestGuard {
        _lock: MutexGuard<'static, ()>,
    }

    impl Drop for TlsTestGuard {
        fn drop(&mut self) {
            clear_test_extra_roots();
            set_test_disable_native_roots(false);
        }
    }

    fn tls_test_guard() -> TlsTestGuard {
        let lock = TLS_TEST_LOCK.get_or_init(|| Mutex::new(()));
        let guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        clear_test_extra_roots();
        set_test_disable_native_roots(false);
        TlsTestGuard { _lock: guard }
    }

    struct TestCertBundle {
        #[cfg(not(windows))]
        ca_der: CertificateDer<'static>,
        server_der: CertificateDer<'static>,
        server_key: PrivateKeyDer<'static>,
        #[cfg(not(windows))]
        wrong_host_der: CertificateDer<'static>,
        #[cfg(not(windows))]
        wrong_host_key: PrivateKeyDer<'static>,
    }

    fn build_test_cert_bundle() -> TestCertBundle {
        let mut ca_params = CertificateParams::default();
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        ca_params
            .distinguished_name
            .push(DnType::CommonName, "Sengoo Test CA");
        let ca_key = KeyPair::generate().expect("ca key");
        let ca_cert = ca_params.self_signed(&ca_key).expect("ca cert");

        let mut server_params =
            CertificateParams::new(vec!["localhost".into()]).expect("server params");
        server_params
            .subject_alt_names
            .push(SanType::IpAddress(std::net::IpAddr::V4(
                std::net::Ipv4Addr::LOCALHOST,
            )));
        let server_key = KeyPair::generate().expect("server key");
        let server_cert = server_params
            .signed_by(&server_key, &ca_cert, &ca_key)
            .expect("server cert");

        #[cfg(not(windows))]
        let (wrong_host_der, wrong_host_key) = {
            let wrong_params =
                CertificateParams::new(vec!["wrong-host.test".into()]).expect("wrong params");
            let wrong_key = KeyPair::generate().expect("wrong key");
            let wrong_cert = wrong_params
                .signed_by(&wrong_key, &ca_cert, &ca_key)
                .expect("wrong cert");
            (
                CertificateDer::from(wrong_cert.der().clone()),
                PrivateKeyDer::Pkcs8(wrong_key.serialize_der().into()),
            )
        };

        TestCertBundle {
            #[cfg(not(windows))]
            ca_der: CertificateDer::from(ca_cert.der().clone()),
            server_der: CertificateDer::from(server_cert.der().clone()),
            server_key: PrivateKeyDer::Pkcs8(server_key.serialize_der().into()),
            #[cfg(not(windows))]
            wrong_host_der,
            #[cfg(not(windows))]
            wrong_host_key,
        }
    }

    fn spawn_tls_server(
        cert: CertificateDer<'static>,
        key: PrivateKeyDer<'static>,
        response: &'static [u8],
    ) -> u16 {
        let config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert], key)
            .expect("server config");
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind tls listener");
        let port = listener.local_addr().expect("local addr").port();
        let config = Arc::new(config);

        thread::spawn(move || {
            if let Ok((tcp, _)) = listener.accept() {
                let conn = rustls::ServerConnection::new(config).expect("server conn");
                let mut tls = rustls::StreamOwned::new(conn, tcp);
                let mut req = [0u8; 2048];
                let _ = tls.read(&mut req);
                let _ = tls.write_all(response);
                let _ = tls.flush();
            }
        });

        port
    }

    #[test]
    #[cfg(not(windows))]
    fn tls_success_with_test_ca_root() {
        let _guard = tls_test_guard();
        let bundle = build_test_cert_bundle();
        set_test_extra_roots(vec![bundle.ca_der.clone()]);

        let response = b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello";
        let port = spawn_tls_server(bundle.server_der, bundle.server_key, response);

        let tcp = TcpStream::connect(("127.0.0.1", port)).expect("connect tcp");
        tcp.set_read_timeout(Some(Duration::from_secs(5)))
            .expect("read timeout");
        tcp.set_write_timeout(Some(Duration::from_secs(5)))
            .expect("write timeout");
        let mut tls = connect_tls(tcp, "127.0.0.1").expect("tls connect");
        tls.write_all(b"GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
            .expect("write request");
        tls.flush().expect("flush request");
        let mut out = [0u8; 256];
        let n = tls.read(&mut out).expect("read tls response");
        assert!(String::from_utf8_lossy(&out[..n]).contains("hello"));
    }

    #[cfg(not(windows))]
    fn clear_test_extra_roots() {
        super::clear_test_extra_roots();
    }

    #[cfg(windows)]
    fn clear_test_extra_roots() {}

    #[cfg(not(windows))]
    fn set_test_disable_native_roots(disabled: bool) {
        super::set_test_disable_native_roots(disabled);
    }

    #[cfg(windows)]
    fn set_test_disable_native_roots(_disabled: bool) {}

    #[test]
    fn tls_untrusted_certificate_maps_to_cert_invalid() {
        let _guard = tls_test_guard();
        let bundle = build_test_cert_bundle();
        let port = spawn_tls_server(
            bundle.server_der,
            bundle.server_key,
            b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        );

        let tcp = TcpStream::connect(("127.0.0.1", port)).expect("connect tcp");
        tcp.set_read_timeout(Some(Duration::from_secs(5)))
            .expect("read timeout");
        tcp.set_write_timeout(Some(Duration::from_secs(5)))
            .expect("write timeout");
        let err = match connect_tls(tcp, "127.0.0.1") {
            Err(code) => code,
            Ok(_) => panic!("untrusted cert should fail"),
        };
        assert!(
            matches!(
                err,
                NetErrorCode::TlsCertInvalid | NetErrorCode::TlsHandshake
            ),
            "unexpected TLS error: {err:?}"
        );
    }

    #[test]
    #[cfg(not(windows))]
    fn https_get_runtime_roundtrip_smoke() {
        let _guard = tls_test_guard();
        use crate::net::{
            sengoo_http_body_copy, sengoo_http_body_len, sengoo_http_close, sengoo_http_get,
            sengoo_http_status,
        };

        let bundle = build_test_cert_bundle();
        set_test_extra_roots(vec![bundle.ca_der.clone()]);
        let response = b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello";
        let port = spawn_tls_server(bundle.server_der, bundle.server_key, response);
        let url = format!("https://127.0.0.1:{}/health\0", port);
        let handle = sengoo_http_get(url.as_ptr(), 5_000);
        assert!(
            handle != 0,
            "https get should succeed against trusted fixture"
        );
        assert_eq!(sengoo_http_status(handle), 200);
        assert_eq!(sengoo_http_body_len(handle), 5);
        let mut out = [0u8; 16];
        let copied = sengoo_http_body_copy(handle, out.as_mut_ptr(), out.len());
        assert_eq!(copied, 5);
        assert_eq!(&out[..5], b"hello");
        assert_eq!(sengoo_http_close(handle), 1);
    }

    #[test]
    #[cfg(not(windows))]
    fn tls_hostname_mismatch_maps_to_hostname_error() {
        let _guard = tls_test_guard();
        let bundle = build_test_cert_bundle();
        set_test_extra_roots(vec![bundle.ca_der.clone()]);
        let port = spawn_tls_server(
            bundle.wrong_host_der,
            bundle.wrong_host_key,
            b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        );

        let tcp = TcpStream::connect(("127.0.0.1", port)).expect("connect tcp");
        tcp.set_read_timeout(Some(Duration::from_secs(5)))
            .expect("read timeout");
        tcp.set_write_timeout(Some(Duration::from_secs(5)))
            .expect("write timeout");
        let err = match connect_tls(tcp, "127.0.0.1") {
            Err(code) => code,
            Ok(_) => panic!("hostname mismatch should fail"),
        };
        assert_eq!(err, NetErrorCode::TlsHostnameMismatch);
    }

    #[test]
    fn tls_unavailable_when_no_trust_roots() {
        let _guard = tls_test_guard();
        #[cfg(not(windows))]
        {
            set_test_disable_native_roots(true);
            assert!(matches!(
                build_rustls_config(),
                Err(NetErrorCode::TlsUnavailable)
            ));
        }
        #[cfg(windows)]
        {
            assert_eq!(NetErrorCode::TlsUnavailable as i32, 18);
        }
    }
}
