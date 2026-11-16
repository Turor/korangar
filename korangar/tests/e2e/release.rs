//! Release parity. Offline cases sign throwaway manifests and drive the startup
//! update hook. The live case logs into Hercules TEST only when
//! `KORANGAR_E2E=1`.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::{env, fs};

use korangar::e2e::{ClientStartup, LogicSession, Outbound, Phase, drive_startup_update};
use korangar_update::manifest::{self, ReleaseFile, ReleaseManifest, UpdatePlatform};
use korangar_update::{StartupOutcome, generate_seed};

fn shell_script() -> &'static [u8] {
    b"#!/bin/sh\nif [ \"$1\" = \"--update-self-check\" ]; then echo update-self-check-ok; exit 0; fi\necho release-e2e\n"
}

fn write_script(path: &Path) {
    fs::write(path, shell_script()).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }
}

fn temp_dir() -> PathBuf {
    let path = env::temp_dir().join(format!("korangar-e2e-release-{}-{}", std::process::id(), unique()));
    fs::create_dir_all(&path).unwrap();
    path
}

fn unique() -> u64 {
    use std::sync::atomic::AtomicU64;
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

struct EnvSet {
    key: &'static str,
    previous: Option<String>,
}

impl EnvSet {
    fn set(key: &'static str, value: &str) -> Self {
        let previous = env::var(key).ok();
        // SAFETY: e2e runs with --test-threads=1. The previous value is restored on
        // drop.
        unsafe { env::set_var(key, value) };
        Self { key, previous }
    }

    fn replace(&self, value: &str) {
        // SAFETY: same single-threaded e2e process. Drop still restores the original
        // value.
        unsafe { env::set_var(self.key, value) };
    }
}

impl Drop for EnvSet {
    fn drop(&mut self) {
        unsafe {
            match &self.previous {
                Some(value) => env::set_var(self.key, value),
                None => env::remove_var(self.key),
            }
        }
    }
}

struct HitServer {
    base: String,
    hits: Arc<Mutex<Vec<String>>>,
    done: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl HitServer {
    fn start(files: HashMap<String, Vec<u8>>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let done = Arc::new(AtomicBool::new(false));
        let hits_thread = hits.clone();
        let done_thread = done.clone();
        let join = thread::spawn(move || {
            while !done_thread.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut socket, _)) => {
                        let _ = socket.set_read_timeout(Some(std::time::Duration::from_secs(2)));
                        let mut buffer = [0u8; 2048];
                        let size = socket.read(&mut buffer).unwrap_or(0);
                        let request = String::from_utf8_lossy(&buffer[..size]);
                        let path = request.split_whitespace().nth(1).unwrap_or("/").to_string();
                        hits_thread.lock().unwrap().push(path.clone());
                        let body = files.get(&path).cloned().unwrap_or_default();
                        let status = if files.contains_key(&path) { "200 OK" } else { "404 Not Found" };
                        let header = format!(
                            "HTTP/1.0 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        let _ = socket.write_all(header.as_bytes());
                        let _ = socket.write_all(&body);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => thread::sleep(std::time::Duration::from_millis(5)),
                    Err(_) => break,
                }
            }
        });
        Self {
            base: format!("http://127.0.0.1:{port}"),
            hits,
            done,
            join: Some(join),
        }
    }

    fn paths(&self) -> Vec<String> {
        self.hits.lock().unwrap().clone()
    }
}

impl Drop for HitServer {
    fn drop(&mut self) {
        self.done.store(true, Ordering::Relaxed);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

struct Fixture {
    seed: [u8; 32],
    public_key: [u8; 32],
}

impl Fixture {
    fn new() -> Self {
        let seed = generate_seed().unwrap();
        let public_key = manifest::public_key_bytes(&seed).unwrap();
        Self { seed, public_key }
    }

    fn sign(&self, version: &str, build: u64, files: Vec<ReleaseFile>) -> String {
        let payload = manifest::canonical_payload(&ReleaseManifest {
            version: version.into(),
            build,
            files,
        })
        .unwrap();
        manifest::sign_envelope(&payload, &self.seed).unwrap()
    }
}

fn file_entry(path: &str, bytes: &[u8]) -> ReleaseFile {
    ReleaseFile {
        path: path.into(),
        sha256: manifest::sha256_hex(bytes),
        size: bytes.len() as u64,
        url: path.into(),
    }
}

fn drive(install: &Path, exe: &Path, key: &[u8; 32], platform: UpdatePlatform) -> Result<StartupOutcome, String> {
    drive_startup_update(&ClientStartup {
        install_root: install,
        current_exe: exe,
        public_key: key,
        platform,
        current_version: "0.1.1",
        relaunch_args: &[],
        spawn_relaunch: false,
    })
}

fn closed_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}

#[test]
fn offline_release_parity_startup_hook() {
    let own = UpdatePlatform::current();
    let sha = "703c76c37929e49b363f1e2f712cb80212a0d3d2";
    let linux_name = format!("korangar-linux-{sha}");
    let windows_name = format!("korangar-windows-{sha}.exe");
    let (own_name, other_name) = match own {
        UpdatePlatform::Linux => (linux_name.clone(), windows_name.clone()),
        UpdatePlatform::Windows => (windows_name.clone(), linux_name.clone()),
    };
    let bytes = shell_script();
    let fixture = Fixture::new();
    let empty = manifest::canonical_payload(&ReleaseManifest {
        version: "empty".into(),
        build: 1,
        files: Vec::new(),
    });
    assert!(empty.unwrap_err().contains("no files"));

    let mut served = HashMap::new();
    served.insert(
        "/own.krup".into(),
        fixture.sign("main-own", 10, vec![file_entry(&own_name, bytes)]).into_bytes(),
    );
    served.insert(
        "/other.krup".into(),
        fixture.sign("main-other", 11, vec![file_entry(&other_name, bytes)]).into_bytes(),
    );
    served.insert(
        "/both.krup".into(),
        fixture
            .sign("main-both", 12, vec![
                file_entry(&linux_name, bytes),
                file_entry(&windows_name, bytes),
            ])
            .into_bytes(),
    );
    served.insert(format!("/{own_name}"), bytes.to_vec());
    served.insert(format!("/{other_name}"), bytes.to_vec());
    let server = HitServer::start(served);

    let root = temp_dir();
    let install = root.join("install");
    fs::create_dir_all(&install).unwrap();
    let exe = install.join(if own == UpdatePlatform::Windows {
        "korangar.exe"
    } else {
        "korangar"
    });
    write_script(&exe);

    let manifest_env = EnvSet::set("KORANGAR_UPDATE_MANIFEST_URL", &format!("{}/own.krup", server.base));
    let outcome = drive(&install, &exe, &fixture.public_key, own).unwrap();
    assert_eq!(outcome, StartupOutcome::Applied { relaunched: false });
    assert_eq!(fs::read(&exe).unwrap(), bytes);
    assert_eq!(fs::read_to_string(install.join("updates/applied.build")).unwrap().trim(), "10");
    assert!(
        server.paths().iter().any(|path| path == &format!("/{own_name}")),
        "own platform binary was not fetched: {:?}",
        server.paths()
    );
    assert!(
        !server.paths().iter().any(|path| path == &format!("/{other_name}")),
        "other platform was downloaded: {:?}",
        server.paths()
    );

    let applied = fs::read_to_string(install.join("updates/applied.build")).unwrap();
    let exe_bytes = fs::read(&exe).unwrap();
    manifest_env.replace(&format!("{}/other.krup", server.base));
    let skipped = drive(&install, &exe, &fixture.public_key, own).unwrap();
    assert_eq!(skipped, StartupOutcome::Skipped);
    assert_eq!(fs::read_to_string(install.join("updates/applied.build")).unwrap(), applied);
    assert_eq!(fs::read(&exe).unwrap(), exe_bytes);
    assert!(
        !server.paths().iter().any(|path| path == &format!("/{other_name}")),
        "other-platform-only release was downloaded: {:?}",
        server.paths()
    );
    let skipped_again = drive(&install, &exe, &fixture.public_key, own).unwrap();
    assert_eq!(skipped_again, StartupOutcome::Skipped);
    assert_eq!(fs::read_to_string(install.join("updates/applied.build")).unwrap(), applied);

    manifest_env.replace(&format!("{}/both.krup", server.base));
    let both = drive(&install, &exe, &fixture.public_key, own).unwrap();
    assert_eq!(both, StartupOutcome::Applied { relaunched: false });
    assert_eq!(fs::read_to_string(install.join("updates/applied.build")).unwrap().trim(), "12");
    assert!(
        !server.paths().iter().any(|path| path == &format!("/{other_name}")),
        "both-platform release downloaded the other binary: {:?}",
        server.paths()
    );
    let once = drive(&install, &exe, &fixture.public_key, own).unwrap();
    assert_eq!(once, StartupOutcome::UpToDate);

    let empty_path = root.join("empty.krup");
    fs::write(&empty_path, b"KRUP1\n1.0.0\n1\n0\n").unwrap();
    let build_before = fs::read_to_string(install.join("updates/applied.build")).unwrap();
    manifest_env.replace(&format!("file://{}", empty_path.display()));
    let refused = drive(&install, &exe, &fixture.public_key, own).unwrap_err();
    assert!(
        refused.contains("KRUP1SIG") || refused.contains("no files") || refused.contains("signature") || refused.contains("magic"),
        "{refused}"
    );
    assert_eq!(fs::read_to_string(install.join("updates/applied.build")).unwrap(), build_before);

    let port = closed_port();
    manifest_env.replace(&format!("http://127.0.0.1:{port}/manifest.jsonup"));
    let unreachable = drive(&install, &exe, &fixture.public_key, own).unwrap();
    assert_eq!(unreachable, StartupOutcome::Skipped);
    assert_eq!(fs::read_to_string(install.join("updates/applied.build")).unwrap(), build_before);

    let mut session = LogicSession::offline();
    session.apply_line("chat release-parity").unwrap();
    assert_eq!(session.phase(), Phase::Idle);
    assert!(session.outbound().iter().any(|item| matches!(
        item,
        Outbound::SendMessage { text } if text == "release-parity"
    )));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn unreachable_update_channel_does_not_block_character_select() {
    let port = closed_port();
    let _manifest = EnvSet::set(
        "KORANGAR_UPDATE_MANIFEST_URL",
        &format!("http://127.0.0.1:{port}/manifest.jsonup"),
    );
    let root = temp_dir();
    let install = root.join("install");
    fs::create_dir_all(&install).unwrap();
    let exe = install.join("korangar");
    write_script(&exe);
    let outcome = drive(&install, &exe, &korangar_update::UPDATE_PUBLIC_KEY, UpdatePlatform::current()).unwrap();
    assert_eq!(outcome, StartupOutcome::Skipped);
    assert!(!install.join("updates/applied.build").exists());

    let Some(mut session) = super::live_session() else {
        let _ = fs::remove_dir_all(&root);
        return;
    };
    session.login_to_character_select().unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(session.phase(), Phase::CharacterSelect);
    assert!(session.outbound().iter().any(|item| matches!(item, Outbound::Login)));
    let _ = fs::remove_dir_all(&root);
}
