use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::{fs, thread};

use korangar_update::manifest::{self, ReleaseFile, ReleaseManifest};
use korangar_update::{StartupOutcome, StartupRequest, should_attempt};

const SEED: [u8; 32] = [7u8; 32];

fn public_key() -> &'static [u8; 32] {
    use std::sync::OnceLock;
    static KEY: OnceLock<[u8; 32]> = OnceLock::new();
    KEY.get_or_init(|| manifest::public_key_bytes(&SEED).unwrap())
}

fn manifest_for(version: &str, files: Vec<ReleaseFile>) -> String {
    let payload = manifest::canonical_payload(&ReleaseManifest {
        version: version.into(),
        files,
    })
    .unwrap();
    manifest::sign_envelope(&payload, &SEED).unwrap()
}

fn file_entry(path: &str, bytes: &[u8], url: &str) -> ReleaseFile {
    ReleaseFile {
        path: path.into(),
        sha256: manifest::sha256_hex(bytes),
        size: bytes.len() as u64,
        url: url.into(),
    }
}

fn write_script(path: &Path, ok: bool) {
    let body = if ok {
        "#!/bin/sh\nif [ \"$1\" = \"--update-self-check\" ]; then echo update-self-check-ok; exit 0; fi\necho relaunched > \"$0.ran\"\n"
    } else {
        "#!/bin/sh\nexit 1\n"
    };
    fs::write(path, body).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }
}

fn temp_dir() -> PathBuf {
    let path = std::env::temp_dir().join(format!("korangar-update-{}-{}", std::process::id(), unique()));
    fs::create_dir_all(&path).unwrap();
    path
}

fn unique() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

#[test]
fn canonical_form_sorts_files_and_rejects_tampering() {
    let hello = b"hello";
    let files = vec![file_entry("b.txt", b"b", "b.txt"), file_entry("a.txt", hello, "a.txt")];
    let payload = manifest::canonical_payload(&ReleaseManifest {
        version: "1.0.0".into(),
        files,
    })
    .unwrap();
    let expected = format!(
        "KRUP1\n1.0.0\n2\na.txt\n{}\n5\na.txt\nb.txt\n{}\n1\nb.txt\n",
        manifest::sha256_hex(hello),
        manifest::sha256_hex(b"b")
    );
    assert_eq!(payload, expected);
    let envelope = manifest::sign_envelope(&payload, &SEED).unwrap();
    let opened = manifest::open_envelope(envelope.as_bytes(), public_key()).unwrap();
    assert_eq!(opened.files[0].path, "a.txt");
    let mut bad = envelope.into_bytes();
    let last = bad.len() - 2;
    bad[last] ^= 1;
    assert!(manifest::open_envelope(&bad, public_key()).is_err());
    assert!(manifest::open_envelope(b"KRUP1SIG\n", public_key()).is_err());
    assert!(manifest::open_envelope(envelope_placeholder().as_bytes(), &[0u8; 32]).is_err());
}

fn envelope_placeholder() -> String {
    "KRUP1SIG\n".into()
}

#[test]
fn unsafe_paths_and_remote_urls_are_rejected() {
    assert!(korangar_update::safe_relative("../korangar.exe").is_err());
    assert!(korangar_update::safe_relative("/etc/passwd").is_err());
    assert!(korangar_update::safe_relative("updates/applied.version").is_err());
    assert!(korangar_update::safe_relative("runtime/data.bin").is_ok());
    assert!(korangar_update::join_url("http://127.0.0.1:9/manifest.krup", "https://cdn.example/a").is_err());
    assert!(korangar_update::join_url("http://127.0.0.1:9/manifest.krup", "../x").is_err());
    assert_eq!(
        korangar_update::join_url("file:///tmp/stage/manifest.krup", "korangar.exe").unwrap(),
        "file:///tmp/stage/korangar.exe"
    );
}

#[test]
fn skip_env_and_unset_key_fail_closed() {
    assert!(!should_attempt(Some("1"), Some("file:///tmp/m.krup")));
    assert!(!should_attempt(None, None));
    assert!(!should_attempt(None, Some("  ")));
    assert!(should_attempt(None, Some("file:///tmp/m.krup")));
    let dir = temp_dir();
    let exe = dir.join("korangar.exe");
    write_script(&exe, true);
    let error = korangar_update::run_startup_update(&StartupRequest {
        skip_env: None,
        manifest_url: Some("file:///tmp/does-not-matter.krup"),
        current_version: "0.1.1",
        install_root: &dir,
        current_exe: &exe,
        public_key: &[0u8; 32],
        relaunch_args: &[],
        spawn_relaunch: false,
    })
    .unwrap_err();
    assert!(error.contains("unset"), "{error}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn file_url_apply_swaps_exe_and_companion_and_rolls_back() {
    let root = temp_dir();
    let publish = root.join("publish");
    let install = root.join("install");
    fs::create_dir_all(&publish).unwrap();
    fs::create_dir_all(&install).unwrap();
    let new_exe = b"#!/bin/sh\nif [ \"$1\" = \"--update-self-check\" ]; then echo update-self-check-ok; exit 0; fi\nexit 0\n";
    let companion = b"new-data";
    fs::write(publish.join("korangar.exe"), new_exe).unwrap();
    fs::write(publish.join("runtime.dat"), companion).unwrap();
    let manifest_path = publish.join("manifest.krup");
    let envelope = manifest_for("9.9.9", vec![
        file_entry("runtime.dat", companion, "runtime.dat"),
        file_entry("korangar.exe", new_exe, "korangar.exe"),
    ]);
    fs::write(&manifest_path, &envelope).unwrap();
    let exe = install.join("korangar.exe");
    write_script(&exe, true);
    fs::write(install.join("runtime.dat"), b"old-data").unwrap();
    let url = format!("file://{}", manifest_path.display());
    let outcome = korangar_update::run_startup_update(&request(&install, &exe, &url, "0.1.1")).unwrap();
    assert_eq!(outcome, StartupOutcome::Applied { relaunched: false });
    assert_eq!(fs::read(install.join("runtime.dat")).unwrap(), companion);
    assert_eq!(fs::read(install.join("korangar.exe")).unwrap(), new_exe);
    assert_eq!(fs::read(install.join("runtime.dat.old")).unwrap(), b"old-data");
    assert!(
        fs::read(install.join("updates/applied.version"))
            .unwrap()
            .starts_with("9.9.9".as_bytes())
    );
    let again = korangar_update::run_startup_update(&request(&install, &exe, &url, "0.1.1")).unwrap();
    assert_eq!(again, StartupOutcome::UpToDate);

    fs::write(install.join("korangar.exe"), b"#!/bin/sh\nexit 1\n").unwrap();
    fs::write(install.join("runtime.dat"), b"keep").unwrap();
    let bad_publish = publish.join("bad-exe");
    fs::write(&bad_publish, b"#!/bin/sh\nexit 1\n").unwrap();
    let bad_bytes = fs::read(&bad_publish).unwrap();
    let bad_manifest = publish.join("bad.krup");
    fs::write(
        &bad_manifest,
        manifest_for("9.9.10", vec![
            file_entry("korangar.exe", &bad_bytes, "bad-exe"),
            file_entry("runtime.dat", b"nope", "missing-would-not-run"),
        ]),
    )
    .unwrap();
    // Hash matches bad-exe but self-check fails. Companion URL is a real second
    // file.
    fs::write(publish.join("side.dat"), b"nope").unwrap();
    fs::write(
        &bad_manifest,
        manifest_for("9.9.10", vec![
            file_entry("korangar.exe", &bad_bytes, "bad-exe"),
            file_entry("runtime.dat", b"nope", "side.dat"),
        ]),
    )
    .unwrap();
    let before_exe = fs::read(install.join("korangar.exe")).unwrap();
    let error =
        korangar_update::run_startup_update(&request(&install, &exe, &format!("file://{}", bad_manifest.display()), "0.1.1")).unwrap_err();
    assert!(error.contains("self-check"), "{error}");
    assert_eq!(fs::read(install.join("korangar.exe")).unwrap(), before_exe);
    assert_eq!(fs::read(install.join("runtime.dat")).unwrap(), b"keep");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn existing_old_backup_is_kept_until_swap_commits_and_a_blocked_aside_rolls_back() {
    let root = temp_dir();
    let publish = root.join("publish");
    let install = root.join("install");
    fs::create_dir_all(&publish).unwrap();
    fs::create_dir_all(&install).unwrap();
    let new_exe = b"#!/bin/sh\nif [ \"$1\" = \"--update-self-check\" ]; then echo update-self-check-ok; exit 0; fi\nexit 0\n";
    fs::write(publish.join("korangar.exe"), new_exe).unwrap();
    let exe = install.join("korangar.exe");
    write_script(&exe, true);
    fs::write(install.join("korangar.exe.old"), b"ancient-exe").unwrap();
    let manifest_path = publish.join("ok.krup");
    fs::write(
        &manifest_path,
        manifest_for("8.0.0", vec![file_entry("korangar.exe", new_exe, "korangar.exe")]),
    )
    .unwrap();
    let outcome = korangar_update::run_startup_update(&request(
        &install,
        &exe,
        &format!("file://{}", manifest_path.display()),
        "0.1.1",
    ))
    .unwrap();
    assert_eq!(outcome, StartupOutcome::Applied { relaunched: false });
    assert_eq!(fs::read(&exe).unwrap(), new_exe);
    assert!(fs::read(install.join("korangar.exe.old")).unwrap().starts_with(b"#!/bin/sh"));
    assert!(!install.join("korangar.exe.old.aside").exists());

    fs::write(install.join("a.txt"), b"old-a").unwrap();
    fs::write(install.join("b.txt"), b"old-b").unwrap();
    fs::write(install.join("b.txt.old"), b"older-b").unwrap();
    fs::create_dir(install.join("b.txt.old.aside")).unwrap();
    fs::write(publish.join("a.txt"), b"new-a").unwrap();
    fs::write(publish.join("b.txt"), b"new-b").unwrap();
    let blocked = publish.join("blocked.krup");
    fs::write(
        &blocked,
        manifest_for("8.0.1", vec![
            file_entry("b.txt", b"new-b", "b.txt"),
            file_entry("a.txt", b"new-a", "a.txt"),
        ]),
    )
    .unwrap();
    let error =
        korangar_update::run_startup_update(&request(&install, &exe, &format!("file://{}", blocked.display()), "0.1.1")).unwrap_err();
    assert!(error.contains("existing backup"), "{error}");
    assert_eq!(fs::read(install.join("a.txt")).unwrap(), b"old-a");
    assert!(!install.join("a.txt.old").exists());
    assert_eq!(fs::read(install.join("b.txt")).unwrap(), b"old-b");
    assert_eq!(fs::read(install.join("b.txt.old")).unwrap(), b"older-b");
    assert!(install.join("b.txt.old.aside").is_dir());
    assert_eq!(fs::read(&exe).unwrap(), new_exe);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn hash_mismatch_does_not_touch_install() {
    let root = temp_dir();
    let publish = root.join("publish");
    let install = root.join("install");
    fs::create_dir_all(&publish).unwrap();
    fs::create_dir_all(&install).unwrap();
    let exe_bytes = b"#!/bin/sh\necho update-self-check-ok\n";
    fs::write(publish.join("korangar.exe"), exe_bytes).unwrap();
    let mut entry = file_entry("korangar.exe", exe_bytes, "korangar.exe");
    entry.sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into();
    let manifest_path = publish.join("manifest.krup");
    fs::write(&manifest_path, manifest_for("3.0.0", vec![entry])).unwrap();
    let exe = install.join("korangar.exe");
    write_script(&exe, true);
    let error = korangar_update::run_startup_update(&request(
        &install,
        &exe,
        &format!("file://{}", manifest_path.display()),
        "0.1.1",
    ))
    .unwrap_err();
    assert!(error.contains("sha256"), "{error}");
    assert!(fs::read_to_string(&exe).unwrap().contains("update-self-check"));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn loopback_http_apply_uses_no_external_host() {
    let root = temp_dir();
    let install = root.join("install");
    fs::create_dir_all(&install).unwrap();
    let exe_bytes = b"#!/bin/sh\nif [ \"$1\" = \"--update-self-check\" ]; then echo update-self-check-ok; exit 0; fi\nexit 0\n";
    let manifest = manifest_for("4.0.0", vec![file_entry("korangar.exe", exe_bytes, "korangar.exe")]);
    let files = std::collections::HashMap::from([
        ("/manifest.krup".to_string(), manifest.into_bytes()),
        ("/korangar.exe".to_string(), exe_bytes.to_vec()),
    ]);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        for _ in 0..2 {
            let (mut socket, _) = listener.accept().unwrap();
            socket.set_read_timeout(Some(std::time::Duration::from_secs(2))).unwrap();
            let mut buffer = [0u8; 1024];
            let size = socket.read(&mut buffer).unwrap_or(0);
            let request = String::from_utf8_lossy(&buffer[..size]);
            let path = request.split_whitespace().nth(1).unwrap_or("/");
            let body = files.get(path).cloned().unwrap_or_default();
            let header = format!("HTTP/1.0 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let _ = socket.write_all(header.as_bytes());
            let _ = socket.write_all(&body);
        }
    });
    let exe = install.join("korangar.exe");
    write_script(&exe, true);
    let outcome = korangar_update::run_startup_update(&request(
        &install,
        &exe,
        &format!("http://127.0.0.1:{port}/manifest.krup"),
        "0.1.1",
    ))
    .unwrap();
    assert_eq!(outcome, StartupOutcome::Applied { relaunched: false });
    assert_eq!(fs::read(&exe).unwrap(), exe_bytes);
    let remote = korangar_update::run_startup_update(&request(&install, &exe, "http://example.com/manifest.krup", "0.1.1")).unwrap_err();
    assert!(remote.contains("disabled") || remote.contains("only http"), "{remote}");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn signing_cli_roundtrip_and_keygen_stays_outside_the_repo() {
    let root = temp_dir();
    let source = root.join("korangar.exe");
    fs::write(&source, b"bytes").unwrap();
    let seed_path = root.join("seed");
    fs::write(&seed_path, SEED).unwrap();
    let out = root.join("manifest.krup");
    korangar_update::cli::dispatch([
        "sign".into(),
        "--version".into(),
        "1.2.3".into(),
        "--out".into(),
        out.display().to_string(),
        "--private-key".into(),
        seed_path.display().to_string(),
        "--file".into(),
        format!("korangar.exe={}", source.display()),
    ])
    .unwrap();
    let signed = fs::read(&out).unwrap();
    let opened = manifest::open_envelope(&signed, public_key()).unwrap();
    assert_eq!(opened.version, "1.2.3");
    assert_eq!(opened.files[0].size, 5);

    let refused = korangar_update::cli::dispatch(["keygen".into(), "--out".into(), "secret.seed".into()]);
    let message = refused.expect_err("keygen inside the worktree must fail");
    assert!(message.contains("worktree"), "{message}");
    assert!(!Path::new("secret.seed").exists());
    let outside = root.join("outside.seed");
    korangar_update::cli::dispatch(["keygen".into(), "--out".into(), outside.display().to_string()]).unwrap();
    assert_eq!(fs::metadata(&outside).unwrap().len(), 32);
    let _ = fs::remove_dir_all(&root);
}

fn request<'a>(install: &'a Path, exe: &'a Path, url: &'a str, version: &'a str) -> StartupRequest<'a> {
    StartupRequest {
        skip_env: None,
        manifest_url: Some(url),
        current_version: version,
        install_root: install,
        current_exe: exe,
        public_key: public_key(),
        relaunch_args: &[],
        spawn_relaunch: false,
    }
}
