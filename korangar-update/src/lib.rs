//! Signed client auto-update.
//!
//! # Manifest canonical form (KRUP1)
//!
//! The Ed25519 signature covers this UTF-8 payload and nothing else (no JSON
//! whitespace, no envelope header). Files are sorted by `path` bytewise.
//!
//! ```text
//! KRUP1\n
//! <version>\n
//! <file_count decimal>\n
//! repeated file_count times:
//! <relative path>\n
//! <sha256 lowercase hex, 64 chars>\n
//! <size decimal>\n
//! <url>\n
//! ```
//!
//! `version` is `[0-9A-Za-z._+-]{1,64}`. `build` is a positive decimal integer
//! (monotonic counter or unix timestamp). Paths are relative, slash-separated,
//! with no `.` or `..` components. `url` is that same relative path, an
//! `http://` loopback URL, a `file://` URL, or an allowlisted HTTPS URL:
//! the catalog at `https://ro.turoran.com/client/korangar/main/latest`, and
//! presigned `https://garage.turoran.com/korangar-releases/main/<name>` objects.
//! Every other host, including other `https` hosts, is rejected.
//!
//! # Envelope
//!
//! ```text
//! KRUP1SIG\n
//! <signature lowercase hex, 128 chars>\n
//! <canonical payload, which itself starts with KRUP1\n>
//! ```
//!
//! The public key is [`EMBEDDED_UPDATE_PUBLIC_KEY`]. The private seed stays on
//! the signing controller, outside the git tree. See
//! `docs/signed-auto-update.md`.

pub mod cli;
pub mod manifest;

/// rustls 0.23 does not install a process-wide crypto provider when its default
/// features are off. Call this before building a `ClientConfig`. A later call
/// is ignored once a provider is already installed.
pub fn install_rustls_crypto() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::Duration;

pub use manifest::EMBEDDED_UPDATE_PUBLIC_KEY as UPDATE_PUBLIC_KEY;
use manifest::ReleaseManifest;
use serde::Deserialize;

/// Public main-channel catalog. remoteclient2 issues short-lived presigned GET
/// URLs for the private Garage bucket `korangar-releases`. An unreachable
/// catalog is skipped so the current binary still runs.
pub const DEFAULT_UPDATE_MANIFEST_URL: &str = "https://ro.turoran.com/client/korangar/main/latest";

const MANIFEST_LIMIT: usize = 256 * 1024;
const FILE_LIMIT: u64 = 512 * 1024 * 1024;
const SELF_CHECK_TOKEN: &str = "update-self-check-ok";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartupOutcome {
    Skipped,
    UpToDate,
    Applied { relaunched: bool },
}

pub struct StartupRequest<'a> {
    pub skip_env: Option<&'a str>,
    pub manifest_url: Option<&'a str>,
    pub current_version: &'a str,
    pub install_root: &'a Path,
    pub current_exe: &'a Path,
    pub public_key: &'a [u8; 32],
    pub relaunch_args: &'a [String],
    /// When false, a successful swap is not followed by a process spawn
    /// (tests).
    pub spawn_relaunch: bool,
}

/// True when an update attempt should run. `KORANGAR_SKIP_UPDATE=1` or an empty
/// manifest URL keeps TEST and developer launches off the network.
pub fn should_attempt(skip_env: Option<&str>, manifest_url: Option<&str>) -> bool {
    if matches!(skip_env.map(str::trim), Some("1")) {
        return false;
    }
    matches!(manifest_url.map(str::trim), Some(url) if !url.is_empty())
}

pub fn run_startup_update(request: &StartupRequest<'_>) -> Result<StartupOutcome, String> {
    if !should_attempt(request.skip_env, request.manifest_url) {
        return Ok(StartupOutcome::Skipped);
    }
    let url = request.manifest_url.unwrap_or("").trim();
    if request.public_key == &[0u8; 32] {
        return Err("embedded update public key is unset; refusing to apply a manifest".into());
    }
    let fetched = match fetch_url(url, MANIFEST_LIMIT as u64) {
        Ok(bytes) => bytes,
        Err(error) if is_unreachable_fetch(&error) => {
            return Ok(StartupOutcome::Skipped);
        }
        Err(error) => return Err(error),
    };
    let (envelope, catalog, manifest_url) = if looks_like_json(&fetched) {
        let catalog = parse_catalog(&fetched)?;
        if !is_fetch_allowed(&catalog.manifest_url) {
            return Err("catalog manifest_url is not an allowlisted release url".into());
        }
        for file in &catalog.files {
            if !is_fetch_allowed(&file.url) {
                return Err(format!("catalog file url for {} is not allowlisted", file.path));
            }
        }
        let envelope = match fetch_url(&catalog.manifest_url, MANIFEST_LIMIT as u64) {
            Ok(bytes) => bytes,
            Err(error) if is_unreachable_fetch(&error) => {
                return Ok(StartupOutcome::Skipped);
            }
            Err(error) => return Err(error),
        };
        let manifest_url = catalog.manifest_url.clone();
        (envelope, Some(catalog), manifest_url)
    } else {
        (fetched, None, url.to_string())
    };
    let manifest = manifest::open_envelope(&envelope, request.public_key)?;
    let installed = installed_build(request.install_root);
    if manifest.build == installed && installed != 0 {
        return Ok(StartupOutcome::UpToDate);
    }
    if !manifest::is_newer_build(manifest.build, installed) {
        return Err(format!(
            "rollback protection: manifest build {} is not newer than installed {installed}",
            manifest.build
        ));
    }
    apply_manifest(&manifest, &manifest_url, catalog.as_ref(), request)
}

fn apply_manifest(
    manifest: &ReleaseManifest,
    manifest_url: &str,
    catalog: Option<&ReleaseCatalog>,
    request: &StartupRequest<'_>,
) -> Result<StartupOutcome, String> {
    let exe_rel = request
        .current_exe
        .strip_prefix(request.install_root)
        .map_err(|_| "current executable is outside the install root")?
        .to_string_lossy()
        .replace('\\', "/");
    let staging = request.install_root.join("updates").join("staging").join(&manifest.version);
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|error| error.to_string())?;
    }
    fs::create_dir_all(&staging).map_err(|error| error.to_string())?;

    let mut staged = Vec::with_capacity(manifest.files.len());
    for file in &manifest.files {
        let relative = safe_relative(&file.path)?;
        if file.size > FILE_LIMIT {
            return Err(format!("artifact {} exceeds size cap", file.path));
        }
        let bytes = fetch_url(&artifact_url(manifest_url, file, catalog)?, file.size)?;
        if bytes.len() as u64 != file.size || manifest::sha256_hex(&bytes) != file.sha256 {
            let _ = fs::remove_dir_all(&staging);
            return Err(format!("artifact {} failed sha256 or size check", file.path));
        }
        let destination = staging.join(&relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        write_full(&destination, &bytes)?;
        staged.push((relative, destination));
    }

    let exe_changed = staged.iter().any(|(relative, _)| relative_string(relative) == exe_rel);
    let swapped = swap_into_place(request.install_root, &staged, exe_changed.then_some(exe_rel.as_str()))?;
    if self_check(request.current_exe).is_err() {
        rollback(&swapped);
        let _ = fs::remove_dir_all(&staging);
        return Err("updated executable failed self-check; previous files restored".into());
    }
    if let Err(error) = write_full(
        &request.install_root.join("updates").join("applied.version"),
        manifest.version.as_bytes(),
    ) {
        rollback(&swapped);
        return Err(error);
    }
    if let Err(error) = write_full(
        &request.install_root.join("updates").join("applied.build"),
        manifest.build.to_string().as_bytes(),
    ) {
        rollback(&swapped);
        let _ = fs::remove_file(request.install_root.join("updates").join("applied.version"));
        let _ = fs::remove_file(request.install_root.join("updates").join("applied.build"));
        return Err(error);
    }
    let _ = fs::remove_dir_all(&staging);
    for item in &swapped {
        if let Some(aside) = &item.displaced_backup {
            let _ = fs::remove_file(aside);
        }
    }

    let relaunched = if request.spawn_relaunch {
        match Command::new(request.current_exe).args(request.relaunch_args).spawn() {
            Ok(_) => true,
            Err(error) => {
                rollback(&swapped);
                let _ = fs::remove_file(request.install_root.join("updates").join("applied.version"));
                let _ = fs::remove_file(request.install_root.join("updates").join("applied.build"));
                return Err(format!("relaunch failed; previous files restored: {error}"));
            }
        }
    } else {
        false
    };
    Ok(StartupOutcome::Applied { relaunched })
}

struct Swapped {
    dest: PathBuf,
    backup: Option<PathBuf>,
    /// Previous `*.old`, moved aside until this swap is committed. Restored
    /// if a later step in the same apply fails.
    displaced_backup: Option<PathBuf>,
}

fn swap_into_place(root: &Path, staged: &[(PathBuf, PathBuf)], exe_rel: Option<&str>) -> Result<Vec<Swapped>, String> {
    let mut done = Vec::new();
    for (relative, source) in staged {
        let dest = root.join(relative);
        if !dest.starts_with(root) {
            rollback(&done);
            return Err(format!("refusing to write {}", dest.display()));
        }
        let backup_path = backup_path(&dest);
        let mut displaced_backup = None;
        let backup = if dest.exists() {
            if backup_path.exists() {
                let aside = aside_path(&dest);
                if let Err(error) = fs::rename(&backup_path, &aside) {
                    rollback(&done);
                    return Err(format!(
                        "could not move existing backup {} aside; earlier files restored: {error}",
                        backup_path.display()
                    ));
                }
                displaced_backup = Some(aside);
            }
            if let Err(error) = fs::rename(&dest, &backup_path) {
                restore_displaced(&dest, displaced_backup.as_deref());
                rollback(&done);
                return Err(format!("backup {}: {error}", dest.display()));
            }
            Some(backup_path)
        } else {
            None
        };
        if let Some(parent) = dest.parent()
            && let Err(error) = fs::create_dir_all(parent)
        {
            undo_current(&dest, backup.as_deref(), displaced_backup.as_deref());
            rollback(&done);
            return Err(error.to_string());
        }
        if let Err(error) = fs::rename(source, &dest) {
            undo_current(&dest, backup.as_deref(), displaced_backup.as_deref());
            rollback(&done);
            return Err(format!("install {}: {error}", dest.display()));
        }
        if exe_rel.is_some_and(|exe| relative_string(relative) == exe)
            && let Err(error) = make_executable(&dest)
        {
            let _ = fs::remove_file(&dest);
            undo_current(&dest, backup.as_deref(), displaced_backup.as_deref());
            rollback(&done);
            return Err(error);
        }
        done.push(Swapped {
            dest,
            backup,
            displaced_backup,
        });
    }
    Ok(done)
}

fn undo_current(dest: &Path, backup: Option<&Path>, displaced: Option<&Path>) {
    if let Some(backup) = backup {
        let _ = fs::rename(backup, dest);
    }
    restore_displaced(dest, displaced);
}

fn restore_displaced(dest: &Path, displaced: Option<&Path>) {
    if let Some(aside) = displaced {
        let _ = fs::rename(aside, backup_path(dest));
    }
}

fn rollback(swapped: &[Swapped]) {
    for item in swapped.iter().rev() {
        let _ = fs::remove_file(&item.dest);
        if let Some(backup) = &item.backup {
            let _ = fs::rename(backup, &item.dest);
        }
        restore_displaced(&item.dest, item.displaced_backup.as_deref());
    }
}

/// Running image → `name.old`, staged file → current name. On Windows the
/// running `.exe` can be renamed; the new image then takes the original name.
/// An existing `name.old` is renamed to `name.old.aside` first and put back if
/// this file, or a later file in the same apply, fails.
fn backup_path(dest: &Path) -> PathBuf {
    let name = dest.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    dest.with_file_name(format!("{name}.old"))
}

fn aside_path(dest: &Path) -> PathBuf {
    let name = dest.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    dest.with_file_name(format!("{name}.old.aside"))
}

fn self_check(exe: &Path) -> Result<(), String> {
    // Retry spawn failures. Parallel tests (and a tight runner) can return
    // EAGAIN from the first exec; an unsuccessful status is final.
    let mut last_error = String::from("self-check failed");
    for attempt in 0..5 {
        match Command::new(exe)
            .arg("--update-self-check")
            .env("KORANGAR_SKIP_UPDATE", "1")
            .output()
        {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if output.status.success() && stdout.contains(SELF_CHECK_TOKEN) {
                    return Ok(());
                }
                return Err("self-check failed".into());
            }
            Err(error) => {
                last_error = error.to_string();
                if attempt + 1 < 5 {
                    std::thread::sleep(Duration::from_millis(20 * (attempt + 1)));
                }
            }
        }
    }
    Err(last_error)
}

fn make_executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path).map_err(|error| error.to_string())?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).map_err(|error| error.to_string())?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

pub fn safe_relative(path: &str) -> Result<PathBuf, String> {
    if path.is_empty() || path.len() > 240 || path.contains('\0') || path.contains('\\') || path.starts_with('/') {
        return Err(format!("unsafe manifest path {path}"));
    }
    let parsed = Path::new(path);
    if parsed.is_absolute() {
        return Err(format!("unsafe manifest path {path}"));
    }
    let mut out = PathBuf::new();
    for component in parsed.components() {
        match component {
            Component::Normal(part) => {
                let text = part.to_string_lossy();
                if text == "updates" && out.as_os_str().is_empty() {
                    return Err("manifest path must not write into updates/".into());
                }
                out.push(part);
            }
            _ => return Err(format!("unsafe manifest path {path}")),
        }
    }
    if out.as_os_str().is_empty() {
        return Err("empty manifest path".into());
    }
    Ok(out)
}

pub fn join_url(base: &str, url: &str) -> Result<String, String> {
    if url.contains('\n') || url.contains('\r') || url.contains('\\') || url.contains("..") {
        return Err("unsafe artifact url".into());
    }
    if url.contains("://") {
        if url.starts_with("http://") || url.starts_with("file://") || is_allowed_https(url) {
            return Ok(url.to_string());
        }
        return Err("only http loopback, file, and allowlisted https release hosts are enabled".into());
    }
    if url.starts_with("//") {
        return Err("unsafe artifact url".into());
    }
    let (prefix, _) = base.rsplit_once('/').ok_or("manifest url has no directory")?;
    Ok(format!("{prefix}/{}", url.trim_start_matches("./")))
}

fn fetch_url(url: &str, max_bytes: u64) -> Result<Vec<u8>, String> {
    if max_bytes > FILE_LIMIT {
        return Err("artifact exceeds size cap".into());
    }
    if let Some(rest) = url.strip_prefix("file://") {
        let path = file_url_path(rest)?;
        let bytes = fs::read(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
        if bytes.len() as u64 > max_bytes {
            return Err(format!("{} exceeds declared size", path.display()));
        }
        return Ok(bytes);
    }
    if let Some(rest) = url.strip_prefix("http://") {
        let (host, port, path) = split_http(rest, 80)?;
        if !is_loopback(&host) {
            return Err("remote update hosts are disabled".into());
        }
        let mut stream = connect_tcp(&host, port)?;
        return read_http(&mut stream, &host, &path, max_bytes);
    }
    if let Some(rest) = url.strip_prefix("https://") {
        if !is_allowed_https(url) {
            return Err("https updates are limited to allowlisted release hosts".into());
        }
        let (host, port, path) = split_http(rest, 443)?;
        return https_get(&host, port, &path, max_bytes);
    }
    Err("only http loopback, file, and allowlisted https release hosts are enabled".into())
}

fn is_allowed_https(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let Ok((host, _, path)) = split_http(rest, 443) else {
        return false;
    };
    let (path_only, query) = match path.split_once('?') {
        Some((path, query)) => (path, query),
        None => (path.as_str(), ""),
    };
    match host.as_str() {
        "ro.turoran.com" => path_only == "/client/korangar/main/latest",
        "garage.turoran.com" => {
            let Some(name) = path_only.strip_prefix("/korangar-releases/main/") else {
                return false;
            };
            allowed_release_object_name(name) && query.contains("X-Amz-Signature=")
        }
        _ => false,
    }
}

fn is_fetch_allowed(url: &str) -> bool {
    if url.starts_with("file://") {
        return true;
    }
    if let Some(rest) = url.strip_prefix("http://") {
        return split_http(rest, 80).is_ok_and(|(host, ..)| is_loopback(&host));
    }
    is_allowed_https(url)
}

fn allowed_release_object_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 128 {
        return false;
    }
    if name.contains('/') || name.contains('\\') || name.contains('\0') || name.contains('%') {
        return false;
    }
    if name.contains("..") || name.starts_with('.') {
        return false;
    }
    if name == "manifest.jsonup" {
        return true;
    }
    if let Some(rest) = name.strip_prefix("korangar-linux-") {
        return is_hex_sha(rest);
    }
    if let Some(rest) = name.strip_prefix("korangar-windows-") {
        return rest.strip_suffix(".exe").is_some_and(is_hex_sha);
    }
    false
}

fn is_hex_sha(s: &str) -> bool {
    let n = s.len();
    (7..=64).contains(&n) && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn looks_like_json(bytes: &[u8]) -> bool {
    bytes.iter().copied().find(|b| !b.is_ascii_whitespace()).is_some_and(|b| b == b'{')
}

#[derive(Debug, Clone, Deserialize)]
struct ReleaseCatalog {
    manifest_url: String,
    #[serde(default)]
    files: Vec<CatalogFile>,
}

#[derive(Debug, Clone, Deserialize)]
struct CatalogFile {
    path: String,
    url: String,
}

fn parse_catalog(bytes: &[u8]) -> Result<ReleaseCatalog, String> {
    serde_json::from_slice(bytes).map_err(|_| "update catalog is not valid json".to_string())
}

fn artifact_url(manifest_url: &str, file: &manifest::ReleaseFile, catalog: Option<&ReleaseCatalog>) -> Result<String, String> {
    if let Some(catalog) = catalog {
        let Some(entry) = catalog.files.iter().find(|entry| entry.path == file.path) else {
            return Err(format!("catalog is missing a presigned url for {}", file.path));
        };
        if !is_fetch_allowed(&entry.url) {
            return Err(format!("catalog file url for {} is not allowlisted", file.path));
        }
        return Ok(entry.url.clone());
    }
    join_url(manifest_url, &file.url)
}

fn file_url_path(rest: &str) -> Result<PathBuf, String> {
    let path = rest.strip_prefix("localhost").unwrap_or(rest);
    if !path.starts_with('/') || path.contains('\0') {
        return Err("file url must be an absolute path".into());
    }
    Ok(PathBuf::from(path))
}

fn is_loopback(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

fn installed_build(install_root: &Path) -> u64 {
    let path = install_root.join("updates").join("applied.build");
    fs::read_to_string(&path)
        .ok()
        .and_then(|text| text.trim().parse::<u64>().ok())
        .unwrap_or(0)
}

fn is_unreachable_fetch(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("connection")
        || lower.contains("timed out")
        || lower.contains("timeout")
        || lower.contains("connection refused")
        || lower.contains("not found")
        || lower.contains("no such file")
        || lower.contains("failed to lookup")
        || lower.contains("no such host")
        || lower.contains("name or service not known")
        || lower.contains("temporary failure")
        || lower.contains("network is unreachable")
        || lower.contains("http 404")
        || lower.contains("http 502")
        || lower.contains("http 503")
        || lower.contains("http 504")
        || lower.contains("read ")
}

fn split_http(rest: &str, default_port: u16) -> Result<(String, u16, String), String> {
    if rest.contains('@') {
        return Err("userinfo in update urls is rejected".into());
    }
    let (authority, path) = match rest.split_once('/') {
        Some((authority, path)) => (authority, format!("/{path}")),
        None => (rest, "/".to_string()),
    };
    let path_only = path.split_once('?').map(|(p, _)| p).unwrap_or(path.as_str());
    if authority.is_empty() || path_only.contains("..") {
        return Err("unsafe http url".into());
    }
    let (host, port) = if let Some(host) = authority.strip_prefix('[') {
        let (host, tail) = host.split_once(']').ok_or("bad ipv6 url")?;
        let port = parse_port(tail, default_port)?;
        (host.to_string(), port)
    } else if let Some((host, port)) = authority.rsplit_once(':') {
        (host.to_string(), port.parse::<u16>().map_err(|_| "bad port")?)
    } else {
        (authority.to_string(), default_port)
    };
    if host.is_empty() {
        return Err("missing host".into());
    }
    Ok((host, port, path))
}

fn parse_port(tail: &str, default_port: u16) -> Result<u16, String> {
    if tail.is_empty() {
        return Ok(default_port);
    }
    let port = tail.strip_prefix(':').ok_or("bad port")?;
    port.parse::<u16>().map_err(|_| "bad port".to_string())
}

fn connect_tcp(host: &str, port: u16) -> Result<TcpStream, String> {
    let address = if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    };
    let stream = TcpStream::connect(address).map_err(|error| error.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(15)))
        .map_err(|error| error.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(15)))
        .map_err(|error| error.to_string())?;
    Ok(stream)
}

fn https_get(host: &str, port: u16, path: &str, max_bytes: u64) -> Result<Vec<u8>, String> {
    use std::sync::Arc;

    install_rustls_crypto();
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder().with_root_certificates(roots).with_no_client_auth();
    let name = rustls::pki_types::ServerName::try_from(host.to_string()).map_err(|_| "bad https server name")?;
    let mut connection = rustls::ClientConnection::new(Arc::new(config), name).map_err(|error| error.to_string())?;
    let mut tcp = connect_tcp(host, port)?;
    let mut tls = rustls::Stream::new(&mut connection, &mut tcp);
    read_http(&mut tls, host, path, max_bytes)
}

fn read_http<S: Read + Write>(stream: &mut S, host: &str, path: &str, max_bytes: u64) -> Result<Vec<u8>, String> {
    let request = format!("GET {path} HTTP/1.0\r\nHost: {host}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).map_err(|error| error.to_string())?;
    let mut raw = Vec::new();
    let mut chunk = [0u8; 8192];
    let cap = max_bytes as usize + 4096;
    loop {
        let read = stream.read(&mut chunk).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        if raw.len() + read > cap {
            return Err("http body exceeds limit".into());
        }
        raw.extend_from_slice(&chunk[..read]);
    }
    let split = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or("truncated http response")?;
    let header = std::str::from_utf8(&raw[..split]).map_err(|_| "http header is not utf-8")?;
    let mut lines = header.split("\r\n");
    let status = lines.next().unwrap_or("");
    let mut parts = status.split_whitespace();
    let version = parts.next().unwrap_or("");
    let code = parts.next().unwrap_or("");
    if !version.starts_with("HTTP/1.") || code != "200" {
        return Err(format!("http status rejected: {status}"));
    }
    let mut length = None;
    for line in lines {
        let lower = line.to_ascii_lowercase();
        if let Some(value) = lower.strip_prefix("content-length:") {
            length = Some(value.trim().parse::<usize>().map_err(|_| "bad content-length")?);
        }
    }
    let length = length.ok_or("content-length required")?;
    if length as u64 > max_bytes {
        return Err("content-length exceeds limit".into());
    }
    let body = &raw[split + 4..];
    if body.len() != length {
        return Err("http body length mismatch".into());
    }
    Ok(body.to_vec())
}

fn write_full(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let tmp = path.parent().unwrap_or(Path::new(".")).join(format!(
        "{}.partial",
        path.file_name().and_then(|name| name.to_str()).unwrap_or("update")
    ));
    {
        let mut file = fs::File::create(&tmp).map_err(|error| error.to_string())?;
        file.write_all(bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
    }
    fs::rename(&tmp, path).map_err(|error| error.to_string())?;
    Ok(())
}

fn relative_string(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::install_rustls_crypto;

    #[test]
    fn rustls_provider_can_be_installed_twice() {
        install_rustls_crypto();
        install_rustls_crypto();
    }
}
