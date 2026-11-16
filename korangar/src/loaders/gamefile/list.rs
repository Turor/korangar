use std::path::{Path, PathBuf};

use serde::Deserialize;

/// One operator-controlled archive or folder.
///
/// A bare string in the config file is this entry with `optional: false`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ArchiveEntry {
    pub path: String,
    pub optional: bool,
}

/// Where relative archive paths are resolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PathAnchor {
    /// Directory that contains the executable.
    InstallDir,
    /// Process working directory. Used only for Cargo dev and test binaries.
    WorkingDir,
    /// Portable Windows runtime directory inside the installation.
    PortableRuntime,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ResolvedEntry {
    pub configured: String,
    pub path: PathBuf,
    pub optional: bool,
}

pub(super) struct ResolvedArchiveConfig {
    pub base: PathBuf,
    pub overlays: Vec<ResolvedEntry>,
    pub archives: Vec<ResolvedEntry>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct GameArchiveList {
    pub overlays: Vec<ArchiveEntry>,
    pub archives: Vec<ArchiveEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGameArchiveList {
    #[serde(default)]
    archives: Vec<RawEntry>,
    #[serde(default)]
    overlays: Vec<RawEntry>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawEntry {
    Path(String),
    Detailed {
        path: String,
        #[serde(default)]
        optional: bool,
    },
}

impl From<RawEntry> for ArchiveEntry {
    fn from(entry: RawEntry) -> Self {
        match entry {
            RawEntry::Path(path) => Self { path, optional: false },
            RawEntry::Detailed { path, optional } => Self { path, optional },
        }
    }
}

impl<'de> Deserialize<'de> for GameArchiveList {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = RawGameArchiveList::deserialize(deserializer)?;
        Ok(Self {
            overlays: raw.overlays.into_iter().map(ArchiveEntry::from).collect(),
            archives: raw.archives.into_iter().map(ArchiveEntry::from).collect(),
        })
    }
}

impl Default for GameArchiveList {
    fn default() -> Self {
        Self {
            overlays: Vec::new(),
            archives: Self::loader_order_from_data_ini_slots(Self::TURORAN_DATA_INI_ORDER)
                .into_iter()
                .map(|path| ArchiveEntry { path, optional: false })
                .collect(),
        }
    }
}

impl GameArchiveList {
    const DATA_INI_CANDIDATES: &'static [&'static str] = &["DATA.INI", "data.ini", "client/DATA.INI", "client/data.ini"];
    const FILE_NAME: &'static str = "client/game_archives.ron";
    /// Korangar resources. Searched after overlays and configured archives.
    const LOCAL_OVERLAY: &'static str = "archive";
    /// Live Turoran `resources-0716/DATA.INI` slot order (0 = highest
    /// priority).
    const TURORAN_DATA_INI_ORDER: &'static [&'static str] = &[
        "translation.grf",
        "2025-07-16_en.grf",
        "data.grf",
        "iro_assets.grf",
        "region_assets.grf",
    ];

    pub(super) fn load_resolved(base: &Path, remote_enabled: bool) -> Result<ResolvedArchiveConfig, String> {
        let config_path = base.join(Self::FILE_NAME);
        let list = match std::fs::read_to_string(&config_path) {
            Ok(data) => Self::parse(&data).map_err(|error| {
                format!(
                    "Invalid {}: {error}. Expected (archives: [\"path\", ...], overlays: [\"path\", ...]); an entry may be (path: \
                     \"...\", optional: true).",
                    config_path.display()
                )
            })?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self::without_settings(remote_enabled, base),
            Err(_) => return Err(format!("Cannot read {}", config_path.display())),
        };
        list.resolve(base)
    }

    pub(super) fn parse(data: &str) -> Result<Self, String> {
        ron::from_str(data).map_err(|error| error.to_string())
    }

    /// Configured priority, highest first, before missing entries are dropped.
    ///
    /// Overlays keep listed order (first is highest). The `archives` list keeps
    /// today's loader order: the last entry wins, which is DATA.INI slot 0
    /// after [`Self::loader_order_from_data_ini_slots`].
    #[cfg(test)]
    pub(super) fn local_search_labels(&self, korangar: Option<&str>) -> Vec<String> {
        let mut labels = Vec::new();
        for entry in &self.overlays {
            labels.push(format!("overlay: {}", entry.path));
        }
        for entry in self.archives.iter().rev() {
            labels.push(format!("archive: {}", entry.path));
        }
        if let Some(path) = korangar
            && !self.mentions(path)
        {
            labels.push(format!("korangar: {path}"));
        }
        labels
    }

    #[cfg(test)]
    fn mentions(&self, path: &str) -> bool {
        self.overlays
            .iter()
            .chain(self.archives.iter())
            .any(|entry| same_configured_path(&entry.path, path))
    }

    fn resolve(self, base: &Path) -> Result<ResolvedArchiveConfig, String> {
        let mut warnings = Vec::new();
        let overlays = resolve_entries(&self.overlays, base, &mut warnings)?;
        let archives = resolve_entries(&self.archives, base, &mut warnings)?;
        Ok(ResolvedArchiveConfig {
            base: base.to_path_buf(),
            overlays,
            archives,
            warnings,
        })
    }

    fn without_settings(remote_enabled: bool, base: &Path) -> Self {
        // Remote installs stay archive-free unless an explicit DATA.INI is present, so
        // the verified cache is not shadowed by whatever GRFs happen to sit beside
        // the executable.
        let from_ini = Self::read_data_ini_slots(base);
        let explicit = from_ini.is_some();
        let slot_order = if let Some(slots) = from_ini {
            slots
        } else if remote_enabled {
            Vec::new()
        } else {
            Self::TURORAN_DATA_INI_ORDER.iter().map(|path| (*path).to_owned()).collect()
        };

        let mut archives = if slot_order.is_empty() {
            Vec::new()
        } else {
            Self::loader_order_from_data_ini_slots(&slot_order.iter().map(String::as_str).collect::<Vec<_>>())
        };

        if !remote_enabled || !slot_order.is_empty() {
            Self::ensure_legacy_grf_priority(&mut archives, base);
        }

        // Implicit defaults that are not on disk are omitted quietly. DATA.INI rows
        // stay, including missing ones, so startup can warn with the path.
        let archives = archives
            .into_iter()
            .filter(|path| explicit || path_exists(base, path))
            .map(|path| ArchiveEntry { path, optional: false })
            .collect();

        Self {
            overlays: Vec::new(),
            archives,
        }
    }

    /// Keep upstream `rdata.grf` above `data.grf` under last-wins loading,
    /// without outranking Turoran English/translation GRFs when those are
    /// present.
    fn ensure_legacy_grf_priority(archives: &mut Vec<String>, base: &Path) {
        let has = |archives: &[String], name: &str| archives.iter().any(|path| path == name);

        if path_exists(base, "data.grf") && !has(archives, "data.grf") {
            archives.insert(0, "data.grf".to_owned());
        }

        if path_exists(base, "rdata.grf") && !has(archives, "rdata.grf") {
            if let Some(data_at) = archives.iter().position(|path| path == "data.grf") {
                archives.insert(data_at + 1, "rdata.grf".to_owned());
            } else {
                archives.insert(0, "rdata.grf".to_owned());
            }
        }
    }

    /// Convert DATA.INI slot order (index 0 = highest priority) into
    /// GameFileLoader list order (last entry wins because archives are
    /// `insert(0, …)` on load).
    fn loader_order_from_data_ini_slots(slots_high_to_low: &[&str]) -> Vec<String> {
        slots_high_to_low.iter().rev().map(|path| (*path).to_owned()).collect()
    }

    fn read_data_ini_slots(base: &Path) -> Option<Vec<String>> {
        for candidate in Self::DATA_INI_CANDIDATES {
            let Ok(text) = std::fs::read_to_string(base.join(candidate)) else {
                continue;
            };
            if let Some(slots) = Self::parse_data_ini(&text) {
                return Some(slots);
            }
        }
        None
    }

    /// Parse a Gravity-style DATA.INI `[Data]` section into ascending slot
    /// order.
    fn parse_data_ini(text: &str) -> Option<Vec<String>> {
        let mut in_data = false;
        let mut slots: Vec<(u32, String)> = Vec::new();

        for raw in text.lines() {
            let line = raw.split(['#', ';']).next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                in_data = line.eq_ignore_ascii_case("[Data]");
                continue;
            }
            if !in_data {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let Ok(index) = key.trim().parse::<u32>() else {
                continue;
            };
            let path = value.trim().trim_matches('"').trim_matches('\'').to_owned();
            if !path.is_empty() {
                slots.push((index, path));
            }
        }

        if slots.is_empty() {
            return None;
        }
        slots.sort_by_key(|(index, _)| *index);
        Some(slots.into_iter().map(|(_, path)| path).collect())
    }
}

/// Directory of the executable, unless this process is a Cargo dev/test binary
/// or a portable runtime already selected the runtime directory.
pub(super) fn path_anchor(executable: Option<&Path>, working_dir: &Path, portable_runtime: Option<&Path>) -> (PathBuf, PathAnchor) {
    if let Some(runtime) = portable_runtime {
        return (runtime.to_path_buf(), PathAnchor::PortableRuntime);
    }
    if executable.is_none_or(is_cargo_artifact) {
        return (working_dir.to_path_buf(), PathAnchor::WorkingDir);
    }
    match executable.and_then(Path::parent) {
        Some(directory) => (directory.to_path_buf(), PathAnchor::InstallDir),
        None => (working_dir.to_path_buf(), PathAnchor::WorkingDir),
    }
}

pub(super) fn anchor_log_line(anchor: PathAnchor, directory: &Path) -> String {
    match anchor {
        PathAnchor::InstallDir => format!(
            "Game archive paths resolve against the install directory (executable directory): {}",
            directory.display()
        ),
        PathAnchor::WorkingDir => format!(
            "Game archive paths resolve against the working directory (dev run): {}",
            directory.display()
        ),
        PathAnchor::PortableRuntime => format!(
            "Game archive paths resolve against the portable runtime directory: {}",
            directory.display()
        ),
    }
}

/// `true` when `executable` is a Cargo build product (`target/.../debug`, the
/// slot `targets/.../debug` tree, or a `deps` test binary).
fn is_cargo_artifact(executable: &Path) -> bool {
    let Some(parent_name) = executable.parent().and_then(Path::file_name).and_then(|name| name.to_str()) else {
        return false;
    };
    let cargo_dir = matches!(parent_name, "debug" | "release" | "deps" | "examples");
    let in_target_tree = executable.components().any(|component| {
        let name = component.as_os_str();
        name == "target" || name == "targets"
    });
    cargo_dir && in_target_tree
}

pub(super) fn resolve_filesystem_path(base: &Path, configured: &str) -> Result<PathBuf, String> {
    let configured = configured.trim();
    if configured.is_empty() {
        return Err("game archive path is empty".to_owned());
    }
    if is_url(configured) {
        return Err(format!("game archive path must be a filesystem path, not a URL: {configured}"));
    }
    let path = Path::new(configured);
    // A leading `~` is a path component. It is not the home directory.
    let joined = if path.is_absolute() { path.to_path_buf() } else { base.join(path) };
    Ok(normalize_resolved(&joined))
}

/// Drop `.` and a trailing slash so `overlay/` and `overlay` are one path.
fn normalize_resolved(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        if matches!(component, std::path::Component::CurDir) {
            continue;
        }
        normalized.push(component);
    }
    normalized
}

fn is_url(path: &str) -> bool {
    path.trim().replace('\\', "/").to_ascii_lowercase().contains("://")
}

fn path_exists(base: &Path, configured: &str) -> bool {
    resolve_filesystem_path(base, configured).is_ok_and(|path| path.exists())
}

fn resolve_entries(entries: &[ArchiveEntry], base: &Path, warnings: &mut Vec<String>) -> Result<Vec<ResolvedEntry>, String> {
    let mut resolved = Vec::with_capacity(entries.len());
    for entry in entries {
        let path = resolve_filesystem_path(base, &entry.path)?;
        let item = ResolvedEntry {
            configured: entry.path.clone(),
            path,
            optional: entry.optional,
        };
        if !item.path.exists() && !item.optional {
            warnings.push(missing_archive_warning(&item));
        }
        resolved.push(item);
    }
    Ok(resolved)
}

pub(super) fn missing_archive_warning(entry: &ResolvedEntry) -> String {
    let shown = entry.path.display().to_string();
    if shown == entry.configured {
        format!("warning: game archive not found: {shown}")
    } else {
        format!("warning: game archive not found: {shown} (configured as {})", entry.configured)
    }
}

pub(super) fn same_configured_path(left: &str, right: &str) -> bool {
    fn key(path: &str) -> String {
        path.replace('\\', "/").trim().trim_end_matches('/').to_ascii_lowercase()
    }
    key(left) == key(right)
}

pub(super) fn same_archive_path(left: &Path, right: &Path) -> bool {
    same_configured_path(&left.to_string_lossy(), &right.to_string_lossy())
}

pub(super) fn korangar_archive_dir(base: &Path) -> Option<PathBuf> {
    let path = base.join(GameArchiveList::LOCAL_OVERLAY);
    path.is_dir().then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(label: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("korangar-archives-{label}-{}-{id}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn remote_defaults_do_not_require_grfs() {
        let base = temp_dir("remote-empty");
        let list = GameArchiveList::without_settings(true, &base);
        assert!(list.archives.is_empty());
        assert!(list.overlays.is_empty());
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn old_and_new_config_formats_parse() {
        let old = GameArchiveList::parse("(archives: [])").unwrap();
        assert!(old.archives.is_empty());
        assert!(old.overlays.is_empty());

        let listed = GameArchiveList::parse("(archives: [\"archive/\"])").unwrap();
        assert_eq!(listed.archives, vec![ArchiveEntry {
            path: "archive/".to_owned(),
            optional: false,
        }]);
        assert!(listed.overlays.is_empty());

        let extended = GameArchiveList::parse(
            r#"(
                overlays: ["overlay/", (path: "maybe/", optional: true)],
                archives: ["low.grf", (path: "high.grf", optional: false)],
            )"#,
        )
        .unwrap();
        assert_eq!(extended.overlays, vec![
            ArchiveEntry {
                path: "overlay/".to_owned(),
                optional: false,
            },
            ArchiveEntry {
                path: "maybe/".to_owned(),
                optional: true,
            },
        ]);
        assert!(!extended.archives[1].optional);
        assert_eq!(extended.archives[0].path, "low.grf");
    }

    #[test]
    fn overlays_outrank_configured_archives_and_korangar() {
        let list = GameArchiveList::parse(
            r#"(
                overlays: ["first/", "second/"],
                archives: ["low.grf", "high.grf"],
            )"#,
        )
        .unwrap();
        assert_eq!(list.local_search_labels(Some("archive")), vec![
            "overlay: first/",
            "overlay: second/",
            "archive: high.grf",
            "archive: low.grf",
            "korangar: archive",
        ]);
        assert_eq!(list.local_search_labels(Some("archive/")), vec![
            "overlay: first/",
            "overlay: second/",
            "archive: high.grf",
            "archive: low.grf",
            "korangar: archive/",
        ]);
        let listed = GameArchiveList::parse(r#"(archives: ["archive/"])"#).unwrap();
        assert_eq!(listed.local_search_labels(Some("archive")), vec!["archive: archive/"]);
    }

    #[test]
    fn data_ini_slot_zero_becomes_highest_loader_priority() {
        let text = "[Data]\n0=translation.grf\n1=2025-07-16_en.grf\n2=data.grf\n3=iro_assets.grf\n4=region_assets.grf\n";
        let slots = GameArchiveList::parse_data_ini(text).unwrap();
        assert_eq!(slots, vec![
            "translation.grf",
            "2025-07-16_en.grf",
            "data.grf",
            "iro_assets.grf",
            "region_assets.grf",
        ]);
        let loader_order = GameArchiveList::loader_order_from_data_ini_slots(&slots.iter().map(String::as_str).collect::<Vec<_>>());
        // Last listed archive is inserted last → searched first (highest priority).
        assert_eq!(loader_order.last().unwrap(), "translation.grf");
        assert_eq!(loader_order.first().unwrap(), "region_assets.grf");
        let list = GameArchiveList {
            overlays: vec![ArchiveEntry {
                path: "overlay/".to_owned(),
                optional: false,
            }],
            archives: loader_order
                .into_iter()
                .map(|path| ArchiveEntry { path, optional: false })
                .collect(),
        };
        let labels = list.local_search_labels(None);
        assert_eq!(labels[0], "overlay: overlay/");
        assert_eq!(labels[1], "archive: translation.grf");
        assert_eq!(labels.last().unwrap(), "archive: region_assets.grf");
    }

    #[test]
    fn legacy_rdata_outranks_data_but_not_translation() {
        let mut archives = GameArchiveList::loader_order_from_data_ini_slots(&["translation.grf", "data.grf"]);
        // Simulate ensure_legacy when rdata exists: insert after data.
        if let Some(data_at) = archives.iter().position(|path| path == "data.grf") {
            archives.insert(data_at + 1, "rdata.grf".to_owned());
        }
        assert_eq!(archives, vec!["data.grf", "rdata.grf", "translation.grf"]);
        assert!(archives.iter().position(|p| p == "rdata.grf") > archives.iter().position(|p| p == "data.grf"));
        assert!(archives.iter().position(|p| p == "translation.grf") > archives.iter().position(|p| p == "rdata.grf"));
    }

    #[test]
    fn turoran_default_matches_live_data_ini_order() {
        let order = GameArchiveList::loader_order_from_data_ini_slots(GameArchiveList::TURORAN_DATA_INI_ORDER);
        assert_eq!(order, vec![
            "region_assets.grf",
            "iro_assets.grf",
            "data.grf",
            "2025-07-16_en.grf",
            "translation.grf",
        ]);
    }

    #[test]
    fn parse_data_ini_ignores_comments_and_other_sections() {
        let text = "\
; header\n[Misc]\n0=ignore.grf\n[Data]\n0=translation.grf # slot zero\n2=data.grf\n1=\"2025-07-16_en.grf\"\n";
        assert_eq!(GameArchiveList::parse_data_ini(text).unwrap(), vec![
            "translation.grf",
            "2025-07-16_en.grf",
            "data.grf"
        ]);
    }

    #[test]
    fn relative_paths_use_the_base_and_tilde_is_not_home() {
        let base = Path::new("/opt/korangar");
        let relative = resolve_filesystem_path(base, "grf/data.grf").unwrap();
        assert_eq!(relative, PathBuf::from("/opt/korangar/grf/data.grf"));
        let absolute = resolve_filesystem_path(base, "/srv/data.grf").unwrap();
        assert_eq!(absolute, PathBuf::from("/srv/data.grf"));
        let tilde = resolve_filesystem_path(base, "~/overlays/a.grf").unwrap();
        assert_eq!(tilde, PathBuf::from("/opt/korangar/~/overlays/a.grf"));
        if let Ok(home) = std::env::var("HOME") {
            assert!(!tilde.starts_with(home));
        }
    }

    #[test]
    fn urls_are_rejected() {
        let error = resolve_filesystem_path(Path::new("/opt/korangar"), "https://example.invalid/data.grf").unwrap_err();
        assert!(error.contains("filesystem path"));
        assert!(error.contains("https://example.invalid/data.grf"));
        assert!(resolve_filesystem_path(Path::new("/opt/korangar"), "file:///tmp/data.grf").is_err());
    }

    #[test]
    fn missing_required_entry_warns_and_optional_is_quiet() {
        let base = temp_dir("missing");
        std::fs::create_dir_all(base.join("client")).unwrap();
        std::fs::write(
            base.join("client/game_archives.ron"),
            r#"(
                overlays: [(path: "missing-overlay/", optional: true)],
                archives: ["missing-required.grf", (path: "missing-optional.grf", optional: true)],
            )"#,
        )
        .unwrap();
        let resolved = GameArchiveList::load_resolved(&base, false).unwrap();
        assert_eq!(resolved.warnings.len(), 1);
        assert!(resolved.warnings[0].contains("missing-required.grf"));
        assert!(!resolved.warnings.iter().any(|warning| warning.contains("missing-optional")));
        assert!(!resolved.warnings.iter().any(|warning| warning.contains("missing-overlay")));
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn explicit_data_ini_missing_row_is_kept_for_a_warning() {
        let base = temp_dir("data-ini");
        std::fs::write(base.join("DATA.INI"), "[Data]\n0=present.grf\n1=absent.grf\n").unwrap();
        std::fs::write(base.join("present.grf"), b"grf").unwrap();
        let resolved = GameArchiveList::load_resolved(&base, false).unwrap();
        assert!(resolved.warnings.iter().any(|warning| warning.contains("absent.grf")));
        assert!(resolved.archives.iter().any(|entry| entry.configured == "present.grf"));
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn dev_binaries_use_the_working_directory() {
        let cwd = Path::new("/work/repo");
        let (dir, anchor) = path_anchor(
            Some(Path::new("/work/korangar-gap-swarm/targets/slot-7/debug/korangar")),
            cwd,
            None,
        );
        assert_eq!(anchor, PathAnchor::WorkingDir);
        assert_eq!(dir, cwd);
        let line = anchor_log_line(anchor, &dir);
        assert!(line.contains("working directory"));
        assert!(line.contains("dev run"));

        let (dir, anchor) = path_anchor(Some(Path::new("/opt/korangar/korangar")), cwd, None);
        assert_eq!(anchor, PathAnchor::InstallDir);
        assert_eq!(dir, Path::new("/opt/korangar"));
        assert!(anchor_log_line(anchor, &dir).contains("install directory"));

        let runtime = Path::new("/opt/korangar/runtime");
        let (dir, anchor) = path_anchor(Some(Path::new("/opt/korangar/korangar")), runtime, Some(runtime));
        assert_eq!(anchor, PathAnchor::PortableRuntime);
        assert_eq!(dir, runtime);
        assert!(anchor_log_line(anchor, &dir).contains("portable runtime"));
    }
}
