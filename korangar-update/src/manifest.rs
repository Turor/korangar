//! Canonical KRUP1 payload and detached Ed25519 envelope. See the crate docs.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};

/// Turoran release public key compiled into the client. The matching Ed25519
/// seed stays on the signing controller and is never stored in this repository.
/// An all-zero key still fails closed.
pub const EMBEDDED_UPDATE_PUBLIC_KEY: [u8; 32] = [
    251, 61, 211, 214, 185, 225, 149, 94, 224, 58, 71, 134, 89, 111, 125, 85, 98, 183, 217, 109, 123, 108, 23, 126, 109, 56, 207, 122, 188, 167,
    222, 24,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseFile {
    pub path: String,
    pub sha256: String,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseManifest {
    pub version: String,
    /// Monotonic build counter or unix timestamp. A manifest is applied only
    /// when this is newer than the installed build.
    pub build: u64,
    pub files: Vec<ReleaseFile>,
}

pub fn generate_seed() -> Result<[u8; 32], String> {
    let mut seed = [0u8; 32];
    getrandom::getrandom(&mut seed).map_err(|error| error.to_string())?;
    if seed == [0u8; 32] {
        return Err("refusing an all-zero seed".into());
    }
    Ok(seed)
}

pub fn public_key_bytes(seed: &[u8; 32]) -> Result<[u8; 32], String> {
    Ok(SigningKey::from_bytes(seed).verifying_key().to_bytes())
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    hex_encode(&digest)
}

pub fn canonical_payload(manifest: &ReleaseManifest) -> Result<String, String> {
    validate_version(&manifest.version)?;
    validate_build(manifest.build)?;
    if manifest.files.is_empty() {
        return Err("manifest has no files".into());
    }
    let mut files = manifest.files.clone();
    files.sort_by(|left, right| left.path.as_bytes().cmp(right.path.as_bytes()));
    let mut seen = std::collections::BTreeSet::new();
    let mut body = String::new();
    body.push_str("KRUP1\n");
    body.push_str(&manifest.version);
    body.push('\n');
    body.push_str(&manifest.build.to_string());
    body.push('\n');
    body.push_str(&files.len().to_string());
    body.push('\n');
    for file in &files {
        if !seen.insert(file.path.as_str()) {
            return Err(format!("duplicate path {}", file.path));
        }
        validate_token("path", &file.path)?;
        validate_token("url", &file.url)?;
        if file.sha256.len() != 64
            || !file
                .sha256
                .chars()
                .all(|char| char.is_ascii_hexdigit() && !char.is_ascii_uppercase())
        {
            return Err(format!("sha256 must be 64 lowercase hex chars for {}", file.path));
        }
        body.push_str(&file.path);
        body.push('\n');
        body.push_str(&file.sha256);
        body.push('\n');
        body.push_str(&file.size.to_string());
        body.push('\n');
        body.push_str(&file.url);
        body.push('\n');
    }
    Ok(body)
}

pub fn sign_envelope(payload: &str, seed: &[u8; 32]) -> Result<String, String> {
    if !payload.starts_with("KRUP1\n") {
        return Err("payload is not a KRUP1 document".into());
    }
    let signature = SigningKey::from_bytes(seed).sign(payload.as_bytes()).to_bytes();
    let mut envelope = String::new();
    envelope.push_str("KRUP1SIG\n");
    envelope.push_str(&hex_encode(&signature));
    envelope.push('\n');
    envelope.push_str(payload);
    Ok(envelope)
}

pub fn open_envelope(bytes: &[u8], public_key: &[u8; 32]) -> Result<ReleaseManifest, String> {
    if public_key == &[0u8; 32] {
        return Err("embedded update public key is unset".into());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "manifest is not utf-8")?;
    let rest = text.strip_prefix("KRUP1SIG\n").ok_or("missing KRUP1SIG header")?;
    let (signature_hex, payload) = rest.split_once('\n').ok_or("truncated signature envelope")?;
    if signature_hex.len() != 128
        || !signature_hex
            .chars()
            .all(|char| char.is_ascii_hexdigit() && !char.is_ascii_uppercase())
    {
        return Err("signature must be 128 lowercase hex chars".into());
    }
    if !payload.starts_with("KRUP1\n") {
        return Err("signed payload must be canonical KRUP1".into());
    }
    let mut signature = [0u8; 64];
    for (index, byte) in signature.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&signature_hex[index * 2..index * 2 + 2], 16).map_err(|_| "invalid signature hex")?;
    }
    let key = VerifyingKey::from_bytes(public_key).map_err(|_| "invalid embedded update public key")?;
    key.verify(payload.as_bytes(), &Signature::from_bytes(&signature))
        .map_err(|_| "manifest signature rejected")?;
    parse_payload(payload)
}

pub fn parse_payload(payload: &str) -> Result<ReleaseManifest, String> {
    let mut lines = payload.split('\n');
    if lines.next() != Some("KRUP1") {
        return Err("payload magic mismatch".into());
    }
    let version = lines.next().ok_or("missing version")?.to_string();
    validate_version(&version)?;
    let build: u64 = lines.next().ok_or("missing build")?.parse().map_err(|_| "bad build")?;
    validate_build(build)?;
    let count: usize = lines.next().ok_or("missing file count")?.parse().map_err(|_| "bad file count")?;
    if count == 0 || count > 10_000 {
        return Err("file count out of range".into());
    }
    let mut files = Vec::with_capacity(count);
    for _ in 0..count {
        let path = lines.next().ok_or("truncated file entry")?.to_string();
        let sha256 = lines.next().ok_or("truncated file entry")?.to_string();
        let size = lines.next().ok_or("truncated file entry")?.parse().map_err(|_| "bad size")?;
        let url = lines.next().ok_or("truncated file entry")?.to_string();
        files.push(ReleaseFile { path, sha256, size, url });
    }
    if lines.next() != Some("") || lines.next().is_some() {
        return Err("trailing manifest data".into());
    }
    let canonical = canonical_payload(&ReleaseManifest {
        version: version.clone(),
        build,
        files: files.clone(),
    })?;
    if canonical != payload {
        return Err("payload is not canonical".into());
    }
    Ok(ReleaseManifest { version, build, files })
}

fn validate_version(version: &str) -> Result<(), String> {
    if (1..=64).contains(&version.len())
        && version
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || matches!(char, '.' | '_' | '+' | '-'))
    {
        Ok(())
    } else {
        Err(format!("unsafe version {version}"))
    }
}

fn validate_build(build: u64) -> Result<(), String> {
    if build == 0 {
        return Err("build must be a positive monotonic counter or timestamp".into());
    }
    Ok(())
}

/// True when `incoming` is a strictly newer build than `installed`.
/// `installed == 0` means no build has been applied yet.
pub fn is_newer_build(incoming: u64, installed: u64) -> bool {
    incoming > installed
}

fn validate_token(label: &str, value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 512 || value.chars().any(|char| char.is_control() || char == ' ') {
        return Err(format!("unsafe {label}"));
    }
    Ok(())
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xF) as usize] as char);
    }
    out
}
