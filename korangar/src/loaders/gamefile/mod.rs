//! Manages archives where game assets are stored and provides convenient
//! methods to retrieve each of them individually. The archives implement the
//! [`Archive`] trait.

mod cache;
mod list;

use core::panic;
use std::path::Path;
use std::sync::{Arc, RwLock};

use blake3::Hash;
#[cfg(feature = "debug")]
use korangar_debug::logging::{Colorize, Timer, print_debug};
use korangar_loaders::remote::PersistentAssets;
use korangar_loaders::{FileLoader, FileNotFoundError};

pub use self::cache::{sync_cache_archive, texture_file_dds_name, video_file_ivf_name};
use self::list::GameArchiveList;
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

struct LoaderArchive {
    archive: Box<dyn Archive>,
    is_game_archive: bool,
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
}

impl FileLoader for GameFileLoader {
    fn get(&self, path: &str) -> Result<Vec<u8>, FileNotFoundError> {
        let archive_path = normalize_archive_path(path);
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
        self.remote
            .read()
            .unwrap()
            .as_ref()
            .map(|remote| {
                remote
                    .get(&archive_path)
                    .map_err(|error| FileNotFoundError::new(format!("{path}: verified remote retrieval failed: {error}")))
            })
            .unwrap_or_else(|| Err(FileNotFoundError::new(path.to_owned())))
    }
}

impl GameFileLoader {
    pub fn file_exists(&self, path: &str) -> bool {
        let archive_path = normalize_archive_path(path);
        self.archives
            .read()
            .unwrap()
            .iter()
            .any(|archive| archive.archive.file_exists(&archive_path))
            || self.remote.read().unwrap().as_ref().is_some_and(|r| r.contains(&archive_path))
    }

    fn add_archive(&self, archive: Box<dyn Archive>, is_game_archive: bool) {
        self.archives.write().unwrap().insert(0, LoaderArchive { archive, is_game_archive });
    }

    fn get_archive_type_by_path(path: &Path) -> ArchiveType {
        if path.is_dir() || path.display().to_string().ends_with('/') {
            ArchiveType::Folder
        } else if let Some(extension) = path.extension()
            && let Some("grf") = extension.to_str()
        {
            ArchiveType::Native
        } else if let Some(extension) = path.extension()
            && let Some("7z") = extension.to_str()
        {
            ArchiveType::SevenZip
        } else {
            panic!("Provided archive must be a directory or have a .grf extension")
        }
    }

    fn load_archive_from_path(path: &str) -> Box<dyn Archive> {
        let path = Path::new(path);

        match GameFileLoader::get_archive_type_by_path(path) {
            ArchiveType::Folder => Box::new(FolderArchive::from_path(path)),
            ArchiveType::Native => Box::new(NativeArchive::from_path(path)),
            ArchiveType::SevenZip => Box::new(SevenZipArchive::from_path(path)),
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
        let game_archive_list = GameArchiveList::load(remote_enabled)?;
        if !remote_enabled && game_archive_list.archives.is_empty() {
            return Err(
                "No game assets configured: add licensed local GRFs/overlays to client/game_archives.ron, or configure \
                 KORANGAR_REMOTE_BASE_URL and a staged manifest/cache. See docs/turoran-native-test.md."
                    .into(),
            );
        }
        for path in &game_archive_list.archives {
            // Missing archives are skipped (DATA.INI / partial installs). Remote mode
            // continues to serve files from the verified cache when local GRFs are absent.
            if !Path::new(path).exists() {
                #[cfg(feature = "debug")]
                print_debug!("Skipping missing game archive {}", path);
                continue;
            }
            let game_archive = Self::load_archive_from_path(path);
            self.add_archive(game_archive, true);
        }

        #[cfg(feature = "debug")]
        timer.stop();
        Ok(())
    }

    pub fn calculate_hash(&self) -> Hash {
        let mut hasher = blake3::Hasher::new_derive_key(GAME_FILE_DERIVE_KEY);
        self.archives
            .read()
            .unwrap()
            .iter()
            .filter(|archive| archive.is_game_archive)
            .for_each(|archive| archive.archive.hash(&mut hasher));
        if let Some(remote) = self.remote.read().unwrap().as_ref() {
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

        let Ok(_hash) = Hash::from_hex(hash_file) else {
            #[cfg(feature = "debug")]
            print_debug!("Can't read game hash file. Using empty cache");
            return;
        };

        #[cfg(feature = "debug")]
        if _hash != game_file_hash {
            print_debug!("[{}] Cache is out of sync. Please re-sync or delete the cache", "error".red());
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
}
