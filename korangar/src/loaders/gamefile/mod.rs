//! Manages archives where game assets are stored and provides convenient
//! methods to retrieve each of them individually. The archives implement the
//! [`Archive`] trait.

mod cache;
mod list;
mod resname;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock, RwLock};

use blake3::Hash;
#[cfg(feature = "debug")]
use korangar_debug::logging::{Colorize, Timer, print_debug};
use korangar_loaders::remote::PersistentAssets;
use korangar_loaders::{FileLoader, FileNotFoundError};

pub use self::cache::{sync_cache_archive, texture_file_dds_name, video_file_ivf_name};
use self::list::{
    GameArchiveList, PathAnchor, ResolvedArchiveConfig, ResolvedEntry, anchor_log_line, korangar_archive_dir, path_anchor,
    same_archive_path,
};
use super::archive::folder::FolderArchive;
use super::archive::native::NativeArchive;
use super::archive::{Archive, ArchiveType, normalize_archive_path};
use crate::loaders::archive::seven_zip::SevenZipArchive;

pub(crate) const CACHE_FILE_NAME: &str = "cache.7z";

pub(crate) const TEMPORARY_CACHE_FILE_NAME: &str = "cache.7z.tmp";
pub(crate) const HASH_FILE_PATH: &str = "game_file_hash.txt";

/// This string is used to derive an initialization vector for the game file
/// hash calculation. We can use this to trigger a de-sync of the cache files of
/// users.
const GAME_FILE_DERIVE_KEY: &str = "korangar 2025-03-09 14:17:23 game file key v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArchiveRole {
    Overlay,
    Configured,
    Korangar,
    /// Derived `cache.7z`. Not part of the operator search order or the hash.
    AssetCache,
}

impl ArchiveRole {
    fn tag(self) -> &'static str {
        match self {
            Self::Overlay => "overlay",
            Self::Configured => "archive",
            Self::Korangar => "korangar",
            Self::AssetCache => "cache",
        }
    }
}

struct LoaderArchive {
    archive: Box<dyn Archive>,
    is_game_archive: bool,
    role: ArchiveRole,
    /// Resolved filesystem path. Empty for archives added without a path.
    location: String,
}

/// Which configured source would serve a virtual path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GameFileOrigin {
    /// Operator overlay. `path` is the resolved folder or GRF.
    Overlay { path: String },
    /// Entry from `archives` in `client/game_archives.ron` or DATA.INI.
    ConfiguredArchive { path: String },
    /// Korangar `archive/` directory, below configured archives.
    KorangarArchive { path: String },
    /// Bytes already stored in the verified remote cache.
    RemoteCache,
    /// Listed by the remote manifest or loose index, but not stored yet.
    RemoteFetch,
}

/// Type implementing the game file loader.
///
/// Currently, there are two types implementing
/// [`Archive`]:
/// - [`NativeArchive`] - Retrieve assets from GRF files.
/// - [`FolderArchive`] - Retrieve assets from an OS folder.
/// - [`SevenZipArchive`] - Retrieve assets from ZIP files.
#[derive(Default)]
pub struct GameFileLoader {
    archives: RwLock<Vec<LoaderArchive>>,
    remote: RwLock<Option<Arc<PersistentAssets>>>,
    /// Map aliases from `data\resnametable.txt`, read on the first map file.
    map_aliases: OnceLock<HashMap<String, String>>,
    warnings: RwLock<Vec<String>>,
    search_order: RwLock<Vec<String>>,
    anchor_line: RwLock<String>,
}

fn note_missing_game_file(path: &str) {
    static SEEN: AtomicUsize = AtomicUsize::new(0);
    let index = SEEN.fetch_add(1, Ordering::Relaxed);
    if index < 20 {
        eprintln!("missing game file ({}/20): {path}", index + 1);
    }
}

impl FileLoader for GameFileLoader {
    fn get(&self, path: &str) -> Result<Vec<u8>, FileNotFoundError> {
        let archive_path = normalize_archive_path(path);
        // Like the official client, an aliased map loads its target's files.
        if let Some(target) = self.map_alias(&archive_path)
            && let Ok(bytes) = self.get_exact(&target, &target)
        {
            return Ok(bytes);
        }
        self.get_exact(path, &archive_path)
    }
}

impl GameFileLoader {
    fn get_exact(&self, path: &str, archive_path: &str) -> Result<Vec<u8>, FileNotFoundError> {
        let archive_path = archive_path.to_owned();
        if let Some(bytes) = self
            .archives
            .read()
            .unwrap()
            .iter()
            .find_map(|archive| archive.archive.get_file_by_path(&archive_path))
        {
            return Ok(bytes);
        }
        // Local archives win. A pinned manifest is next; paths missing from that
        // snapshot fall through to a loose fetch under the same base URL.
        match self.remote.read().unwrap().as_ref() {
            // The caller's case goes to the host: remoteclient2 matches real
            // `System/` files by exact name.
            Some(remote) => remote.get(path).map_err(|error| {
                note_missing_game_file(path);
                FileNotFoundError::new(format!("{path}: verified remote retrieval failed: {error}"))
            }),
            None => {
                note_missing_game_file(path);
                Err(FileNotFoundError::new(path.to_owned()))
            }
        }
    }

    /// The resnametable target of a map file, if any. The table is read once;
    /// a failed read (no network yet) is retried on the next map file.
    fn map_alias(&self, archive_path: &str) -> Option<String> {
        let name = archive_path.strip_prefix("data\\")?;
        if !(name.ends_with(".rsw") || name.ends_with(".gnd") || name.ends_with(".gat")) {
            return None;
        }
        let aliases = match self.map_aliases.get() {
            Some(aliases) => aliases,
            None => {
                let bytes = self.get_exact(resname::RESNAMETABLE_PATH, resname::RESNAMETABLE_PATH).ok()?;
                let aliases = resname::parse_map_aliases(&resname::decode_table(&bytes));
                self.map_aliases.get_or_init(|| aliases)
            }
        };
        resname::aliased_path(aliases, archive_path)
    }

    /// True only when a mounted GRF, folder, or cache archive contains `path`.
    /// A remote manifest entry is not a local hit.
    pub fn local_file_exists(&self, path: &str) -> bool {
        let archive_path = normalize_archive_path(path);
        self.archives
            .read()
            .unwrap()
            .iter()
            .any(|archive| archive.archive.file_exists(&archive_path))
    }

    pub fn file_exists(&self, path: &str) -> bool {
        let archive_path = normalize_archive_path(path);
        if let Some(target) = self.map_alias(&archive_path)
            && self.file_exists_exact(&target)
        {
            return true;
        }
        self.file_exists_exact(&archive_path)
    }

    fn file_exists_exact(&self, archive_path: &str) -> bool {
        self.local_file_exists(archive_path) || self.remote.read().unwrap().as_ref().is_some_and(|r| r.contains(archive_path))
    }

    fn add_archive(&self, archive: Box<dyn Archive>, is_game_archive: bool) {
        let role = if is_game_archive {
            ArchiveRole::Configured
        } else {
            ArchiveRole::AssetCache
        };
        self.add_mounted(archive, is_game_archive, role, String::new());
    }

    fn add_mounted(&self, archive: Box<dyn Archive>, is_game_archive: bool, role: ArchiveRole, location: String) {
        self.archives.write().unwrap().insert(0, LoaderArchive {
            archive,
            is_game_archive,
            role,
            location,
        });
    }

    fn get_archive_type_by_path(path: &Path) -> Option<ArchiveType> {
        if path.is_dir() {
            Some(ArchiveType::Folder)
        } else if let Some(extension) = path.extension().and_then(|extension| extension.to_str()) {
            match extension {
                "grf" => Some(ArchiveType::Native),
                "7z" => Some(ArchiveType::SevenZip),
                _ => None,
            }
        } else {
            None
        }
    }

    fn load_archive_from_path(path: &Path) -> Result<Box<dyn Archive>, String> {
        match GameFileLoader::get_archive_type_by_path(path) {
            Some(ArchiveType::Folder) => Ok(Box::new(FolderArchive::from_path(path))),
            Some(ArchiveType::Native) => Ok(Box::new(NativeArchive::from_path(path))),
            Some(ArchiveType::SevenZip) => Ok(Box::new(SevenZipArchive::from_path(path))),
            None => Err(format!(
                "warning: game archive skipped (not a directory, .grf, or .7z): {}",
                path.display()
            )),
        }
    }

    pub fn load_archives_from_settings(&self) -> Result<(), String> {
        #[cfg(feature = "debug")]
        let timer = Timer::new("load game archives");

        if let Some(config) = crate::deployment::remote_config() {
            let remote = PersistentAssets::open(config).map_err(|error| {
                format!(
                    "Cannot initialize verified remote assets: {error}. Stage an approved manifest/cache first; see \
                     docs/turoran-native-test.md. No local GRFs are required in remote mode."
                )
            })?;
            *self.remote.write().unwrap() = Some(Arc::new(remote));
            if crate::deployment::remote_prefetch_enabled() {
                let line = self.prefetch_remote_status();
                eprintln!("{line}");
            }
        }
        let remote_enabled = self.remote.read().unwrap().is_some();
        let executable = std::env::current_exe().ok();
        let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let portable = crate::deployment::portable_runtime_dir();
        let (base, anchor) = path_anchor(executable.as_deref(), &working_dir, portable.as_deref());
        self.mount_at(&base, anchor, remote_enabled)?;

        #[cfg(feature = "debug")]
        timer.stop();
        Ok(())
    }

    /// Attach a remote asset host without reading deployment environment
    /// variables. e2e scenarios use this with a local test host.
    pub(crate) fn attach_remote_assets(&self, assets: PersistentAssets) {
        *self.remote.write().unwrap() = Some(Arc::new(assets));
    }

    /// Load `client/game_archives.ron` from `install_dir` as the install root.
    ///
    /// Tests use this so they do not depend on the process working directory.
    /// Remote assets already attached to this loader stay attached.
    pub fn load_from_install(&self, install_dir: &Path) -> Result<(), String> {
        let remote_enabled = self.remote.read().unwrap().is_some();
        self.mount_at(install_dir, PathAnchor::InstallDir, remote_enabled)
    }

    /// Startup warnings for missing non-optional archives. Optional misses are
    /// absent. Each warning names the resolved path.
    pub fn startup_warnings(&self) -> Vec<String> {
        self.warnings.read().unwrap().clone()
    }

    /// Final search order, highest priority first. Includes the remote layers
    /// when a remote cache is configured.
    pub fn search_order(&self) -> Vec<String> {
        self.search_order.read().unwrap().clone()
    }

    /// The path-resolution log line from the last load.
    pub fn anchor_line(&self) -> String {
        self.anchor_line.read().unwrap().clone()
    }

    /// Source that [`Self::get`] would use for `path`, ignoring the derived
    /// texture cache. `None` when no mounted archive or remote entry has it.
    pub fn origin_of(&self, path: &str) -> Option<GameFileOrigin> {
        let archive_path = normalize_archive_path(path);
        if let Some(target) = self.map_alias(&archive_path)
            && let Some(origin) = self.origin_exact(&target)
        {
            return Some(origin);
        }
        self.origin_exact(&archive_path)
    }

    fn origin_exact(&self, archive_path: &str) -> Option<GameFileOrigin> {
        for mounted in self.archives.read().unwrap().iter() {
            if mounted.role == ArchiveRole::AssetCache || !mounted.archive.file_exists(archive_path) {
                continue;
            }
            let path = mounted.location.clone();
            return Some(match mounted.role {
                ArchiveRole::Overlay => GameFileOrigin::Overlay { path },
                ArchiveRole::Configured => GameFileOrigin::ConfiguredArchive { path },
                ArchiveRole::Korangar => GameFileOrigin::KorangarArchive { path },
                ArchiveRole::AssetCache => unreachable!("asset cache is skipped above"),
            });
        }
        let remote = self.remote.read().unwrap();
        let remote = remote.as_ref()?;
        if remote.is_cached_locally(archive_path) {
            Some(GameFileOrigin::RemoteCache)
        } else if remote.contains(archive_path) {
            Some(GameFileOrigin::RemoteFetch)
        } else {
            None
        }
    }

    fn mount_at(&self, base: &Path, anchor: PathAnchor, remote_enabled: bool) -> Result<(), String> {
        let line = anchor_log_line(anchor, base);
        eprintln!("{line}");
        *self.anchor_line.write().unwrap() = line;

        let resolved = GameArchiveList::load_resolved(base, remote_enabled)?;
        for warning in &resolved.warnings {
            eprintln!("{warning}");
        }
        *self.warnings.write().unwrap() = resolved.warnings.clone();

        let mut opened = open_existing(&resolved, &mut self.warnings.write().unwrap());
        for (role, path, archive) in opened.drain(..).rev() {
            let location = path.display().to_string();
            self.add_mounted(archive, true, role, location);
        }

        let mut order = Vec::new();
        for mounted in self.archives.read().unwrap().iter() {
            if mounted.role == ArchiveRole::AssetCache {
                continue;
            }
            order.push(format!("{}: {}", mounted.role.tag(), mounted.location));
        }
        if remote_enabled {
            order.push("remote verified cache".to_owned());
            order.push("remote fetch".to_owned());
        }
        #[cfg(feature = "debug")]
        {
            print_debug!("Game file search order (highest priority first):");
            for (index, entry) in order.iter().enumerate() {
                print_debug!("  {}: {entry}", index + 1);
            }
        }
        *self.search_order.write().unwrap() = order;

        let mounted_any = self.archives.read().unwrap().iter().any(|archive| archive.is_game_archive);
        if !remote_enabled && !mounted_any {
            return Err(
                "No game assets configured: add licensed local GRFs/overlays to client/game_archives.ron, or configure \
                 KORANGAR_REMOTE_BASE_URL and a staged manifest/cache. See docs/turoran-native-test.md."
                    .into(),
            );
        }
        Ok(())
    }

    pub fn calculate_hash(&self) -> Hash {
        let mut hasher = blake3::Hasher::new_derive_key(GAME_FILE_DERIVE_KEY);
        // Role and resolved path are part of the key so a different overlay set
        // cannot reuse `cache.7z` when the file bytes happen to match.
        self.archives
            .read()
            .unwrap()
            .iter()
            .filter(|archive| archive.is_game_archive)
            .for_each(|archive| {
                hasher.update(archive.role.tag().as_bytes());
                hasher.update(b"\0");
                hasher.update(archive.location.as_bytes());
                hasher.update(b"\0");
                archive.archive.hash(&mut hasher);
                hasher.update(b"\0");
            });
        if let Some(remote) = self.remote.read().unwrap().as_ref() {
            hasher.update(b"remote\0");
            hasher.update(remote.revision().as_bytes());
        }
        hasher.finalize()
    }

    pub fn get_files_with_extension(&self, extensions: &[&str]) -> Vec<String> {
        let mut files = Vec::new();
        self.archives
            .read()
            .unwrap()
            .iter()
            .for_each(|archive| archive.archive.get_files_with_extension(&mut files, extensions));

        if let Some(remote) = self.remote.read().unwrap().as_ref() {
            files.extend(
                remote
                    .paths()
                    .filter(|path| extensions.iter().any(|ext| path.ends_with(ext)))
                    .map(str::to_owned),
            );
        }
        files.sort();
        files.dedup();

        files
    }

    #[allow(unused_variables)]
    pub fn load_cache_archive(&self, game_file_hash: Hash) {
        let path = Path::new(CACHE_FILE_NAME);

        if !path.exists() && !path.is_dir() {
            return;
        }

        let archive = Box::new(SevenZipArchive::from_path(path));

        let Some(hash_file) = archive.get_file_by_path(HASH_FILE_PATH) else {
            #[cfg(feature = "debug")]
            print_debug!("Can't find game hash file. Using empty cache");
            return;
        };

        let Ok(stored) = Hash::from_hex(hash_file) else {
            #[cfg(feature = "debug")]
            print_debug!("Can't read game hash file. Using empty cache");
            return;
        };

        if stored != game_file_hash {
            eprintln!("warning: asset cache does not match the current archives and overlays; not using {CACHE_FILE_NAME}");
            #[cfg(feature = "debug")]
            print_debug!("[{}] Cache is out of sync. Please re-sync or delete the cache", "error".red());
            return;
        }

        self.add_archive(archive, false);
    }

    /// Finish the pinned snapshot when remote assets are configured. A failure
    /// is a status line, not a startup error: later reads stay on demand.
    /// The line names a virtual path only and never a URL.
    pub(crate) fn prefetch_remote_status(&self) -> String {
        let Some(remote) = self.remote.read().unwrap().clone() else {
            return "Remote asset prefetch was requested but no remote cache is configured. Startup continues.".to_owned();
        };
        let report = remote.prefetch();
        match (report.error, report.complete) {
            (None, true) => "Remote asset prefetch verified the pinned snapshot locally.".to_owned(),
            (None, false) => "Remote asset prefetch reported success but the pinned snapshot is incomplete. Startup continues with \
                              on-demand retrieval."
                .to_owned(),
            (Some(error), complete) => format!(
                "Remote asset prefetch did not finish ({error}). Pinned snapshot complete: {complete}. Startup continues with on-demand \
                 retrieval."
            ),
        }
    }
}

/// Highest priority first. Missing paths are already warned about and are not
/// opened. A later entry that resolves to the same filesystem path is skipped.
fn open_existing(resolved: &ResolvedArchiveConfig, warnings: &mut Vec<String>) -> Vec<(ArchiveRole, PathBuf, Box<dyn Archive>)> {
    let mut planned = Vec::new();
    for entry in &resolved.overlays {
        push_existing(&mut planned, ArchiveRole::Overlay, entry);
    }
    for entry in resolved.archives.iter().rev() {
        push_existing(&mut planned, ArchiveRole::Configured, entry);
    }
    if let Some(korangar) = korangar_archive_dir(&resolved.base)
        && !planned.iter().any(|(_, path)| same_archive_path(path, &korangar))
    {
        planned.push((ArchiveRole::Korangar, korangar));
    }

    let mut opened = Vec::new();
    for (role, path) in planned {
        match GameFileLoader::load_archive_from_path(&path) {
            Ok(archive) => opened.push((role, path, archive)),
            Err(warning) => {
                eprintln!("{warning}");
                warnings.push(warning);
            }
        }
    }
    opened
}

fn push_existing(planned: &mut Vec<(ArchiveRole, PathBuf)>, role: ArchiveRole, entry: &ResolvedEntry) {
    if !entry.path.exists() || planned.iter().any(|(_, path)| same_archive_path(path, &entry.path)) {
        return;
    }
    planned.push((role, entry.path.clone()));
}

pub fn fix_broken_texture_file_endings(path: &str) -> String {
    let mut path = path.to_string();

    if path.ends_with(".bm") {
        path.push('p');
    }

    if path.ends_with(".jp") {
        path.push('g');
    }

    if path.ends_with(".pn") {
        path.push('g');
    }

    if path.ends_with(".tg") {
        path.push('a');
    }

    path
}

#[cfg(test)]
mod prefetch_tests {
    use super::*;

    #[test]
    fn prefetch_without_remote_is_a_soft_status() {
        let loader = GameFileLoader::default();
        let line = loader.prefetch_remote_status();
        assert!(line.contains("no remote cache"));
        assert!(!line.contains("http"));
    }

    #[test]
    fn aliased_maps_load_their_target_files() {
        use std::fs;

        let root = std::env::temp_dir().join(format!("korangar-resname-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let data = root.join("data");
        fs::create_dir_all(&data).unwrap();
        fs::write(
            data.join("resnametable.txt"),
            "06guild_01.rsw#guild_vs2.rsw#\r\n06guild_01.gat#guild_vs2.gat#\r\n",
        )
        .unwrap();
        fs::write(data.join("guild_vs2.rsw"), b"rsw").unwrap();
        fs::write(data.join("guild_vs2.gat"), b"gat").unwrap();
        fs::write(data.join("prt_in.rsw"), b"prt_in").unwrap();

        let loader = GameFileLoader::default();
        loader.add_archive(Box::new(FolderArchive::from_path(&root)), true);

        assert_eq!(loader.get(r"data\06guild_01.rsw").unwrap(), b"rsw");
        assert_eq!(loader.get("data/06GUILD_01.gat").unwrap(), b"gat");
        assert!(loader.file_exists(r"data\06guild_01.rsw"));
        // Maps without an entry and maps whose alias is missing keep their own files.
        assert_eq!(loader.get(r"data\prt_in.rsw").unwrap(), b"prt_in");
        assert!(loader.get(r"data\06guild_01.gnd").is_err());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn local_archive_is_used_before_remote_and_hangul_keys_match() {
        use std::fs;
        use std::sync::Arc;

        use korangar_loaders::remote::{PersistentAssets, RemoteConfig};

        let root = std::env::temp_dir().join(format!("korangar-local-before-remote-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let folder = root.join("archive");
        let cache = root.join("cache");
        let model = folder.join("data/model/내부소품");
        fs::create_dir_all(&model).unwrap();
        fs::create_dir_all(cache.join("objects")).unwrap();
        fs::write(model.join("x.rsm"), b"local-bytes").unwrap();

        let remote_bytes = b"remote-bytes";
        let sha = "0fcb052055ffc840bf26ce7ad980f959da83ca69c8439369053cee468ae93463";
        fs::write(cache.join("objects").join(sha), remote_bytes).unwrap();
        // `revision` is the SHA-256 of this compact `files` array.
        let manifest = r#"{"format_version":1,"revision":"c4b346315360558720621f85128c49258448611a1922b170071d148c390673ec","files":[{"path":"data/model/내부소품/x.rsm","size":12,"sha256":"0fcb052055ffc840bf26ce7ad980f959da83ca69c8439369053cee468ae93463"}]}"#;
        fs::write(cache.join("manifest.json"), manifest).unwrap();

        let assets = PersistentAssets::open(RemoteConfig {
            base_url: "https://example.invalid/client/".into(),
            cache_dir: cache,
            allow_http: false,
            offline: true,
        })
        .unwrap();

        let loader = GameFileLoader::default();
        loader.add_archive(Box::new(FolderArchive::from_path(&folder)), true);
        *loader.remote.write().unwrap() = Some(Arc::new(assets));

        let loaded = loader.get(r"DATA/Model/내부소품/x.rsm").unwrap();
        assert_eq!(loaded, b"local-bytes");
        assert!(loader.file_exists(r"data\model\내부소품\x.rsm"));

        fs::remove_file(model.join("x.rsm")).unwrap();
        let reloaded = GameFileLoader::default();
        // Folder index was captured at open; build a fresh archive without the file.
        let empty = root.join("empty");
        fs::create_dir_all(empty.join("data")).unwrap();
        reloaded.add_archive(Box::new(FolderArchive::from_path(&empty)), true);
        *reloaded.remote.write().unwrap() = loader.remote.read().unwrap().clone();
        assert_eq!(reloaded.get(r"data\model\내부소품\x.rsm").unwrap(), b"remote-bytes");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn manifest_hangul_file_exists_is_not_a_local_hit_and_cp949_is_selected() {
        use std::fs;
        use std::io::Cursor;

        use korangar_loaders::remote::{PersistentAssets, RemoteConfig};

        use crate::interface::windows::{InterfaceTextureAttempt, interface_texture_candidates, select_interface_texture};
        use crate::loaders::color_image_decodes;

        let root = std::env::temp_dir().join(format!("korangar-worldmap-cp949-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let folder = root.join("archive");
        let cache = root.join("cache");
        fs::create_dir_all(cache.join("objects")).unwrap();

        let candidates = interface_texture_candidates("worldmap.jpg");
        let latin_relative = candidates[1].replace('\\', "/");
        let latin_file = folder.join("data").join("texture").join(&latin_relative);
        fs::create_dir_all(latin_file.parent().unwrap()).unwrap();
        let image = image::RgbaImage::from_pixel(2, 2, image::Rgba([12, 40, 20, 255]));
        let mut jpeg = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut Cursor::new(&mut jpeg), image::ImageFormat::Jpeg)
            .unwrap();
        fs::write(&latin_file, &jpeg).unwrap();

        // Hangul is advertised by the manifest only. The object is absent and offline,
        // so the Hangul read fails. `revision` is the SHA-256 of the compact `files`
        // array.
        let manifest = r#"{"format_version":1,"revision":"94db9853d23c9aa631de84dc94c4f277f6541573954a0d91c7cb88285278c4ef","files":[{"path":"data/texture/유저인터페이스/worldmap.jpg","size":12,"sha256":"0fcb052055ffc840bf26ce7ad980f959da83ca69c8439369053cee468ae93463"}]}"#;
        fs::write(cache.join("manifest.json"), manifest).unwrap();
        let assets = PersistentAssets::open(RemoteConfig {
            base_url: "https://example.invalid/client/".into(),
            cache_dir: cache,
            allow_http: false,
            offline: true,
        })
        .unwrap();

        let loader = GameFileLoader::default();
        loader.add_archive(Box::new(FolderArchive::from_path(&folder)), true);
        *loader.remote.write().unwrap() = Some(Arc::new(assets));

        let hangul = format!(r"data\texture\{}", candidates[0]);
        let latin = format!(r"data\texture\{}", candidates[1]);
        assert!(loader.file_exists(&hangul), "manifest entry counts as file_exists");
        assert!(!loader.local_file_exists(&hangul), "manifest entry is not a local archive hit");
        assert!(loader.get(&hangul).is_err(), "hangul read fails when the object is absent");
        assert!(loader.local_file_exists(&latin));

        let mut hangul_reads = 0;
        let selected = select_interface_texture(
            "worldmap.jpg",
            |path| loader.local_file_exists(path),
            |path| {
                if path == hangul {
                    hangul_reads += 1;
                }
                match loader.get(path) {
                    Ok(bytes) => {
                        let relative = path.strip_prefix(r"data\texture\").unwrap_or(path);
                        color_image_decodes(relative, &bytes)
                    }
                    Err(_) => false,
                }
            },
        );
        assert_eq!(selected, InterfaceTextureAttempt::Ready(candidates[1].clone()));
        assert_eq!(hangul_reads, 0);
        let _ = fs::remove_dir_all(&root);
    }
}

#[cfg(test)]
mod archive_config_tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use korangar_loaders::remote::{PersistentAssets, RemoteConfig};

    use super::*;
    use crate::world::{display_name_from_table, load_map_display_names};

    fn temp_install(label: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("korangar-mount-{label}-{}-{id}", std::process::id()));
        fs::create_dir_all(path.join("client")).unwrap();
        path
    }

    fn write_virtual(root: &Path, virtual_path: &str, bytes: &[u8]) {
        let relative = virtual_path.replace('\\', "/");
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn overlay_beats_configured_archive_and_korangar_directory() {
        let install = temp_install("order");
        write_virtual(&install.join("overlay"), r"data\e2e\shared.txt", b"from-overlay");
        write_virtual(
            &install.join("overlay"),
            r"data\luafiles514\lua files\Enchant\EnchantList.lub",
            b"lub-from-overlay",
        );
        write_virtual(
            &install.join("overlay"),
            r"data\mapnametable.txt",
            b"prontera.rsw#Overlay Prontera#\n",
        );
        write_virtual(&install.join("packs"), r"data\e2e\shared.txt", b"from-configured");
        write_virtual(&install.join("packs"), r"data\e2e\only-configured.txt", b"from-configured");
        write_virtual(&install.join("archive"), r"data\e2e\shared.txt", b"from-korangar");
        write_virtual(&install.join("archive"), r"data\e2e\only-configured.txt", b"from-korangar");
        write_virtual(&install.join("archive"), r"data\e2e\only-korangar.txt", b"from-korangar");
        fs::write(
            install.join("client/game_archives.ron"),
            r#"(
                overlays: ["overlay/", "lower-overlay/"],
                archives: ["packs/", (path: "missing-required.grf", optional: false), (path: "missing-optional.grf", optional: true)],
            )"#,
        )
        .unwrap();
        write_virtual(&install.join("lower-overlay"), r"data\e2e\shared.txt", b"from-lower-overlay");

        let loader = GameFileLoader::default();
        loader.load_from_install(&install).unwrap();

        assert_eq!(loader.get(r"data\e2e\shared.txt").unwrap(), b"from-overlay");
        assert!(matches!(
            loader.origin_of(r"data\e2e\shared.txt"),
            Some(GameFileOrigin::Overlay { .. })
        ));
        assert_eq!(
            loader.get(r"data\luafiles514\lua files\Enchant\EnchantList.lub").unwrap(),
            b"lub-from-overlay"
        );
        assert_eq!(loader.get(r"data\e2e\only-configured.txt").unwrap(), b"from-configured");
        assert!(matches!(
            loader.origin_of(r"data\e2e\only-configured.txt"),
            Some(GameFileOrigin::ConfiguredArchive { .. })
        ));
        assert_eq!(loader.get(r"data\e2e\only-korangar.txt").unwrap(), b"from-korangar");
        assert!(matches!(
            loader.origin_of(r"data\e2e\only-korangar.txt"),
            Some(GameFileOrigin::KorangarArchive { .. })
        ));

        let names = load_map_display_names(&loader);
        assert_eq!(display_name_from_table(&names, "prontera.rsw"), "Overlay Prontera");

        let warnings = loader.startup_warnings();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("missing-required.grf"));
        assert!(warnings[0].contains(&install.display().to_string()));
        assert!(!warnings.iter().any(|warning| warning.contains("missing-optional")));

        let order = loader.search_order();
        assert_eq!(order[0], format!("overlay: {}", install.join("overlay").display()));
        assert_eq!(order[1], format!("overlay: {}", install.join("lower-overlay").display()));
        assert!(order.iter().any(|entry| entry.starts_with("archive:") && entry.contains("packs")));
        assert!(order.iter().any(|entry| entry.starts_with("korangar:")));
        let overlay_at = order.iter().position(|entry| entry.starts_with("overlay:")).unwrap();
        let archive_at = order.iter().position(|entry| entry.starts_with("archive:")).unwrap();
        let korangar_at = order.iter().position(|entry| entry.starts_with("korangar:")).unwrap();
        assert!(overlay_at < archive_at && archive_at < korangar_at, "{order:?}");
        assert!(loader.anchor_line().contains("install directory"));
        assert!(loader.anchor_line().contains(&install.display().to_string()));
        assert_ne!(install, std::env::current_dir().unwrap());

        let _ = fs::remove_dir_all(install);
    }

    #[test]
    fn nothing_mounted_still_reports_the_missing_archive() {
        let install = temp_install("empty");
        fs::write(
            install.join("client/game_archives.ron"),
            r#"(archives: ["missing-required.grf"], overlays: [])"#,
        )
        .unwrap();
        let loader = GameFileLoader::default();
        let error = loader.load_from_install(&install).unwrap_err();
        assert!(error.contains("No game assets"));
        assert!(
            loader
                .startup_warnings()
                .iter()
                .any(|warning| warning.contains("missing-required.grf"))
        );
        let _ = fs::remove_dir_all(install);
    }

    #[test]
    fn overlay_identity_changes_the_archive_hash() {
        let first = temp_install("hash-a");
        let second = temp_install("hash-b");
        for install in [&first, &second] {
            write_virtual(&install.join("overlay"), r"data\e2e\shared.txt", b"same-bytes");
            fs::write(
                install.join("client/game_archives.ron"),
                r#"(overlays: ["overlay/"], archives: [])"#,
            )
            .unwrap();
        }
        let loader_a = GameFileLoader::default();
        loader_a.load_from_install(&first).unwrap();
        let hash_a = loader_a.calculate_hash();
        fs::write(first.join("overlay/data/e2e/shared.txt"), b"changed-bytes").unwrap();
        assert_ne!(loader_a.calculate_hash(), hash_a);

        let loader_b = GameFileLoader::default();
        loader_b.load_from_install(&second).unwrap();
        let restored = GameFileLoader::default();
        fs::write(first.join("overlay/data/e2e/shared.txt"), b"same-bytes").unwrap();
        restored.load_from_install(&first).unwrap();
        assert_eq!(restored.calculate_hash(), hash_a);
        assert_ne!(
            loader_b.calculate_hash(),
            hash_a,
            "a different overlay path must not reuse the cache key"
        );
        let _ = fs::remove_dir_all(first);
        let _ = fs::remove_dir_all(second);
    }

    #[test]
    fn replaced_grf_changes_the_archive_hash() {
        let install = temp_install("grf-hash");
        let grf = install.join("patch.grf");
        write_grf(&grf, b"first-bytes");
        fs::write(install.join("client/game_archives.ron"), r#"(archives: ["patch.grf"])"#).unwrap();

        let first = GameFileLoader::default();
        first.load_from_install(&install).unwrap();
        let hash_first = first.calculate_hash();
        drop(first);

        write_grf(&grf, b"second-bytes-differ");
        let second = GameFileLoader::default();
        second.load_from_install(&install).unwrap();
        assert_ne!(
            second.calculate_hash(),
            hash_first,
            "a replaced GRF at the same path must not reuse the cache key"
        );
        assert_eq!(second.get(r"data\e2e\body.txt").unwrap(), b"second-bytes-differ");
        let _ = fs::remove_dir_all(install);
    }

    fn write_grf(path: &Path, body: &[u8]) {
        use crate::loaders::archive::native::builder::NativeArchiveBuilder;
        use crate::loaders::archive::{Compression, Writable};

        let mut builder = NativeArchiveBuilder::from_path(path);
        builder.add_file(r"data\e2e\body.txt", body.to_vec(), Compression::Default);
        builder.finish().unwrap();
    }

    #[test]
    fn remote_cache_is_below_local_overlays() {
        let install = temp_install("remote");
        write_virtual(&install.join("overlay"), r"data\e2e\shared.txt", b"from-overlay");
        fs::write(
            install.join("client/game_archives.ron"),
            r#"(overlays: ["overlay/"], archives: [])"#,
        )
        .unwrap();

        let body = b"remote-bytes";
        let sha = "0fcb052055ffc840bf26ce7ad980f959da83ca69c8439369053cee468ae93463";
        let absent_sha = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let cache = install.join("cache");
        fs::create_dir_all(cache.join("objects")).unwrap();
        fs::write(cache.join("objects").join(sha), body).unwrap();
        fs::write(
            cache.join("loose-index.json"),
            format!(
                r#"{{"files":[{{"path":"data/e2e/shared.txt","size":12,"sha256":"{sha}"}},{{"path":"data/e2e/cached.txt","size":12,"sha256":"{sha}"}},{{"path":"data/e2e/fetch.txt","size":12,"sha256":"{absent_sha}"}}]}}"#
            ),
        )
        .unwrap();
        let assets = PersistentAssets::open(RemoteConfig {
            base_url: "https://example.invalid/client/".into(),
            cache_dir: cache,
            allow_http: false,
            offline: true,
        })
        .unwrap();

        let loader = GameFileLoader::default();
        *loader.remote.write().unwrap() = Some(Arc::new(assets));
        loader.load_from_install(&install).unwrap();

        assert_eq!(loader.get(r"data\e2e\shared.txt").unwrap(), b"from-overlay");
        assert!(matches!(
            loader.origin_of(r"data\e2e\shared.txt"),
            Some(GameFileOrigin::Overlay { .. })
        ));
        assert_eq!(loader.get(r"data\e2e\cached.txt").unwrap(), body);
        assert_eq!(loader.origin_of(r"data\e2e\cached.txt"), Some(GameFileOrigin::RemoteCache));
        assert_eq!(loader.origin_of(r"data\e2e\fetch.txt"), Some(GameFileOrigin::RemoteFetch));
        let order = loader.search_order();
        assert_eq!(order[order.len() - 2], "remote verified cache");
        assert_eq!(order.last().map(String::as_str), Some("remote fetch"));
        let _ = fs::remove_dir_all(install);
    }
}
