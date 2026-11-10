//! Env-gated headless login and command channel.
//!
//! Off unless `KORANGAR_AUTOPILOT` points at an account JSON file
//! `{username, password, login_address}`. The password is never written to
//! logs. See `docs/autopilot.md`.
//!
//! PIN entry is intentionally not sent. [`Autopilot::on_login_failed`] treats a
//! server message that mentions a PIN as a challenge and keeps
//! `KORANGAR_AUTOPILOT_PIN` available through [`Autopilot::take_pending_pin`]
//! for the G02 PIN flow.

mod command;
mod events;
mod keycode;

use std::collections::VecDeque;
use std::fs::File;
use std::io::{self, Read};
use std::os::unix::fs::{FileTypeExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ragnarok_packets::TilePosition;
use serde::Deserialize;
use winit::keyboard::KeyCode;

pub use self::command::{ClickKind, DumpPath};
use self::command::{Command, parse_command};
use crate::input::InputEvent;
use crate::interface::windows::WindowClass;

/// Linux `O_NONBLOCK`. Command files and fifos must not stall the frame loop.
const O_NONBLOCK: i32 = 0o4000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Idle,
    AwaitingLogin,
    AwaitingCharacters,
    Creating,
    EnteringMap,
    InWorld,
    AwaitingPin,
    Failed,
    Quit,
}

#[derive(Debug)]
pub enum Effect {
    /// Credentials stay on [`Autopilot`]. The client copies them into `LogIn`.
    Login,
    SelectServer(usize),
    SelectCharacter(usize),
    CreateCharacter {
        slot: usize,
        name: String,
        hair_color: u16,
        hair_style: u16,
        start_job: u16,
        sex: u8,
    },
    Input(InputEvent),
    Window {
        class: WindowClass,
        open: bool,
        event: InputEvent,
    },
    Click {
        x: f32,
        y: f32,
        kind: ClickKind,
    },
    /// Press, move, and release across the next frames. Interface pixels.
    Drag {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
    },
    Key {
        code: KeyCode,
        alt: bool,
        ctrl: bool,
        shift: bool,
    },
    Dump(DumpPath),
    Quit,
    Log {
        ok: bool,
        line: String,
    },
    Ready(String),
    /// PIN is stored on the autopilot. G02 calls
    /// [`Autopilot::take_pending_pin`].
    PinArmed,
}

impl Effect {
    pub fn log_line(&self) -> Option<String> {
        match self {
            Self::Log { ok, line } => Some(format!("AUTOPILOT_CMD {} {line}", if *ok { "ok" } else { "err" })),
            Self::Ready(timing) => Some(timing.clone()),
            Self::PinArmed => Some("AUTOPILOT_PIN_HOOK armed".to_owned()),
            _ => None,
        }
    }
}

#[derive(Deserialize)]
struct AccountFile {
    username: String,
    password: String,
    login_address: String,
}

pub struct AutopilotConfig {
    pub username: String,
    pub password: String,
    pub login_host: String,
    pub login_port: u16,
    pub server_name: Option<String>,
    pub slot: usize,
    pub character_name: String,
    pub register: bool,
    pub pin: Option<String>,
    pub commands_path: Option<PathBuf>,
}

pub struct Autopilot {
    username: String,
    password: String,
    login_host: String,
    login_port: u16,
    server_name: Option<String>,
    slot: usize,
    character_name: String,
    register: bool,
    pin: Option<String>,
    commands_path: Option<PathBuf>,
    command_file: Option<CommandFile>,
    queued_lines: VecDeque<String>,
    phase: Phase,
    started: Option<Instant>,
    login_ok_ms: Option<u128>,
    server_selected_ms: Option<u128>,
    char_selected_ms: Option<u128>,
    map_loaded_ms: Option<u128>,
    wait_until: Option<Instant>,
    ready_sent: bool,
}

struct CommandFile {
    path: PathBuf,
    file: File,
    pending: String,
    fifo: bool,
    reopen: bool,
}

impl Autopilot {
    pub fn from_env() -> Option<Self> {
        let path = std::env::var_os("KORANGAR_AUTOPILOT")?;
        match config_from_path(Path::new(&path)) {
            Ok(config) => Some(Self::new(config)),
            Err(error) => {
                eprintln!("autopilot disabled: {error}");
                None
            }
        }
    }

    pub fn new(config: AutopilotConfig) -> Self {
        Self {
            username: config.username,
            password: config.password,
            login_host: config.login_host,
            login_port: config.login_port,
            server_name: config.server_name,
            slot: config.slot,
            character_name: config.character_name,
            register: config.register,
            pin: config.pin,
            commands_path: config.commands_path,
            command_file: None,
            queued_lines: VecDeque::new(),
            phase: Phase::Idle,
            started: None,
            login_ok_ms: None,
            server_selected_ms: None,
            char_selected_ms: None,
            map_loaded_ms: None,
            wait_until: None,
            ready_sent: false,
        }
    }

    pub fn slot(&self) -> usize {
        self.slot
    }

    pub fn login_host(&self) -> &str {
        &self.login_host
    }

    pub fn login_port(&self) -> u16 {
        self.login_port
    }

    pub fn password(&self) -> &str {
        &self.password
    }

    /// Username sent to the login server. `KORANGAR_AUTOPILOT_REGISTER=1`
    /// appends `_M` so Hercules `new_account` creates the account. The
    /// suffix is only for that first login.
    pub fn login_username(&self) -> String {
        let already_suffixed = self.username.ends_with("_M")
            || self.username.ends_with("_F")
            || self.username.ends_with("_m")
            || self.username.ends_with("_f");
        if self.register && !already_suffixed {
            format!("{}_M", self.username)
        } else {
            self.username.clone()
        }
    }

    /// One-shot PIN code for G02. Never logged. The client does not send a
    /// PIN packet yet, so this stays unused outside the unit test and G02.
    #[allow(dead_code)]
    pub fn take_pending_pin(&mut self) -> Option<String> {
        self.pin.take()
    }

    pub fn on_window_shown(&mut self, now: Instant) -> Vec<Effect> {
        if self.phase != Phase::Idle {
            return Vec::new();
        }
        self.started = Some(now);
        self.phase = Phase::AwaitingLogin;
        vec![Effect::Login]
    }

    pub fn on_login_ok(&mut self, now: Instant, server_names: &[String]) -> Vec<Effect> {
        if self.phase != Phase::AwaitingLogin {
            return Vec::new();
        }
        self.login_ok_ms = Some(self.elapsed_ms(now));
        let Some(index) = pick_server(server_names, self.server_name.as_deref()) else {
            self.phase = Phase::Failed;
            let wanted = self.server_name.clone().unwrap_or_default();
            return vec![Effect::Log {
                ok: false,
                line: format!("no character server matched {wanted:?} among {server_names:?}"),
            }];
        };
        self.server_selected_ms = Some(self.elapsed_ms(now));
        self.phase = Phase::AwaitingCharacters;
        vec![Effect::SelectServer(index)]
    }

    pub fn on_login_failed(&mut self, message: &str) -> Vec<Effect> {
        if self.phase != Phase::AwaitingLogin && self.phase != Phase::Idle {
            return Vec::new();
        }
        if message.to_ascii_lowercase().contains("pin") {
            self.phase = Phase::AwaitingPin;
            if self.pin.is_some() {
                return vec![Effect::PinArmed];
            }
            return vec![Effect::Log {
                ok: false,
                line: "pin challenge (set KORANGAR_AUTOPILOT_PIN; G02 calls take_pending_pin)".to_owned(),
            }];
        }
        self.phase = Phase::Failed;
        vec![Effect::Log {
            ok: false,
            line: format!("login failed: {}", self.redact(message)),
        }]
    }

    pub fn on_characters(&mut self, now: Instant, slot_occupied: bool) -> Vec<Effect> {
        match self.phase {
            Phase::AwaitingCharacters | Phase::Creating if slot_occupied => self.select_character(now),
            Phase::AwaitingCharacters => {
                self.phase = Phase::Creating;
                vec![Effect::CreateCharacter {
                    slot: self.slot,
                    name: self.character_name.clone(),
                    // Style 2 and color 3 are off the old name-only defaults (style 0, color 0).
                    hair_color: 3,
                    hair_style: 2,
                    start_job: 0,
                    sex: 1,
                }]
            }
            _ => Vec::new(),
        }
    }

    pub fn on_character_created(&mut self, now: Instant) -> Vec<Effect> {
        if self.phase != Phase::Creating {
            return Vec::new();
        }
        self.select_character(now)
    }

    pub fn on_character_creation_failed(&mut self, message: &str) -> Vec<Effect> {
        if self.phase != Phase::Creating {
            return Vec::new();
        }
        self.phase = Phase::Failed;
        vec![Effect::Log {
            ok: false,
            line: format!("character creation failed: {}", self.redact(message)),
        }]
    }

    pub fn on_map_loaded(&mut self, now: Instant) -> Vec<Effect> {
        if self.ready_sent || !matches!(self.phase, Phase::EnteringMap | Phase::InWorld) {
            return Vec::new();
        }
        self.map_loaded_ms = Some(self.elapsed_ms(now));
        self.phase = Phase::InWorld;
        self.ready_sent = true;
        self.open_commands();
        vec![Effect::Ready(self.timing_line())]
    }

    pub fn poll(&mut self, now: Instant) -> Vec<Effect> {
        if self.phase != Phase::InWorld {
            return Vec::new();
        }
        if let Some(until) = self.wait_until {
            if now < until {
                return Vec::new();
            }
            self.wait_until = None;
        }
        self.pull_commands();
        let mut effects = Vec::new();
        while let Some(line) = self.queued_lines.pop_front() {
            let redacted = self.redact(&line);
            match parse_command(&line) {
                Ok(command) => {
                    let stop = self.enqueue_command(command, &redacted, now, &mut effects);
                    if stop {
                        break;
                    }
                }
                Err(error) => effects.push(Effect::Log {
                    ok: false,
                    line: format!("{redacted}: {error}"),
                }),
            }
        }
        effects
    }

    /// Test helper. Lines are parsed on the next [`Self::poll`] after map load.
    #[cfg(test)]
    fn inject_line(&mut self, line: &str) {
        self.queued_lines.push_back(line.to_owned());
    }

    fn select_character(&mut self, now: Instant) -> Vec<Effect> {
        self.char_selected_ms = Some(self.elapsed_ms(now));
        self.phase = Phase::EnteringMap;
        vec![Effect::SelectCharacter(self.slot)]
    }

    fn enqueue_command(&mut self, command: Command, line: &str, now: Instant, effects: &mut Vec<Effect>) -> bool {
        match command {
            Command::Chat(text) => {
                effects.push(log_ok(line));
                effects.push(Effect::Input(InputEvent::SendMessage { text }));
                false
            }
            Command::Window { class, open, event } => {
                effects.push(log_ok(line));
                effects.push(Effect::Window { class, open, event });
                false
            }
            Command::Toggle(event) | Command::Event(event) => {
                effects.push(log_ok(line));
                effects.push(Effect::Input(event));
                false
            }
            Command::Wait(ms) => {
                effects.push(log_ok(line));
                self.wait_until = Some(now + Duration::from_millis(ms));
                true
            }
            Command::Move { x, y } => {
                effects.push(log_ok(line));
                effects.push(Effect::Input(InputEvent::PlayerMove {
                    destination: TilePosition { x, y },
                }));
                false
            }
            Command::Click { x, y, kind } => {
                effects.push(log_ok(line));
                effects.push(Effect::Click { x, y, kind });
                true
            }
            Command::Drag { x0, y0, x1, y1 } => {
                effects.push(log_ok(line));
                effects.push(Effect::Drag { x0, y0, x1, y1 });
                true
            }
            Command::Key { code, alt, ctrl, shift } => {
                effects.push(log_ok(line));
                effects.push(Effect::Key { code, alt, ctrl, shift });
                true
            }
            Command::Dump(path) => {
                effects.push(log_ok(line));
                effects.push(Effect::Dump(path));
                false
            }
            Command::Quit => {
                effects.push(log_ok(line));
                effects.push(Effect::Quit);
                self.phase = Phase::Quit;
                true
            }
        }
    }

    fn open_commands(&mut self) {
        let Some(path) = self.commands_path.clone() else {
            return;
        };
        match open_command_file(&path) {
            Ok(file) => self.command_file = Some(file),
            Err(_) => {
                // The fifo or file may appear after map load. [`pull_commands`]
                // retries.
            }
        }
    }

    fn pull_commands(&mut self) {
        if self.command_file.is_none()
            && let Some(path) = self.commands_path.clone()
            && let Ok(file) = open_command_file(&path)
        {
            self.command_file = Some(file);
        }
        let mut lines = Vec::new();
        if let Some(source) = self.command_file.as_mut() {
            let mut buffer = [0_u8; 4096];
            loop {
                match source.file.read(&mut buffer) {
                    Ok(0) => {
                        if source.fifo {
                            source.reopen = true;
                        }
                        break;
                    }
                    Ok(count) => {
                        let text = String::from_utf8_lossy(&buffer[..count]);
                        source.pending.push_str(&text);
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                    Err(_) => break,
                }
            }
            if source.reopen {
                let path = source.path.clone();
                if let Ok(reopened) = open_command_file(&path) {
                    source.file = reopened.file;
                    source.reopen = false;
                }
            }
            while let Some(index) = source.pending.find('\n') {
                let line: String = source.pending.drain(..=index).collect();
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                lines.push(line.to_owned());
            }
        }
        self.queued_lines.extend(lines);
    }

    fn elapsed_ms(&self, now: Instant) -> u128 {
        self.started
            .map(|start| now.saturating_duration_since(start).as_millis())
            .unwrap_or(0)
    }

    fn timing_line(&self) -> String {
        format!(
            "AUTOPILOT_TIMING login_ok={}ms server_selected={}ms char_selected={}ms map_loaded={}ms",
            stamp(self.login_ok_ms),
            stamp(self.server_selected_ms),
            stamp(self.char_selected_ms),
            stamp(self.map_loaded_ms),
        )
    }

    fn redact(&self, text: &str) -> String {
        let mut text = text.to_owned();
        if !self.password.is_empty() {
            text = text.replace(&self.password, "[redacted]");
        }
        if let Some(pin) = &self.pin
            && !pin.is_empty()
        {
            text = text.replace(pin, "[redacted]");
        }
        text
    }
}

impl std::fmt::Debug for Autopilot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Autopilot")
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .field("pin", &self.pin.as_ref().map(|_| "<redacted>"))
            .field("login_host", &self.login_host)
            .field("login_port", &self.login_port)
            .field("slot", &self.slot)
            .field("phase", &self.phase)
            .finish()
    }
}

fn log_ok(line: &str) -> Effect {
    Effect::Log {
        ok: true,
        line: line.to_owned(),
    }
}

fn stamp(value: Option<u128>) -> String {
    value.map(|ms| ms.to_string()).unwrap_or_else(|| "na".to_owned())
}

fn pick_server(server_names: &[String], wanted: Option<&str>) -> Option<usize> {
    let cleaned: Vec<String> = server_names.iter().map(|name| clean_server_name(name)).collect();
    let Some(wanted) = wanted.map(str::trim).filter(|name| !name.is_empty()) else {
        return (!cleaned.is_empty()).then_some(0);
    };
    if let Some(index) = cleaned.iter().position(|name| name.eq_ignore_ascii_case(wanted)) {
        return Some(index);
    }
    cleaned
        .iter()
        .position(|name| name.to_ascii_lowercase().contains(&wanted.to_ascii_lowercase()))
}

fn clean_server_name(name: &str) -> String {
    name.trim_matches(|character: char| character.is_whitespace() || character == '\0')
        .to_owned()
}

fn open_command_file(path: &Path) -> io::Result<CommandFile> {
    let fifo = path.metadata().is_ok_and(|metadata| metadata.file_type().is_fifo());
    let file = std::fs::OpenOptions::new().read(true).custom_flags(O_NONBLOCK).open(path)?;
    Ok(CommandFile {
        path: path.to_path_buf(),
        file,
        pending: String::new(),
        fifo,
        reopen: false,
    })
}

fn config_from_path(path: &Path) -> Result<AutopilotConfig, String> {
    let account = load_account(path)?;
    let (login_host, login_port) = parse_login_address(&account.login_address)?;
    let server_name = optional_env("KORANGAR_AUTOPILOT_SERVER");
    let slot = std::env::var("KORANGAR_AUTOPILOT_SLOT")
        .ok()
        .map(|value| {
            value
                .parse::<usize>()
                .map_err(|_| "KORANGAR_AUTOPILOT_SLOT is not a number".to_owned())
        })
        .transpose()?
        .unwrap_or(0);
    let character_name = optional_env("KORANGAR_AUTOPILOT_CHAR").unwrap_or_else(|| default_character_name(&account.username));
    let register = flag_env("KORANGAR_AUTOPILOT_REGISTER");
    let pin = optional_env("KORANGAR_AUTOPILOT_PIN");
    let commands_path = std::env::var_os("KORANGAR_AUTOPILOT_CMDS").map(PathBuf::from);
    Ok(AutopilotConfig {
        username: account.username,
        password: account.password,
        login_host,
        login_port,
        server_name,
        slot,
        character_name,
        register,
        pin,
        commands_path,
    })
}

fn load_account(path: &Path) -> Result<AccountFile, String> {
    let text = std::fs::read_to_string(path).map_err(|_| "cannot read KORANGAR_AUTOPILOT account file".to_owned())?;
    let account: AccountFile = serde_json::from_str(&text)
        .map_err(|_| "KORANGAR_AUTOPILOT account file must be JSON with username, password, and login_address".to_owned())?;
    if account.username.is_empty() || account.password.is_empty() || account.login_address.is_empty() {
        return Err("autopilot account is missing username, password, or login_address".to_owned());
    }
    Ok(account)
}

fn parse_login_address(value: &str) -> Result<(String, u16), String> {
    let (host, port) = value.trim().rsplit_once(':').ok_or("login_address must be host:port")?;
    if host.is_empty() {
        return Err("login_address is missing a host".to_owned());
    }
    let port = port.parse::<u16>().map_err(|_| "login_address has a bad port".to_owned())?;
    Ok((host.to_owned(), port))
}

fn optional_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn flag_env(name: &str) -> bool {
    match std::env::var(name) {
        Ok(value) => {
            let value = value.trim();
            !(value.is_empty() || value == "0" || value.eq_ignore_ascii_case("false"))
        }
        Err(_) => false,
    }
}

pub fn default_character_name(username: &str) -> String {
    let mut name: String = username.chars().filter(|character| character.is_ascii_alphanumeric()).collect();
    if name.len() < 4 {
        name.push_str("hero");
    }
    name.truncate(23);
    name
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{Autopilot, AutopilotConfig, Effect, Phase, default_character_name, pick_server};
    use crate::input::InputEvent;

    fn config() -> AutopilotConfig {
        AutopilotConfig {
            username: "kgap01".to_owned(),
            password: "s3cret-value".to_owned(),
            login_host: "192.168.11.110".to_owned(),
            login_port: 6910,
            server_name: Some("TuroranRO-TEST".to_owned()),
            slot: 0,
            character_name: "kgap01".to_owned(),
            register: true,
            pin: Some("pin-secret".to_owned()),
            commands_path: None,
        }
    }

    fn reach_characters(auto: &mut Autopilot, now: Instant) {
        let login = auto.on_window_shown(now);
        assert!(matches!(login.first(), Some(Effect::Login)));
        assert_eq!(auto.login_username(), "kgap01_M");
        let servers = vec!["  Other\0".to_owned(), "TuroranRO-TEST".to_owned()];
        let selected = auto.on_login_ok(now, &servers);
        assert!(matches!(selected.as_slice(), [Effect::SelectServer(1)]));
    }

    #[test]
    fn debug_redacts_secrets() {
        let auto = Autopilot::new(config());
        let rendered = format!("{auto:?}");
        assert!(!rendered.contains("s3cret-value"));
        assert!(!rendered.contains("pin-secret"));
        assert!(rendered.contains("<redacted>"));
    }

    #[test]
    fn empty_slot_creates_then_selects() {
        let mut auto = Autopilot::new(config());
        let now = Instant::now();
        reach_characters(&mut auto, now);
        let created = auto.on_characters(now, false);
        assert!(matches!(
            &created[0],
            Effect::CreateCharacter {
                slot: 0,
                name,
                hair_color: 3,
                hair_style: 2,
                start_job: 0,
                sex: 1,
            } if name == "kgap01"
        ));
        let selected = auto.on_character_created(now);
        assert!(matches!(selected.as_slice(), [Effect::SelectCharacter(0)]));
        let ready = auto.on_map_loaded(now);
        let Effect::Ready(line) = &ready[0] else {
            panic!("expected timing");
        };
        assert!(line.starts_with("AUTOPILOT_TIMING login_ok="));
        assert!(line.contains("map_loaded="));
        assert!(!line.contains("na"));
    }

    #[test]
    fn occupied_slot_skips_creation() {
        let mut auto = Autopilot::new(config());
        let now = Instant::now();
        reach_characters(&mut auto, now);
        let selected = auto.on_characters(now, true);
        assert!(matches!(selected.as_slice(), [Effect::SelectCharacter(0)]));
        assert!(auto.on_character_created(now).is_empty());
    }

    #[test]
    fn first_server_when_unnamed() {
        assert_eq!(pick_server(&["Alpha".into(), "Beta".into()], None), Some(0));
        assert_eq!(pick_server(&["Alpha".into()], Some("nope")), None);
    }

    #[test]
    fn commands_run_after_map_load_and_wait() {
        let mut auto = Autopilot::new(config());
        let now = Instant::now();
        reach_characters(&mut auto, now);
        auto.on_characters(now, true);
        auto.on_map_loaded(now);
        auto.inject_line("chat @where");
        auto.inject_line("wait 1000");
        auto.inject_line("open Inventory");
        let first = auto.poll(now);
        assert!(
            first
                .iter()
                .any(|effect| matches!(effect, Effect::Input(InputEvent::SendMessage { .. })))
        );
        assert!(auto.poll(now).is_empty());
        let later = auto.poll(now + Duration::from_millis(1000));
        assert!(later.iter().any(|effect| matches!(effect, Effect::Window { .. })));
    }

    #[test]
    fn pin_challenge_keeps_code_unlogged() {
        let mut auto = Autopilot::new(config());
        let now = Instant::now();
        auto.on_window_shown(now);
        let effects = auto.on_login_failed("Enter your PIN code");
        assert!(matches!(effects.as_slice(), [Effect::PinArmed]));
        let rendered = format!("{effects:?}");
        assert!(!rendered.contains("pin-secret"));
        assert_eq!(auto.take_pending_pin().as_deref(), Some("pin-secret"));
        assert!(auto.take_pending_pin().is_none());
        assert_eq!(auto.phase, Phase::AwaitingPin);
    }

    #[test]
    fn character_name_fallback_is_alphanumeric() {
        assert_eq!(default_character_name("ab"), "abhero");
        assert_eq!(default_character_name("kgap-01"), "kgap01");
    }

    #[test]
    fn command_file_lines_are_read_without_blocking() {
        let path = std::env::temp_dir().join(format!("korangar-autopilot-test-{}", std::process::id()));
        std::fs::write(&path, "chat hello\n# comment\nquit\n").unwrap();
        let mut auto = Autopilot::new(AutopilotConfig {
            commands_path: Some(path.clone()),
            ..config()
        });
        let now = Instant::now();
        reach_characters(&mut auto, now);
        auto.on_characters(now, true);
        let ready = auto.on_map_loaded(now);
        assert!(matches!(ready.as_slice(), [Effect::Ready(_)]));
        let effects = auto.poll(now);
        assert!(effects.iter().any(|effect| matches!(effect, Effect::Quit)));
        let _ = std::fs::remove_file(path);
    }
}
