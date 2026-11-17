//! Skill window description text over the remote asset host (t273u).
//!
//! The offline host mimics remoteclient2: real `System/` files match only by
//! their exact name, `data/` paths match any case, and there is no snapshot
//! manifest. The live case reads the real TEST asset host and skips unless
//! `KORANGAR_E2E=1`.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use korangar_loaders::remote::{PersistentAssets, RemoteConfig};

use super::LiveConnect;
use crate::loaders::{GameFileLoader, GameFileOrigin};
use crate::world::{compose_skill_description, load_skill_text, skill_hover_text};

/// `SM_BASH`.
pub const BASH_SKILL_ID: u16 = 5;
const TEST_ASSETS: &str = "https://ro.turoran.com/client/";

const SKILL_ID_LUB: &str = "SKID = {\n\tNV_BASIC = 1,\n\tSM_BASH = 5,\n}\n";
const RENEWAL_LUB: &str = "SKILL_DESCRIPT = {\n\t[SKID.SM_BASH] = {\n\t\t\"Bash\",\n\t\t\"Max Level: 10\",\n\t\t\"Description: \
                           ^777777Hits one enemy with a heavy blow.^000000\",\n\t},\n}\n";
const OFFICIAL_LUB: &str =
    "SKILL_DESCRIPT = {\n\t[SKID.SM_BASH] = {\n\t\t\"Bash\",\n\t\t\"Description: Official skillinfoz text.\",\n\t},\n}\n";

pub struct SkillDescriptionReport {
    /// Hover tooltip of an unlearned Bash slot, as the skill window builds it.
    pub bash_tooltip: String,
    /// Raw Bash text from the loaded table.
    pub bash_body: String,
    pub description_count: usize,
    /// Request paths the host saw, in order.
    pub requests: Vec<String>,
    /// `System/skilldescript.turoran.re2.lub` was fetched and cached.
    pub turoran_file_cached: bool,
}

/// Which description files the offline host has.
#[derive(Clone, Copy, Debug)]
pub enum DescriptionHost {
    /// Turoran `System/skilldescript.turoran.re2.lub`, like ro.turoran.com.
    TuroranRenewal,
    /// Only the official `skillinfoz/skilldescript.lub`.
    OfficialOnly,
}

struct CaseHost {
    base: String,
    requests: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for CaseHost {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn reply_for(path: &str, host: DescriptionHost) -> Option<&'static str> {
    // `System/` is exact-case, like remoteclient2 serving its real System dir.
    let system_renewal = "/client/System/skilldescript.turoran.re2.lub";
    if matches!(host, DescriptionHost::TuroranRenewal) && path == system_renewal {
        return Some(RENEWAL_LUB);
    }
    let lower = path.to_lowercase();
    if lower == "/client/data/luafiles514/lua%20files/skillinfoz/skillid.turoran4.lub" {
        return Some(SKILL_ID_LUB);
    }
    if matches!(host, DescriptionHost::OfficialOnly) && lower == "/client/data/luafiles514/lua%20files/skillinfoz/skilldescript.lub" {
        return Some(OFFICIAL_LUB);
    }
    None
}

fn start_host(host: DescriptionHost) -> Result<CaseHost, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|error| error.to_string())?;
    listener.set_nonblocking(true).map_err(|error| error.to_string())?;
    let base = format!("http://{}/client/", listener.local_addr().map_err(|error| error.to_string())?);
    let requests = Arc::new(Mutex::new(Vec::new()));
    let stop = Arc::new(AtomicBool::new(false));
    let (seen, stopping) = (requests.clone(), stop.clone());
    let thread = std::thread::spawn(move || {
        while !stopping.load(Ordering::SeqCst) {
            let Ok((stream, _)) = listener.accept() else {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            };
            let _ = stream.set_nonblocking(false);
            let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
            let mut reader = BufReader::new(stream);
            let mut request_line = String::new();
            if reader.read_line(&mut request_line).is_err() {
                continue;
            }
            loop {
                let mut header = String::new();
                if reader.read_line(&mut header).map_or(true, |read| read == 0) || header == "\r\n" {
                    break;
                }
            }
            let path = request_line.split_whitespace().nth(1).unwrap_or("").to_owned();
            seen.lock().unwrap().push(path.clone());
            let mut stream = reader.into_inner();
            let _ = match reply_for(&path, host) {
                Some(body) => write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                ),
                None => write!(
                    stream,
                    "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                ),
            };
        }
    });
    Ok(CaseHost {
        base,
        requests,
        stop,
        thread: Some(thread),
    })
}

fn temp_cache(tag: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("korangar-e2e-skilldesc-{tag}-{}-{stamp}", std::process::id()))
}

fn report_from(loader: &GameFileLoader, requests: Vec<String>) -> SkillDescriptionReport {
    let (descriptions, _) = load_skill_text(loader);
    let bash_body = descriptions.get(&BASH_SKILL_ID).cloned().unwrap_or_default();
    // An unlearned slot: level 0 of 10, Bash SP 8.
    let composed = compose_skill_description(&bash_body, 0, 10, &[8, 8, 8, 8, 8, 15, 15, 15, 15, 15]);
    SkillDescriptionReport {
        bash_tooltip: skill_hover_text("Bash", &composed),
        bash_body,
        description_count: descriptions.len(),
        requests,
        turoran_file_cached: matches!(
            loader.origin_of(r"System\skilldescript.turoran.re2.lub"),
            Some(GameFileOrigin::RemoteCache)
        ),
    }
}

fn load_through(base: &str, tag: &str, allow_http: bool) -> Result<(GameFileLoader, PathBuf), String> {
    let cache = temp_cache(tag);
    let assets = PersistentAssets::open(RemoteConfig {
        base_url: base.to_owned(),
        cache_dir: cache.clone(),
        allow_http,
        offline: false,
    })
    .map_err(|error| format!("remote assets: {error}"))?;
    let loader = GameFileLoader::default();
    loader.attach_remote_assets(assets);
    Ok((loader, cache))
}

/// Load skill text through a local remoteclient2-shaped host. No GRF and no
/// network host outside 127.0.0.1.
pub fn offline_skill_description(host: DescriptionHost) -> Result<SkillDescriptionReport, String> {
    let server = start_host(host)?;
    let (loader, cache) = load_through(&server.base, "offline", true)?;
    let requests = server.requests.clone();
    let report = report_from(&loader, Vec::new());
    drop(server);
    let _ = std::fs::remove_dir_all(&cache);
    Ok(SkillDescriptionReport {
        requests: requests.lock().unwrap().clone(),
        ..report
    })
}

/// Same load against the real TEST asset host. Reads only public client files.
pub fn live_skill_description() -> LiveConnect<SkillDescriptionReport> {
    if std::env::var("KORANGAR_E2E").ok().as_deref() != Some("1") {
        return LiveConnect::Skip("set KORANGAR_E2E=1 to read the TEST asset host".to_owned());
    }
    let (loader, cache) = match load_through(TEST_ASSETS, "live", false) {
        Ok(loaded) => loaded,
        Err(error) => return LiveConnect::Skip(error),
    };
    let report = report_from(&loader, Vec::new());
    let _ = std::fs::remove_dir_all(&cache);
    LiveConnect::Ready(report)
}
