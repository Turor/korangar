//! Keyboard shortcuts, modeled on roBrowser `ShortCutControls` /
//! `ShortCutOption`.
//!
//! Defaults follow the official bar: F1–F9 are hotbar row 1, Alt+F1–F9 row 2,
//! Ctrl+F1–F9 row 3, Shift+F1–F9 row 4. Window toggles use the roBrowser chords
//! (Alt+E inventory, Alt+Q equip, Alt+S skills, Alt+A stats, Alt+Z friends,
//! Alt+G guild, Insert sit). Escape is never a bind. The existing Ctrl+debug
//! keys stay hard-coded in the input system and are not listed here.

#[cfg(feature = "debug")]
use korangar_debug::logging::{Colorize, print_debug};
use korangar_interface::element::StateElement;
use ron::ser::PrettyConfig;
use rust_state::RustState;
use serde::{Deserialize, Serialize};
use winit::keyboard::KeyCode;

use crate::input::InputEvent;
use crate::state::hotbar::{HOTBAR_COLUMNS, stored_slot};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyName {
    Unbound,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Insert,
}

impl KeyName {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Unbound => "unbound",
            Self::F1 => "F1",
            Self::F2 => "F2",
            Self::F3 => "F3",
            Self::F4 => "F4",
            Self::F5 => "F5",
            Self::F6 => "F6",
            Self::F7 => "F7",
            Self::F8 => "F8",
            Self::F9 => "F9",
            Self::F10 => "F10",
            Self::F11 => "F11",
            Self::F12 => "F12",
            Self::Digit0 => "0",
            Self::Digit1 => "1",
            Self::Digit2 => "2",
            Self::Digit3 => "3",
            Self::Digit4 => "4",
            Self::Digit5 => "5",
            Self::Digit6 => "6",
            Self::Digit7 => "7",
            Self::Digit8 => "8",
            Self::Digit9 => "9",
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
            Self::E => "E",
            Self::F => "F",
            Self::G => "G",
            Self::H => "H",
            Self::I => "I",
            Self::J => "J",
            Self::K => "K",
            Self::L => "L",
            Self::M => "M",
            Self::N => "N",
            Self::O => "O",
            Self::P => "P",
            Self::Q => "Q",
            Self::R => "R",
            Self::S => "S",
            Self::T => "T",
            Self::U => "U",
            Self::V => "V",
            Self::W => "W",
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
            Self::Insert => "Insert",
        }
    }

    pub const fn key_code(self) -> Option<KeyCode> {
        Some(match self {
            Self::Unbound => return None,
            Self::F1 => KeyCode::F1,
            Self::F2 => KeyCode::F2,
            Self::F3 => KeyCode::F3,
            Self::F4 => KeyCode::F4,
            Self::F5 => KeyCode::F5,
            Self::F6 => KeyCode::F6,
            Self::F7 => KeyCode::F7,
            Self::F8 => KeyCode::F8,
            Self::F9 => KeyCode::F9,
            Self::F10 => KeyCode::F10,
            Self::F11 => KeyCode::F11,
            Self::F12 => KeyCode::F12,
            Self::Digit0 => KeyCode::Digit0,
            Self::Digit1 => KeyCode::Digit1,
            Self::Digit2 => KeyCode::Digit2,
            Self::Digit3 => KeyCode::Digit3,
            Self::Digit4 => KeyCode::Digit4,
            Self::Digit5 => KeyCode::Digit5,
            Self::Digit6 => KeyCode::Digit6,
            Self::Digit7 => KeyCode::Digit7,
            Self::Digit8 => KeyCode::Digit8,
            Self::Digit9 => KeyCode::Digit9,
            Self::A => KeyCode::KeyA,
            Self::B => KeyCode::KeyB,
            Self::C => KeyCode::KeyC,
            Self::D => KeyCode::KeyD,
            Self::E => KeyCode::KeyE,
            Self::F => KeyCode::KeyF,
            Self::G => KeyCode::KeyG,
            Self::H => KeyCode::KeyH,
            Self::I => KeyCode::KeyI,
            Self::J => KeyCode::KeyJ,
            Self::K => KeyCode::KeyK,
            Self::L => KeyCode::KeyL,
            Self::M => KeyCode::KeyM,
            Self::N => KeyCode::KeyN,
            Self::O => KeyCode::KeyO,
            Self::P => KeyCode::KeyP,
            Self::Q => KeyCode::KeyQ,
            Self::R => KeyCode::KeyR,
            Self::S => KeyCode::KeyS,
            Self::T => KeyCode::KeyT,
            Self::U => KeyCode::KeyU,
            Self::V => KeyCode::KeyV,
            Self::W => KeyCode::KeyW,
            Self::X => KeyCode::KeyX,
            Self::Y => KeyCode::KeyY,
            Self::Z => KeyCode::KeyZ,
            Self::Insert => KeyCode::Insert,
        })
    }
}

pub const BINDABLE_KEYS: [KeyName; 49] = [
    KeyName::F1,
    KeyName::F2,
    KeyName::F3,
    KeyName::F4,
    KeyName::F5,
    KeyName::F6,
    KeyName::F7,
    KeyName::F8,
    KeyName::F9,
    KeyName::F10,
    KeyName::F11,
    KeyName::F12,
    KeyName::Digit0,
    KeyName::Digit1,
    KeyName::Digit2,
    KeyName::Digit3,
    KeyName::Digit4,
    KeyName::Digit5,
    KeyName::Digit6,
    KeyName::Digit7,
    KeyName::Digit8,
    KeyName::Digit9,
    KeyName::A,
    KeyName::B,
    KeyName::C,
    KeyName::D,
    KeyName::E,
    KeyName::F,
    KeyName::G,
    KeyName::H,
    KeyName::I,
    KeyName::J,
    KeyName::K,
    KeyName::L,
    KeyName::M,
    KeyName::N,
    KeyName::O,
    KeyName::P,
    KeyName::Q,
    KeyName::R,
    KeyName::S,
    KeyName::T,
    KeyName::U,
    KeyName::V,
    KeyName::W,
    KeyName::X,
    KeyName::Y,
    KeyName::Z,
    KeyName::Insert,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chord {
    pub key: KeyName,
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
}

impl Chord {
    pub const fn unbound() -> Self {
        Self {
            key: KeyName::Unbound,
            alt: false,
            ctrl: false,
            shift: false,
        }
    }

    pub const fn new(key: KeyName, alt: bool, ctrl: bool, shift: bool) -> Self {
        Self { key, alt, ctrl, shift }
    }

    pub fn label(self) -> String {
        if self.key == KeyName::Unbound {
            return "unbound".to_owned();
        }
        let mut text = String::new();
        if self.alt {
            text.push_str("Alt+");
        }
        if self.ctrl {
            text.push_str("Ctrl+");
        }
        if self.shift {
            text.push_str("Shift+");
        }
        text.push_str(self.key.label());
        text
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionKind {
    /// Visual hotbar slot 0..35. Row shift is applied when the key is pressed.
    Hotbar(u16),
    /// Quick slot 36 or 37. Row shift does not move these.
    Quick(u16),
    Inventory,
    Equipment,
    Skills,
    Stats,
    Friends,
    Guild,
    Clan,
    ChatRoom,
    ItemFinder,
    Cart,
    GameSettings,
    InterfaceSettings,
    GraphicsSettings,
    AudioSettings,
    ShowInterface,
    CloseTop,
    Sit,
    CycleRows,
    RotateRow,
    SwitchTab,
    Keybinds,
    Emotion(u8),
}

#[derive(Clone, Debug)]
struct ActionDef {
    id: String,
    title: String,
    chord: Chord,
    kind: ActionKind,
}

fn default_actions() -> Vec<ActionDef> {
    const F_KEYS: [KeyName; 9] = [
        KeyName::F1,
        KeyName::F2,
        KeyName::F3,
        KeyName::F4,
        KeyName::F5,
        KeyName::F6,
        KeyName::F7,
        KeyName::F8,
        KeyName::F9,
    ];
    const ROW_MODS: [(bool, bool, bool); 4] = [
        (false, false, false),
        (true, false, false),
        (false, true, false),
        (false, false, true),
    ];

    let mut actions = Vec::with_capacity(80);
    for (row, (alt, ctrl, shift)) in ROW_MODS.into_iter().enumerate() {
        for (column, key) in F_KEYS.into_iter().enumerate() {
            let visual = (row * HOTBAR_COLUMNS + column) as u16;
            actions.push(ActionDef {
                id: format!("hotbar-{visual}"),
                title: format!("Hotbar {}-{}", row + 1, column + 1),
                chord: Chord::new(key, alt, ctrl, shift),
                kind: ActionKind::Hotbar(visual),
            });
        }
    }
    for quick in 0..2u16 {
        let slot = 36 + quick;
        actions.push(ActionDef {
            id: format!("quick-{slot}"),
            title: format!("Quick slot {}", quick + 1),
            chord: Chord::unbound(),
            kind: ActionKind::Quick(slot),
        });
    }

    let windows = [
        ("inventory", "Inventory", KeyName::E, true, false, false, ActionKind::Inventory),
        ("equipment", "Equipment", KeyName::Q, true, false, false, ActionKind::Equipment),
        ("skills", "Skills", KeyName::S, true, false, false, ActionKind::Skills),
        ("stats", "Stats", KeyName::A, true, false, false, ActionKind::Stats),
        (
            "friends",
            "Party / friends",
            KeyName::Z,
            true,
            false,
            false,
            ActionKind::Friends,
        ),
        ("guild", "Guild", KeyName::G, true, false, false, ActionKind::Guild),
        ("clan", "Clan", KeyName::C, true, false, false, ActionKind::Clan),
        ("chat-room", "Chat room", KeyName::C, true, false, true, ActionKind::ChatRoom),
        (
            "item-finder",
            "Item finder",
            KeyName::F,
            true,
            false,
            false,
            ActionKind::ItemFinder,
        ),
        ("cart", "Cart", KeyName::W, true, false, false, ActionKind::Cart),
        (
            "game-settings",
            "Game settings",
            KeyName::S,
            false,
            true,
            false,
            ActionKind::GameSettings,
        ),
        (
            "interface-settings",
            "Interface settings",
            KeyName::I,
            false,
            true,
            false,
            ActionKind::InterfaceSettings,
        ),
        (
            "graphics-settings",
            "Graphics settings",
            KeyName::G,
            false,
            true,
            false,
            ActionKind::GraphicsSettings,
        ),
        (
            "audio-settings",
            "Audio settings",
            KeyName::A,
            false,
            true,
            false,
            ActionKind::AudioSettings,
        ),
        (
            "show-interface",
            "Show interface",
            KeyName::H,
            false,
            true,
            false,
            ActionKind::ShowInterface,
        ),
        (
            "close-top",
            "Close top window",
            KeyName::Q,
            false,
            true,
            false,
            ActionKind::CloseTop,
        ),
        ("sit", "Sit / stand", KeyName::Insert, false, false, false, ActionKind::Sit),
        (
            "cycle-rows",
            "Hotbar rows",
            KeyName::F12,
            false,
            false,
            false,
            ActionKind::CycleRows,
        ),
        (
            "rotate-row",
            "Shift hotbar row",
            KeyName::Unbound,
            false,
            false,
            false,
            ActionKind::RotateRow,
        ),
        (
            "switch-tab",
            "Switch hotbar tab",
            KeyName::Unbound,
            false,
            false,
            false,
            ActionKind::SwitchTab,
        ),
        ("keybinds", "Keybinds", KeyName::M, true, false, false, ActionKind::Keybinds),
    ];
    for (id, title, key, alt, ctrl, shift, kind) in windows {
        actions.push(ActionDef {
            id: id.to_owned(),
            title: title.to_owned(),
            chord: Chord::new(key, alt, ctrl, shift),
            kind,
        });
    }
    for emotion in 0..9u8 {
        let key = BINDABLE_KEYS[12 + 1 + emotion as usize];
        actions.push(ActionDef {
            id: format!("emotion-{emotion}"),
            title: format!("Emotion {}", emotion + 1),
            chord: Chord::new(key, true, false, false),
            kind: ActionKind::Emotion(emotion),
        });
    }
    actions
}

#[derive(Clone, Debug)]
pub struct KeybindMap {
    actions: Vec<ActionDef>,
}

impl Default for KeybindMap {
    fn default() -> Self {
        Self {
            actions: default_actions(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SavedBind {
    action: String,
    key: KeyName,
    alt: bool,
    ctrl: bool,
    shift: bool,
}

impl KeybindMap {
    pub fn len(&self) -> usize {
        self.actions.len()
    }

    pub fn chord(&self, index: usize) -> Chord {
        self.actions.get(index).map(|action| action.chord).unwrap_or_else(Chord::unbound)
    }

    pub fn editor_label(&self, index: usize) -> String {
        match self.actions.get(index) {
            Some(action) => format!("{} — {}", action.title, action.chord.label()),
            None => String::new(),
        }
    }

    pub fn rebind(&mut self, index: usize, chord: Chord) {
        if index >= self.actions.len() {
            return;
        }
        if chord.key != KeyName::Unbound {
            for (other, action) in self.actions.iter_mut().enumerate() {
                if other != index && action.chord == chord {
                    action.chord = Chord::unbound();
                }
            }
        }
        self.actions[index].chord = chord;
    }

    pub fn to_ron(&self) -> String {
        let saved = self
            .actions
            .iter()
            .map(|action| SavedBind {
                action: action.id.clone(),
                key: action.chord.key,
                alt: action.chord.alt,
                ctrl: action.chord.ctrl,
                shift: action.chord.shift,
            })
            .collect::<Vec<_>>();
        ron::ser::to_string_pretty(&saved, PrettyConfig::new()).unwrap_or_else(|_| "[]".to_owned())
    }

    pub fn from_ron(text: &str) -> Self {
        let mut map = Self::default();
        let Ok(saved) = ron::from_str::<Vec<SavedBind>>(text) else {
            return map;
        };
        for bind in saved {
            let Some(index) = map.actions.iter().position(|action| action.id == bind.action) else {
                continue;
            };
            map.rebind(index, Chord::new(bind.key, bind.alt, bind.ctrl, bind.shift));
        }
        map
    }

    #[cfg(test)]
    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.actions.iter().position(|action| action.id == id)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct KeyEdges {
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub pressed: [bool; BINDABLE_KEYS.len()],
    pub released: [bool; BINDABLE_KEYS.len()],
    /// When the debug camera owns WASD, those keys do not fire binds.
    pub lock_movement_keys: bool,
}

#[derive(Clone, Debug)]
pub struct PollOutcome {
    pub events: Vec<InputEvent>,
    pub rebound: Option<(u16, Chord)>,
}

fn movement_key(key: KeyName) -> bool {
    matches!(key, KeyName::W | KeyName::A | KeyName::S | KeyName::D)
}

/// Build key edges from per-key press and release flags.
pub fn edges_from_transitions(
    alt: bool,
    ctrl: bool,
    shift: bool,
    lock_movement_keys: bool,
    mut transition: impl FnMut(KeyCode) -> (bool, bool),
) -> KeyEdges {
    let mut pressed = [false; BINDABLE_KEYS.len()];
    let mut released = [false; BINDABLE_KEYS.len()];
    for (index, key) in BINDABLE_KEYS.into_iter().enumerate() {
        if let Some(code) = key.key_code() {
            let (was_pressed, was_released) = transition(code);
            pressed[index] = was_pressed;
            released[index] = was_released;
        }
    }
    KeyEdges {
        alt,
        ctrl,
        shift,
        pressed,
        released,
        lock_movement_keys,
    }
}

impl KeybindMap {
    pub fn poll(&self, capturing: Option<u16>, active_tab: u8, row_shift: u8, edges: &KeyEdges) -> PollOutcome {
        if let Some(index) = capturing {
            for (key_index, key) in BINDABLE_KEYS.into_iter().enumerate() {
                if edges.pressed[key_index] {
                    return PollOutcome {
                        events: Vec::new(),
                        rebound: Some((index, Chord::new(key, edges.alt, edges.ctrl, edges.shift))),
                    };
                }
            }
            return PollOutcome {
                events: Vec::new(),
                rebound: None,
            };
        }

        let mut events = Vec::new();
        for action in &self.actions {
            let Some(code_index) = BINDABLE_KEYS.iter().position(|key| *key == action.chord.key) else {
                continue;
            };
            if edges.lock_movement_keys && movement_key(action.chord.key) {
                continue;
            }
            let mods = action.chord.alt == edges.alt && action.chord.ctrl == edges.ctrl && action.chord.shift == edges.shift;
            if !mods {
                continue;
            }
            if edges.pressed[code_index] {
                push_action(&mut events, action.kind, active_tab, row_shift, true);
            } else if edges.released[code_index] {
                push_action(&mut events, action.kind, active_tab, row_shift, false);
            }
        }
        PollOutcome { events, rebound: None }
    }
}

fn push_action(events: &mut Vec<InputEvent>, kind: ActionKind, active_tab: u8, row_shift: u8, pressed: bool) {
    use ragnarok_packets::{HotbarSlot, HotbarTab};
    let hotbar_event = |visual: u16, quick: bool| {
        let slot = if quick { visual } else { stored_slot(visual, row_shift) };
        if pressed {
            InputEvent::ActivateHotbar {
                tab: HotbarTab(u16::from(active_tab)),
                slot: HotbarSlot(slot),
            }
        } else {
            InputEvent::ReleaseHotbar {
                tab: HotbarTab(u16::from(active_tab)),
                slot: HotbarSlot(slot),
            }
        }
    };
    let event = match kind {
        ActionKind::Hotbar(visual) => Some(hotbar_event(visual, false)),
        ActionKind::Quick(slot) => Some(hotbar_event(slot, true)),
        ActionKind::Inventory if pressed => Some(InputEvent::ToggleInventoryWindow),
        ActionKind::Equipment if pressed => Some(InputEvent::ToggleEquipmentWindow),
        ActionKind::Skills if pressed => Some(InputEvent::ToggleSkillTreeWindow),
        ActionKind::Stats if pressed => Some(InputEvent::ToggleStatsWindow),
        ActionKind::Friends if pressed => Some(InputEvent::ToggleFriendListWindow),
        ActionKind::Guild if pressed => Some(InputEvent::ToggleGuildWindow),
        ActionKind::Clan if pressed => Some(InputEvent::ToggleClanWindow),
        ActionKind::ChatRoom if pressed => Some(InputEvent::OpenChatRoomCreate),
        ActionKind::ItemFinder if pressed => Some(InputEvent::ToggleItemFinderWindow),
        ActionKind::Cart if pressed => Some(InputEvent::ToggleCartWindow),
        ActionKind::GameSettings if pressed => Some(InputEvent::ToggleGameSettingsWindow),
        ActionKind::InterfaceSettings if pressed => Some(InputEvent::ToggleInterfaceSettingsWindow),
        ActionKind::GraphicsSettings if pressed => Some(InputEvent::ToggleGraphicsSettingsWindow),
        ActionKind::AudioSettings if pressed => Some(InputEvent::ToggleAudioSettingsWindow),
        ActionKind::ShowInterface if pressed => Some(InputEvent::ToggleShowInterface),
        ActionKind::CloseTop if pressed => Some(InputEvent::CloseTopWindow),
        ActionKind::Sit if pressed => Some(InputEvent::ToggleSit),
        ActionKind::CycleRows if pressed => Some(InputEvent::CycleHotbarRows),
        ActionKind::RotateRow if pressed => Some(InputEvent::RotateHotbar),
        ActionKind::SwitchTab if pressed => Some(InputEvent::SwitchHotbarTab),
        ActionKind::Keybinds if pressed => Some(InputEvent::ToggleKeybindsWindow),
        ActionKind::Emotion(emotion) if pressed => Some(InputEvent::SendEmotion { emotion }),
        _ => None,
    };
    if let Some(event) = event {
        events.push(event);
    }
}

#[derive(Clone, Debug, RustState, StateElement)]
pub struct KeybindRow {
    pub label: String,
    pub chord: String,
}

#[derive(RustState, StateElement)]
pub struct Keybinds {
    rows: Vec<KeybindRow>,
    capturing: Option<u16>,
    #[hidden_element]
    map: KeybindMap,
}

impl Default for Keybinds {
    fn default() -> Self {
        Self::from_map(KeybindMap::default())
    }
}

impl Keybinds {
    pub const FILE_NAME: &'static str = "client/keybinds.ron";

    pub fn new() -> Self {
        let map = std::fs::read_to_string(Self::FILE_NAME)
            .ok()
            .map(|text| KeybindMap::from_ron(&text))
            .unwrap_or_default();
        Self::from_map(map)
    }

    fn from_map(map: KeybindMap) -> Self {
        let mut keybinds = Self {
            rows: Vec::new(),
            capturing: None,
            map,
        };
        keybinds.refresh_rows();
        keybinds
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_capturing(&self) -> bool {
        self.capturing.is_some()
    }

    pub fn capturing(&self) -> Option<u16> {
        self.capturing
    }

    pub fn map(&self) -> &KeybindMap {
        &self.map
    }

    pub fn row_label(&self, index: usize) -> &str {
        self.rows.get(index).map(|row| row.label.as_str()).unwrap_or("")
    }

    pub fn begin_capture(&mut self, index: u16) {
        if (index as usize) < self.map.len() {
            self.capturing = Some(index);
            self.refresh_rows();
        }
    }

    pub fn cancel_capture(&mut self) {
        self.capturing = None;
        self.refresh_rows();
    }

    pub fn apply_rebind(&mut self, index: u16, chord: Chord) {
        self.map.rebind(index as usize, chord);
        self.capturing = None;
        self.refresh_rows();
        self.save();
    }

    pub fn reset_defaults(&mut self) {
        self.map = KeybindMap::default();
        self.capturing = None;
        self.refresh_rows();
        self.save();
    }

    fn refresh_rows(&mut self) {
        self.rows.clear();
        for index in 0..self.map.len() {
            let mut label = self.map.editor_label(index);
            if self.capturing == Some(index as u16) {
                label.push_str("  (press a key)");
            }
            self.rows.push(KeybindRow {
                label,
                chord: self.map.chord(index).label(),
            });
        }
    }

    pub fn save(&self) {
        #[cfg(feature = "debug")]
        print_debug!("saving keybinds to {}", Self::FILE_NAME.magenta());
        if let Err(_error) = std::fs::write(Self::FILE_NAME, self.map.to_ron()) {
            #[cfg(feature = "debug")]
            print_debug!("failed to save keybinds to {}: {:?}", Self::FILE_NAME.magenta(), _error.red());
        }
    }
}

impl Drop for Keybinds {
    fn drop(&mut self) {
        self.save();
    }
}

#[cfg(test)]
mod tests {
    use ragnarok_packets::HotbarSlot;

    use super::*;

    fn edges(pressed: Option<KeyName>, alt: bool, ctrl: bool, shift: bool) -> KeyEdges {
        let mut edges = KeyEdges {
            alt,
            ctrl,
            shift,
            pressed: [false; BINDABLE_KEYS.len()],
            released: [false; BINDABLE_KEYS.len()],
            lock_movement_keys: false,
        };
        if let Some(key) = pressed
            && let Some(index) = BINDABLE_KEYS.iter().position(|candidate| *candidate == key)
        {
            edges.pressed[index] = true;
        }
        edges
    }

    #[test]
    fn defaults_match_robrowser_chords_and_leave_escape_unbound() {
        let map = KeybindMap::default();
        assert_eq!(
            map.chord(map.index_of("hotbar-0").unwrap()),
            Chord::new(KeyName::F1, false, false, false)
        );
        assert_eq!(
            map.chord(map.index_of("hotbar-8").unwrap()),
            Chord::new(KeyName::F9, false, false, false)
        );
        assert_eq!(
            map.chord(map.index_of("hotbar-9").unwrap()),
            Chord::new(KeyName::F1, true, false, false)
        );
        assert_eq!(
            map.chord(map.index_of("hotbar-18").unwrap()),
            Chord::new(KeyName::F1, false, true, false)
        );
        assert_eq!(
            map.chord(map.index_of("hotbar-27").unwrap()),
            Chord::new(KeyName::F1, false, false, true)
        );
        assert_eq!(
            map.chord(map.index_of("inventory").unwrap()),
            Chord::new(KeyName::E, true, false, false)
        );
        assert_eq!(
            map.chord(map.index_of("equipment").unwrap()),
            Chord::new(KeyName::Q, true, false, false)
        );
        assert_eq!(
            map.chord(map.index_of("skills").unwrap()),
            Chord::new(KeyName::S, true, false, false)
        );
        assert_eq!(
            map.chord(map.index_of("stats").unwrap()),
            Chord::new(KeyName::A, true, false, false)
        );
        assert_eq!(
            map.chord(map.index_of("friends").unwrap()),
            Chord::new(KeyName::Z, true, false, false)
        );
        assert_eq!(
            map.chord(map.index_of("guild").unwrap()),
            Chord::new(KeyName::G, true, false, false)
        );
        assert_eq!(
            map.chord(map.index_of("sit").unwrap()),
            Chord::new(KeyName::Insert, false, false, false)
        );
        assert_eq!(
            map.chord(map.index_of("game-settings").unwrap()),
            Chord::new(KeyName::S, false, true, false)
        );
        assert!(map.actions.iter().all(|action| {
            action.chord.key != KeyName::Unbound || action.id.starts_with("quick") || action.id == "rotate-row" || action.id == "switch-tab"
        }));
        let escape_file = "[(action: \"sit\", key: Escape, alt: false, ctrl: false, shift: false)]";
        let rejected = KeybindMap::from_ron(escape_file);
        assert_eq!(rejected.chord(rejected.index_of("sit").unwrap()).key, KeyName::Insert);
    }

    #[test]
    fn ron_roundtrip_keeps_custom_chords_and_defaults() {
        let mut map = KeybindMap::default();
        let hotbar = map.index_of("hotbar-0").unwrap();
        map.rebind(hotbar, Chord::new(KeyName::A, true, false, false));
        let loaded = KeybindMap::from_ron(&map.to_ron());
        assert_eq!(loaded.chord(hotbar), Chord::new(KeyName::A, true, false, false));
        assert_eq!(loaded.chord(loaded.index_of("inventory").unwrap()).key, KeyName::E);
        assert_eq!(loaded.chord(loaded.index_of("sit").unwrap()).key, KeyName::Insert);
        let partial = KeybindMap::from_ron("[(action: \"guild\", key: H, alt: true, ctrl: false, shift: false)]");
        assert_eq!(
            partial.chord(partial.index_of("guild").unwrap()),
            Chord::new(KeyName::H, true, false, false)
        );
        assert_eq!(partial.chord(partial.index_of("inventory").unwrap()).key, KeyName::E);
    }

    #[test]
    fn poll_dispatches_hotbar_rows_sit_and_capture() {
        let map = KeybindMap::default();
        let f1 = map.poll(None, 0, 0, &edges(Some(KeyName::F1), false, false, false));
        assert!(matches!(f1.events.as_slice(), [InputEvent::ActivateHotbar {
            slot: HotbarSlot(0),
            ..
        }]));
        let alt_f1 = map.poll(None, 0, 0, &edges(Some(KeyName::F1), true, false, false));
        assert!(matches!(alt_f1.events.as_slice(), [InputEvent::ActivateHotbar {
            slot: HotbarSlot(9),
            ..
        }]));
        let shifted = map.poll(None, 0, 1, &edges(Some(KeyName::F1), false, false, false));
        assert!(matches!(shifted.events.as_slice(), [InputEvent::ActivateHotbar {
            slot: HotbarSlot(9),
            ..
        }]));
        let sit = map.poll(None, 0, 0, &edges(Some(KeyName::Insert), false, false, false));
        assert!(matches!(sit.events.as_slice(), [InputEvent::ToggleSit]));
        let inventory = map.poll(None, 0, 0, &edges(Some(KeyName::E), true, false, false));
        assert!(matches!(inventory.events.as_slice(), [InputEvent::ToggleInventoryWindow]));

        let capturing = map.poll(Some(0), 0, 0, &edges(Some(KeyName::F2), false, false, false));
        assert!(capturing.events.is_empty());
        assert_eq!(capturing.rebound, Some((0, Chord::new(KeyName::F2, false, false, false))));
    }
}
