use crate::manifest::validate_package_name;
use miette::{Context, IntoDiagnostic, Result};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MAX_UPLOAD_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug)]
struct Request {
    method: String,
    path: String,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
}

#[derive(Debug)]
struct Response {
    status: u16,
    content_type: &'static str,
    body: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RegistryVersion {
    version: String,
    checksum: String,
    #[serde(default)]
    yanked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    yank_reason: Option<String>,
    #[serde(default)]
    features: Vec<String>,
}

#[derive(Debug, Serialize)]
struct RegistryIndex {
    versions: Vec<RegistryVersion>,
}

#[derive(Debug, Deserialize)]
struct YankRequest {
    #[serde(default)]
    reason: Option<String>,
}

pub(crate) fn serve(root: &Path, listen: &str, max_requests: Option<usize>) -> Result<()> {
    fs::create_dir_all(root)
        .into_diagnostic()
        .with_context(|| format!("failed to create registry root {}", root.display()))?;
    let root = fs::canonicalize(root)
        .into_diagnostic()
        .with_context(|| format!("failed to resolve registry root {}", root.display()))?;
    let listener = TcpListener::bind(listen)
        .into_diagnostic()
        .with_context(|| format!("failed to listen on {listen}"))?;
    println!(
        "sgpm reference registry listening on http://{}",
        listener.local_addr().into_diagnostic()?
    );

    for (handled, incoming) in listener.incoming().enumerate() {
        if max_requests.is_some_and(|limit| handled >= limit) {
            break;
        }
        let mut stream = incoming.into_diagnostic()?;
        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .into_diagnostic()?;
        let response = match read_request(&mut stream).and_then(|request| route(&root, request)) {
            Ok(response) => response,
            Err(err) => json_error(500, &format!("internal registry error: {err}")),
        };
        write_response(&mut stream, response)?;
        if max_requests.is_some_and(|limit| handled + 1 >= limit) {
            break;
        }
    }
    Ok(())
}

fn read_request(stream: &mut TcpStream) -> Result<Request> {
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 8192];
    let header_end = loop {
        let read = stream.read(&mut buffer).into_diagnostic()?;
        if read == 0 {
            miette::bail!("connection closed before request headers");
        }
        bytes.extend_from_slice(&buffer[..read]);
        if bytes.len() > 64 * 1024 {
            miette::bail!("request headers exceed 64 KiB");
        }
        if let Some(position) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break position + 4;
        }
    };

    let header_text = std::str::from_utf8(&bytes[..header_end])
        .into_diagnostic()
        .context("request headers are not UTF-8")?;
    let mut lines = header_text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| miette::miette!("missing HTTP request line"))?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts
        .next()
        .ok_or_else(|| miette::miette!("missing HTTP method"))?
        .to_string();
    let path = request_parts
        .next()
        .ok_or_else(|| miette::miette!("missing HTTP path"))?
        .split('?')
        .next()
        .unwrap_or_default()
        .to_string();
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_string()))
        .collect::<BTreeMap<_, _>>();
    let content_length = headers
        .get("content-length")
        .map(|value| value.parse::<usize>().into_diagnostic())
        .transpose()?
        .unwrap_or(0);
    if content_length > MAX_UPLOAD_BYTES {
        miette::bail!("request body exceeds 64 MiB");
    }
    while bytes.len().saturating_sub(header_end) < content_length {
        let read = stream.read(&mut buffer).into_diagnostic()?;
        if read == 0 {
            miette::bail!("connection closed before request body");
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    let body = bytes[header_end..header_end + content_length].to_vec();
    Ok(Request {
        method,
        path,
        headers,
        body,
    })
}

fn route(root: &Path, request: Request) -> Result<Response> {
    let parts = request
        .path
        .trim_matches('/')
        .split('/')
        .collect::<Vec<_>>();
    if parts.len() < 4 || parts[..3] != ["api", "v1", "packages"] {
        return Ok(json_error(404, "route not found"));
    }
    let package = parts[3];
    if let Err(err) = validate_package_name(package) {
        return Ok(json_error(400, &err.to_string()));
    }
    if let Some(version) = parts.get(4) {
        if let Err(err) = Version::parse(version) {
            return Ok(json_error(400, &format!("invalid package version: {err}")));
        }
    }

    match (request.method.as_str(), parts.as_slice()) {
        ("GET", ["api", "v1", "packages", _]) => package_index(root, package),
        ("POST", ["api", "v1", "packages", _, version]) => {
            publish(root, package, version, &request)
        }
        ("GET", ["api", "v1", "packages", _, version]) => version_metadata(root, package, version),
        ("GET", ["api", "v1", "packages", _, version, "download"]) => {
            download(root, package, version)
        }
        ("POST", ["api", "v1", "packages", _, version, "yank"]) => {
            set_yanked(root, package, version, &request, true)
        }
        ("POST", ["api", "v1", "packages", _, version, "unyank"]) => {
            set_yanked(root, package, version, &request, false)
        }
        _ => Ok(json_error(404, "route not found")),
    }
}

fn publish(root: &Path, package: &str, version: &str, request: &Request) -> Result<Response> {
    let token = match bearer_token(request) {
        Ok(token) => token,
        Err(response) => return Ok(response),
    };
    if request.headers.get("x-sengoo-package").map(String::as_str) != Some(package)
        || request.headers.get("x-sengoo-version").map(String::as_str) != Some(version)
    {
        return Ok(json_error(
            400,
            "route package/version must match x-sengoo-package and x-sengoo-version",
        ));
    }
    let actual_checksum = format!("{:x}", Sha256::digest(&request.body));
    if request.headers.get("x-sengoo-checksum").map(String::as_str)
        != Some(actual_checksum.as_str())
    {
        return Ok(json_error(400, "package checksum mismatch"));
    }
    if !request.body.starts_with(&[0x1f, 0x8b]) {
        return Ok(json_error(400, "package archive must be gzip encoded"));
    }

    let package_root = root.join("packages").join(package);
    if let Some(response) = reject_wrong_owner(&package_root, token)? {
        return Ok(response);
    }
    let version_root = package_root.join(version);
    if version_root.exists() {
        return Ok(json_error(409, "package version already exists"));
    }

    fs::create_dir_all(&package_root)
        .into_diagnostic()
        .with_context(|| format!("failed to create {}", package_root.display()))?;
    reserve_owner(&package_root, token)?;
    let staging = unique_staging_path(&package_root, version);
    fs::create_dir(&staging)
        .into_diagnostic()
        .with_context(|| format!("failed to create {}", staging.display()))?;
    let metadata = RegistryVersion {
        version: version.to_string(),
        checksum: actual_checksum,
        yanked: false,
        yank_reason: None,
        features: Vec::new(),
    };
    let write_result = (|| {
        fs::write(staging.join("package.tar.gz"), &request.body)
            .into_diagnostic()
            .context("failed to write package archive")?;
        write_json(&staging.join("metadata.json"), &metadata)?;
        fs::rename(&staging, &version_root)
            .into_diagnostic()
            .with_context(|| {
                format!(
                    "failed to finalize package version {} to {}",
                    staging.display(),
                    version_root.display()
                )
            })
    })();
    if write_result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    write_result?;
    json_response(201, &metadata)
}

fn package_index(root: &Path, package: &str) -> Result<Response> {
    let package_root = root.join("packages").join(package);
    if !package_root.is_dir() {
        return Ok(json_error(404, "package not found"));
    }
    let mut versions = Vec::new();
    for entry in fs::read_dir(&package_root)
        .into_diagnostic()
        .with_context(|| format!("failed to read {}", package_root.display()))?
    {
        let entry = entry.into_diagnostic()?;
        if !entry.path().is_dir() {
            continue;
        }
        let metadata_path = entry.path().join("metadata.json");
        if metadata_path.is_file() {
            versions.push(read_metadata(&metadata_path)?);
        }
    }
    versions.sort_by(|left, right| {
        Version::parse(&left.version)
            .ok()
            .cmp(&Version::parse(&right.version).ok())
    });
    json_response(200, &RegistryIndex { versions })
}

fn version_metadata(root: &Path, package: &str, version: &str) -> Result<Response> {
    let metadata_path = version_root(root, package, version).join("metadata.json");
    if !metadata_path.is_file() {
        return Ok(json_error(404, "package version not found"));
    }
    json_response(200, &read_metadata(&metadata_path)?)
}

fn download(root: &Path, package: &str, version: &str) -> Result<Response> {
    let archive = version_root(root, package, version).join("package.tar.gz");
    if !archive.is_file() {
        return Ok(json_error(404, "package version not found"));
    }
    Ok(Response {
        status: 200,
        content_type: "application/gzip",
        body: fs::read(&archive)
            .into_diagnostic()
            .with_context(|| format!("failed to read {}", archive.display()))?,
    })
}

fn set_yanked(
    root: &Path,
    package: &str,
    version: &str,
    request: &Request,
    yanked: bool,
) -> Result<Response> {
    let token = match bearer_token(request) {
        Ok(token) => token,
        Err(response) => return Ok(response),
    };
    let package_root = root.join("packages").join(package);
    if let Some(response) = reject_wrong_owner(&package_root, token)? {
        return Ok(response);
    }
    let metadata_path = version_root(root, package, version).join("metadata.json");
    if !metadata_path.is_file() {
        return Ok(json_error(404, "package version not found"));
    }
    let mut metadata = read_metadata(&metadata_path)?;
    metadata.yanked = yanked;
    metadata.yank_reason = if yanked && !request.body.is_empty() {
        serde_json::from_slice::<YankRequest>(&request.body)
            .into_diagnostic()
            .context("invalid yank request JSON")?
            .reason
            .filter(|reason| !reason.trim().is_empty())
    } else {
        None
    };
    write_json_atomic(&metadata_path, &metadata)?;
    json_response(200, &metadata)
}

fn bearer_token(request: &Request) -> std::result::Result<&str, Response> {
    request
        .headers
        .get("authorization")
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|token| !token.trim().is_empty())
        .ok_or_else(|| json_error(401, "bearer token required"))
}

fn reject_wrong_owner(package_root: &Path, token: &str) -> Result<Option<Response>> {
    let owner_path = package_root.join("owner.sha256");
    if !owner_path.exists() {
        return Ok(None);
    }
    let owner = fs::read_to_string(&owner_path)
        .into_diagnostic()
        .with_context(|| format!("failed to read {}", owner_path.display()))?;
    let candidate = token_hash(token);
    Ok((owner.trim() != candidate)
        .then(|| json_error(403, "package name is reserved by another registry owner")))
}

fn reserve_owner(package_root: &Path, token: &str) -> Result<()> {
    let owner_path = package_root.join("owner.sha256");
    if owner_path.exists() {
        return Ok(());
    }
    fs::write(&owner_path, format!("{}\n", token_hash(token)))
        .into_diagnostic()
        .with_context(|| {
            format!(
                "failed to reserve package owner at {}",
                owner_path.display()
            )
        })
}

fn token_hash(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

fn version_root(root: &Path, package: &str, version: &str) -> PathBuf {
    root.join("packages").join(package).join(version)
}

fn unique_staging_path(package_root: &Path, version: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    package_root.join(format!(".{version}.sgpm-registry-{stamp}"))
}

fn read_metadata(path: &Path) -> Result<RegistryVersion> {
    let bytes = fs::read(path)
        .into_diagnostic()
        .with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_slice(&bytes)
        .into_diagnostic()
        .with_context(|| format!("failed to parse {}", path.display()))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value).into_diagnostic()?;
    bytes.push(b'\n');
    fs::write(path, bytes)
        .into_diagnostic()
        .with_context(|| format!("failed to write {}", path.display()))
}

fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let staging = path.with_extension("json.tmp");
    write_json(&staging, value)?;
    fs::rename(&staging, path)
        .into_diagnostic()
        .with_context(|| format!("failed to replace {}", path.display()))
}

fn json_response<T: Serialize>(status: u16, value: &T) -> Result<Response> {
    Ok(Response {
        status,
        content_type: "application/json",
        body: serde_json::to_vec(value).into_diagnostic()?,
    })
}

fn json_error(status: u16, message: &str) -> Response {
    Response {
        status,
        content_type: "application/json",
        body: serde_json::to_vec(&serde_json::json!({ "error": message }))
            .unwrap_or_else(|_| b"{\"error\":\"registry error\"}".to_vec()),
    }
}

fn write_response(stream: &mut TcpStream, response: Response) -> Result<()> {
    let reason = match response.status {
        200 => "OK",
        201 => "Created",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        409 => "Conflict",
        413 => "Payload Too Large",
        _ => "Internal Server Error",
    };
    let headers = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        response.status,
        reason,
        response.content_type,
        response.body.len()
    );
    stream.write_all(headers.as_bytes()).into_diagnostic()?;
    stream.write_all(&response.body).into_diagnostic()?;
    stream.flush().into_diagnostic()
}
