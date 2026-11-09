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
//! `version` is `[0-9A-Za-z._+-]{1,64}`. Paths are relative, slash-separated,
//! with no `.` or `..` components. `url` is either that same relative path, an
//! `http://` loopback URL, or a `file://` URL. Non-loopback and `https` URLs
//! are rejected so this build cannot phone a CDN.
//!
//! # Envelope
//!
//! ```text
//! KRUP1SIG\n
//! <signature lowercase hex, 128 chars>\n
//! <canonical payload, which itself starts with KRUP1\n>
//! ```
//!
//! The public key is [`EMBEDDED_UPDATE_PUBLIC_KEY`] (all zeros until a release
//! build pastes the const from `korangar-sign-release public-key`). The private
//! seed stays outside the git tree. See `docs/signed-auto-update.md`.

pub mod cli;
pub mod manifest;

use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::Duration;

pub use manifest::EMBEDDED_UPDATE_PUBLIC_KEY as UPDATE_PUBLIC_KEY;
use manifest::ReleaseManifest;

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
    let envelope = fetch_url(url, MANIFEST_LIMIT as u64)?;
    let manifest = manifest::open_envelope(&envelope, request.public_key)?;
    if manifest.version == request.current_version {
        return Ok(StartupOutcome::UpToDate);
    }
    let marker = request.install_root.join("updates").join("applied.version");
    if fs::read_to_string(&marker).ok().as_deref().map(str::trim) == Some(manifest.version.as_str()) {
        return Ok(StartupOutcome::UpToDate);
    }
    apply_manifest(&manifest, url, request)
}

fn apply_manifest(manifest: &ReleaseManifest, manifest_url: &str, request: &StartupRequest<'_>) -> Result<StartupOutcome, String> {
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
        let bytes = fetch_url(&join_url(manifest_url, &file.url)?, file.size)?;
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
    let _ = fs::remove_dir_all(&staging);

    let relaunched = if request.spawn_relaunch {
        match Command::new(request.current_exe).args(request.relaunch_args).spawn() {
            Ok(_) => true,
            Err(error) => {
                rollback(&swapped);
                let _ = fs::remove_file(request.install_root.join("updates").join("applied.version"));
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
        let backup = if dest.exists() {
            if backup_path.exists() {
                fs::remove_file(&backup_path).map_err(|error| error.to_string())?;
            }
            fs::rename(&dest, &backup_path).map_err(|error| {
                rollback(&done);
                format!("backup {}: {error}", dest.display())
            })?;
            Some(backup_path)
        } else {
            None
        };
        if let Some(parent) = dest.parent()
            && let Err(error) = fs::create_dir_all(parent)
        {
            if let Some(backup) = &backup {
                let _ = fs::rename(backup, &dest);
            }
            rollback(&done);
            return Err(error.to_string());
        }
        if let Err(error) = fs::rename(source, &dest) {
            if let Some(backup) = &backup {
                let _ = fs::rename(backup, &dest);
            }
            rollback(&done);
            return Err(format!("install {}: {error}", dest.display()));
        }
        if exe_rel.is_some_and(|exe| relative_string(relative) == exe)
            && let Err(error) = make_executable(&dest)
        {
            let _ = fs::remove_file(&dest);
            if let Some(backup) = &backup {
                let _ = fs::rename(backup, &dest);
            }
            rollback(&done);
            return Err(error);
        }
        done.push(Swapped { dest, backup });
    }
    Ok(done)
}

fn rollback(swapped: &[Swapped]) {
    for item in swapped.iter().rev() {
        let _ = fs::remove_file(&item.dest);
        if let Some(backup) = &item.backup {
            let _ = fs::rename(backup, &item.dest);
        }
    }
}

/// Running image → `name.old`, staged file → current name. On Windows the
/// running `.exe` can be renamed; the new image then takes the original name.
fn backup_path(dest: &Path) -> PathBuf {
    let name = dest.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    dest.with_file_name(format!("{name}.old"))
}

fn self_check(exe: &Path) -> Result<(), String> {
    let output = Command::new(exe)
        .arg("--update-self-check")
        .env("KORANGAR_SKIP_UPDATE", "1")
        .output()
        .map_err(|error| error.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if output.status.success() && stdout.contains(SELF_CHECK_TOKEN) {
        Ok(())
    } else {
        Err("self-check failed".into())
    }
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
        if url.starts_with("http://") || url.starts_with("file://") {
            return Ok(url.to_string());
        }
        return Err("only http loopback and file URLs are enabled".into());
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
        let (host, port, path) = split_http(rest)?;
        if !is_loopback(&host) {
            return Err("remote update hosts are disabled".into());
        }
        return http_get(&host, port, &path, max_bytes);
    }
    Err("only http loopback and file URLs are enabled".into())
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

fn split_http(rest: &str) -> Result<(String, u16, String), String> {
    if rest.contains('@') {
        return Err("userinfo in update urls is rejected".into());
    }
    let (authority, path) = match rest.split_once('/') {
        Some((authority, path)) => (authority, format!("/{path}")),
        None => (rest, "/".to_string()),
    };
    if authority.is_empty() || path.contains("..") {
        return Err("unsafe http url".into());
    }
    let (host, port) = if let Some(host) = authority.strip_prefix('[') {
        let (host, tail) = host.split_once(']').ok_or("bad ipv6 url")?;
        let port = parse_port(tail)?;
        (host.to_string(), port)
    } else if let Some((host, port)) = authority.rsplit_once(':') {
        (host.to_string(), port.parse::<u16>().map_err(|_| "bad port")?)
    } else {
        (authority.to_string(), 80)
    };
    if host.is_empty() {
        return Err("missing host".into());
    }
    Ok((host, port, path))
}

fn parse_port(tail: &str) -> Result<u16, String> {
    if tail.is_empty() {
        return Ok(80);
    }
    let port = tail.strip_prefix(':').ok_or("bad port")?;
    port.parse::<u16>().map_err(|_| "bad port".to_string())
}

fn http_get(host: &str, port: u16, path: &str, max_bytes: u64) -> Result<Vec<u8>, String> {
    let address = if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    };
    let mut stream = TcpStream::connect(address).map_err(|error| error.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| error.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| error.to_string())?;
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
