//! CLI for `korangar-sign-release`. Key material is never written into a git
//! worktree.

use std::path::{Component, Path, PathBuf};
use std::{env, fs};

use crate::manifest::{self, ReleaseFile, ReleaseManifest};

pub fn dispatch(args: impl IntoIterator<Item = String>) -> Result<(), String> {
    let args: Vec<String> = args.into_iter().collect();
    let Some(command) = args.first().map(String::as_str) else {
        return Err(usage());
    };
    match command {
        "keygen" => keygen(&args[1..]),
        "public-key" => public_key(&args[1..]),
        "sign" => sign(&args[1..]),
        "-h" | "--help" | "help" => {
            println!("{}", usage());
            Ok(())
        }
        other => Err(format!("unknown command {other}\n{}", usage())),
    }
}

fn usage() -> String {
    "\
commands:
  keygen --out PATH
      Write a new 32-byte Ed25519 seed. PATH must be outside the git worktree.
  public-key (--private-key PATH | uses KORANGAR_UPDATE_SIGNING_KEY)
      Print the raw public key and a Rust const for EMBEDDED_UPDATE_PUBLIC_KEY.
  sign --version VER --out MANIFEST (--private-key PATH) --file REL=SOURCE [--url REL_URL]...
      Hash each SOURCE file, build the canonical payload, and write a signed envelope.
      Repeat --file. A following --url overrides the default relative URL for the previous file.
Keys: KORANGAR_UPDATE_SIGNING_KEY is either 64 hex seed chars or a filesystem path.
Never pass a path inside this repo. Generate with: korangar-sign-release keygen --out /secure/korangar-update.seed"
        .into()
}

fn keygen(args: &[String]) -> Result<(), String> {
    let out = flag(args, "--out")?;
    let path = PathBuf::from(out);
    assert_key_path_outside_repo(&path)?;
    if path.exists() {
        return Err("refusing to overwrite an existing key file".into());
    }
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let seed = manifest::generate_seed()?;
    write_secret(&path, &seed)?;
    let public = manifest::public_key_bytes(&seed)?;
    println!("wrote {}", path.display());
    println!("public {}", hex_encode(&public));
    Ok(())
}

fn public_key(args: &[String]) -> Result<(), String> {
    let seed = load_seed(args)?;
    let public = manifest::public_key_bytes(&seed)?;
    println!("{}", hex_encode(&public));
    println!("{}", rust_const(&public));
    Ok(())
}

fn sign(args: &[String]) -> Result<(), String> {
    let version = flag(args, "--version")?.to_string();
    let out = PathBuf::from(flag(args, "--out")?);
    let seed = load_seed(args)?;
    let mut files = Vec::new();
    let mut index = 0;
    while index < args.len() {
        if args[index] != "--file" {
            index += 1;
            continue;
        }
        let spec = args.get(index + 1).ok_or("--file needs REL=SOURCE")?;
        let (rel, source) = spec.split_once('=').ok_or("--file must be REL=SOURCE")?;
        index += 2;
        let url = if args.get(index).map(String::as_str) == Some("--url") {
            let url = args.get(index + 1).ok_or("--url needs a value")?.clone();
            index += 2;
            url
        } else {
            rel.to_string()
        };
        let bytes = fs::read(source).map_err(|error| format!("read {source}: {error}"))?;
        files.push(ReleaseFile {
            path: rel.to_string(),
            sha256: manifest::sha256_hex(&bytes),
            size: bytes.len() as u64,
            url,
        });
    }
    if files.is_empty() {
        return Err("sign requires at least one --file REL=SOURCE".into());
    }
    let manifest = ReleaseManifest { version, files };
    let payload = manifest::canonical_payload(&manifest)?;
    let envelope = manifest::sign_envelope(&payload, &seed)?;
    if let Some(parent) = out.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(&out, envelope).map_err(|error| error.to_string())?;
    println!("wrote {}", out.display());
    Ok(())
}

fn load_seed(args: &[String]) -> Result<[u8; 32], String> {
    if let Some(path) = optional_flag(args, "--private-key") {
        return read_seed_file(Path::new(path));
    }
    match env::var("KORANGAR_UPDATE_SIGNING_KEY") {
        Ok(value) => parse_seed_or_path(&value),
        Err(_) => Err("pass --private-key PATH or set KORANGAR_UPDATE_SIGNING_KEY (path or 64 hex chars)".into()),
    }
}

fn parse_seed_or_path(value: &str) -> Result<[u8; 32], String> {
    let trimmed = value.trim();
    if trimmed.len() == 64 && trimmed.chars().all(|char| char.is_ascii_hexdigit()) {
        return decode_seed_hex(trimmed);
    }
    read_seed_file(Path::new(trimmed))
}

pub fn read_seed_file(path: &Path) -> Result<[u8; 32], String> {
    let bytes = fs::read(path).map_err(|error| format!("read signing key: {error}"))?;
    if bytes.len() == 32 {
        let mut seed = [0u8; 32];
        seed.copy_from_slice(&bytes);
        return Ok(seed);
    }
    let text = String::from_utf8(bytes).map_err(|_| "signing key must be 32 raw bytes or 64 hex chars".to_string())?;
    let trimmed = text.trim();
    if trimmed.len() == 64 && trimmed.chars().all(|char| char.is_ascii_hexdigit()) {
        return decode_seed_hex(trimmed);
    }
    Err("signing key must be 32 raw bytes or 64 hex chars".into())
}

fn decode_seed_hex(text: &str) -> Result<[u8; 32], String> {
    let mut seed = [0u8; 32];
    for (index, byte) in seed.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).map_err(|_| "invalid hex seed".to_string())?;
    }
    Ok(seed)
}

fn write_secret(path: &Path, seed: &[u8; 32]) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .map_err(|error| error.to_string())?;
        file.write_all(seed).map_err(|error| error.to_string())?;
    }
    #[cfg(not(unix))]
    {
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|error| error.to_string())?;
        file.write_all(seed).map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// Refuse destinations that resolve inside a `.git` worktree found from the
/// cwd.
pub fn assert_key_path_outside_repo(path: &Path) -> Result<(), String> {
    let cwd = env::current_dir().map_err(|error| error.to_string())?;
    let Some(repo) = find_git_root(&cwd) else {
        return Ok(());
    };
    let joined = if path.is_absolute() { path.to_path_buf() } else { cwd.join(path) };
    let normalized = normalize_lexical(&joined);
    if normalized.starts_with(&repo) {
        return Err(format!(
            "refusing to write a private key inside the git worktree ({})",
            repo.display()
        ));
    }
    Ok(())
}

fn find_git_root(start: &Path) -> Option<PathBuf> {
    let mut current = Some(start);
    while let Some(dir) = current {
        if dir.join(".git").exists() {
            return Some(dir.to_path_buf());
        }
        current = dir.parent();
    }
    None
}

fn normalize_lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

fn flag<'a>(args: &'a [String], name: &str) -> Result<&'a str, String> {
    optional_flag(args, name).ok_or_else(|| format!("missing {name}"))
}

fn optional_flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.windows(2).find(|pair| pair[0] == name).map(|pair| pair[1].as_str())
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

fn rust_const(public: &[u8; 32]) -> String {
    let body = public.iter().map(|byte| byte.to_string()).collect::<Vec<_>>().join(", ");
    format!("pub const EMBEDDED_UPDATE_PUBLIC_KEY: [u8; 32] = [{body}];")
}
