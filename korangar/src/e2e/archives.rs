//! Local GRF and folder overlay probe for the archive e2e scenarios.
//!
//! The fixture is a temp install: an overlay folder, a GRF built with the
//! in-tree builder, and `client/game_archives.ron`. Nothing here reads the
//! real client data or a network asset host.

use std::fs;
use std::path::{Path, PathBuf};

use korangar_loaders::FileLoader;

use super::{LiveConnect, LogicSession};
use crate::loaders::archive::native::builder::NativeArchiveBuilder;
use crate::loaders::archive::{Compression, Writable};
use crate::loaders::{GameFileLoader, GameFileOrigin};
use crate::world::load_map_display_names;

const SHARED: &str = r"data\e2e\shared.txt";
const LUB: &str = r"data\luafiles514\lua files\Enchant\EnchantList.lub";
const ONLY_GRF: &str = r"data\e2e\only-grf.txt";
const ONLY_KORANGAR: &str = r"data\e2e\only-korangar.txt";
const MAP_TABLE: &str = r"data\mapnametable.txt";
const OVERLAY_MAP: &str = "prontera.rsw#E2E Overlay Prontera#\n";

pub struct OfflineArchiveReport {
    pub shared_bytes: Vec<u8>,
    pub shared_origin: String,
    pub lub_bytes: Vec<u8>,
    pub lub_origin: String,
    pub grf_only_bytes: Vec<u8>,
    pub grf_only_origin: String,
    pub korangar_bytes: Vec<u8>,
    pub korangar_origin: String,
    pub warnings: Vec<String>,
    pub search_order: Vec<String>,
    pub anchor_line: String,
    pub install_dir: PathBuf,
    pub display_name: String,
    root: PathBuf,
}

impl Drop for OfflineArchiveReport {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub struct LiveOverlaySession {
    pub session: LogicSession,
    pub display_name: String,
    pub asset_bytes: Vec<u8>,
    pub origin: String,
    root: PathBuf,
}

impl Drop for LiveOverlaySession {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

struct LoadedFixture {
    root: PathBuf,
    shared_bytes: Vec<u8>,
    shared_origin: String,
    lub_bytes: Vec<u8>,
    lub_origin: String,
    grf_only_bytes: Vec<u8>,
    grf_only_origin: String,
    korangar_bytes: Vec<u8>,
    korangar_origin: String,
    warnings: Vec<String>,
    search_order: Vec<String>,
    anchor_line: String,
    display_name: String,
    asset_bytes: Vec<u8>,
    map_origin: String,
}

pub fn probe_offline_archives() -> Result<OfflineArchiveReport, String> {
    let loaded = load_fixture("offline")?;
    Ok(OfflineArchiveReport {
        shared_bytes: loaded.shared_bytes,
        shared_origin: loaded.shared_origin,
        lub_bytes: loaded.lub_bytes,
        lub_origin: loaded.lub_origin,
        grf_only_bytes: loaded.grf_only_bytes,
        grf_only_origin: loaded.grf_only_origin,
        korangar_bytes: loaded.korangar_bytes,
        korangar_origin: loaded.korangar_origin,
        warnings: loaded.warnings,
        search_order: loaded.search_order,
        anchor_line: loaded.anchor_line,
        install_dir: loaded.root.clone(),
        display_name: loaded.display_name,
        root: loaded.root,
    })
}

pub fn start_live_with_overlay() -> LiveConnect<LiveOverlaySession> {
    let loaded = match load_fixture("live") {
        Ok(loaded) => loaded,
        Err(error) => return LiveConnect::Invalid(error),
    };
    match LogicSession::start_live() {
        LiveConnect::Skip(reason) => {
            let _ = fs::remove_dir_all(&loaded.root);
            LiveConnect::Skip(reason)
        }
        LiveConnect::Invalid(error) => {
            let _ = fs::remove_dir_all(&loaded.root);
            LiveConnect::Invalid(error)
        }
        LiveConnect::Ready(session) => LiveConnect::Ready(LiveOverlaySession {
            session,
            display_name: loaded.display_name,
            asset_bytes: loaded.asset_bytes,
            origin: loaded.map_origin,
            root: loaded.root,
        }),
    }
}

fn load_fixture(label: &str) -> Result<LoadedFixture, String> {
    let root = temp_root(label);
    if let Err(error) = build_install(&root) {
        let _ = fs::remove_dir_all(&root);
        return Err(error);
    }
    let loader = GameFileLoader::default();
    if let Err(error) = loader.load_from_install(&root) {
        let _ = fs::remove_dir_all(&root);
        return Err(error);
    }
    let names = load_map_display_names(&loader);
    let display_name = names.get("prontera").cloned().unwrap_or_default();
    let asset_bytes = loader.get(MAP_TABLE).unwrap_or_default();
    let loaded = LoadedFixture {
        root: root.clone(),
        shared_bytes: read_asset(&loader, SHARED)?,
        shared_origin: origin_label(loader.origin_of(SHARED)),
        lub_bytes: read_asset(&loader, LUB)?,
        lub_origin: origin_label(loader.origin_of(LUB)),
        grf_only_bytes: read_asset(&loader, ONLY_GRF)?,
        grf_only_origin: origin_label(loader.origin_of(ONLY_GRF)),
        korangar_bytes: read_asset(&loader, ONLY_KORANGAR)?,
        korangar_origin: origin_label(loader.origin_of(ONLY_KORANGAR)),
        warnings: loader.startup_warnings(),
        search_order: loader.search_order(),
        anchor_line: loader.anchor_line(),
        display_name,
        asset_bytes,
        map_origin: origin_label(loader.origin_of(MAP_TABLE)),
    };
    drop(loader);
    Ok(loaded)
}

fn build_install(root: &Path) -> Result<(), String> {
    fs::create_dir_all(root.join("client")).map_err(|error| error.to_string())?;
    write_virtual(&root.join("overlay"), SHARED, b"from-overlay")?;
    write_virtual(&root.join("overlay"), LUB, b"lub-from-overlay")?;
    write_virtual(root.join("overlay").as_path(), MAP_TABLE, OVERLAY_MAP.as_bytes())?;
    write_virtual(&root.join("archive"), SHARED, b"from-korangar")?;
    write_virtual(&root.join("archive"), ONLY_GRF, b"from-korangar")?;
    write_virtual(&root.join("archive"), ONLY_KORANGAR, b"from-korangar")?;

    let grf_path = root.join("base.grf");
    let mut builder = NativeArchiveBuilder::from_path(&grf_path);
    builder.add_file(SHARED, b"from-grf".to_vec(), Compression::Default);
    builder.add_file(LUB, b"lub-from-grf".to_vec(), Compression::Default);
    builder.add_file(ONLY_GRF, b"only-grf".to_vec(), Compression::Default);
    builder.add_file(MAP_TABLE, b"prontera.rsw#GRF Prontera#\n".to_vec(), Compression::Default);
    builder.finish().map_err(|error| error.to_string())?;

    let config = r#"(
    overlays: ["overlay/"],
    archives: [
        "base.grf",
        (path: "missing-required.grf", optional: false),
        (path: "missing-optional.grf", optional: true),
    ],
)
"#;
    fs::write(root.join("client/game_archives.ron"), config).map_err(|error| error.to_string())
}

fn write_virtual(root: &Path, virtual_path: &str, bytes: &[u8]) -> Result<(), String> {
    let path = root.join(virtual_path.replace('\\', "/"));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(path, bytes).map_err(|error| error.to_string())
}

fn read_asset(loader: &GameFileLoader, path: &str) -> Result<Vec<u8>, String> {
    loader.get(path).map_err(|error| format!("{path}: {error:?}"))
}

fn origin_label(origin: Option<GameFileOrigin>) -> String {
    match origin {
        Some(GameFileOrigin::Overlay { path }) => format!("overlay:{path}"),
        Some(GameFileOrigin::ConfiguredArchive { path }) => format!("archive:{path}"),
        Some(GameFileOrigin::KorangarArchive { path }) => format!("korangar:{path}"),
        Some(GameFileOrigin::RemoteCache) => "remote-cache".to_owned(),
        Some(GameFileOrigin::RemoteFetch) => "remote-fetch".to_owned(),
        None => "missing".to_owned(),
    }
}

fn temp_root(label: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("korangar-g49-{label}-{}-{id}", std::process::id()))
}
