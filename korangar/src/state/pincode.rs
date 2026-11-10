//! PIN window state. Packet bytes and the cipher live in `ragnarok_packets`.
//!
//! The steps follow roBrowser `CharEngine.onPincodeCheckSuccess` and
//! `PincodeWindow`: ask, create-then-confirm, change (old, new, confirm),
//! and the character-select button that sends `0x08C5`. Digits are hidden.
//! The pad order is the roBrowser keypad. Reset moves only the labels.

use korangar_interface::element::StateElement;
use korangar_networking::PincodeNotice;
use ragnarok_packets::{AccountId, advance_pincode_visual_seed, encrypt_pincode, pincode_keypad};
use rust_state::RustState;

const MODE_HIDDEN: u8 = 0;
const MODE_CHECK: u8 = 1;
const MODE_CREATE: u8 = 2;
const MODE_CHANGE: u8 = 3;
const MODE_BUTTON: u8 = 4;

const ENTRY_CURRENT: u8 = 0;
const ENTRY_NEW: u8 = 1;
const ENTRY_CONFIRM: u8 = 2;

const MAX_ATTEMPTS: u32 = 3;
const FLAG_LOCKED: u8 = 0;

/// What the client should send after a PIN action. Pins are encrypted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PincodeOutbound {
    Check {
        account_id: AccountId,
        pin: [u8; 4],
    },
    Make {
        account_id: AccountId,
        pin: [u8; 4],
    },
    Edit {
        account_id: AccountId,
        old_pin: [u8; 4],
        new_pin: [u8; 4],
    },
    Available {
        account_id: AccountId,
    },
}

/// UI and network side effects of one PIN step.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PincodeEffect {
    pub open_window: bool,
    pub close_window: bool,
    pub disconnect: bool,
    pub outbound: Option<PincodeOutbound>,
}

/// Mouse-only PIN pad. Character select stays blocked until the server says ok.
#[derive(Debug, RustState, StateElement)]
pub struct PincodeWindowState {
    pub prompt: String,
    pub masked: String,
    pub message: String,
    /// Label at each pad location. Location `n` shows `keypad[n]`.
    pub pad: Vec<String>,
    pub show_pad: bool,
    pub show_change: bool,
    /// Character select offers the PIN button while this is set.
    pub needs_pin: bool,
    /// Select and delete are sent only after a successful check.
    pub allows_select: bool,
    #[hidden_element]
    account_id: u32,
    #[hidden_element]
    seed: u32,
    #[hidden_element]
    mode: u8,
    #[hidden_element]
    entry: u8,
    #[hidden_element]
    current: String,
    #[hidden_element]
    staged_old: String,
    #[hidden_element]
    staged_new: String,
    #[hidden_element]
    attempts: u32,
    /// Encryption keypad as ten ASCII digits. Visual [`Self::pad`] may differ.
    #[hidden_element]
    encryption_keypad: String,
    #[hidden_element]
    visual_seed: u32,
    #[hidden_element]
    creating: bool,
    #[hidden_element]
    changing: bool,
    #[hidden_element]
    keep_message: bool,
}

impl Default for PincodeWindowState {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            masked: String::new(),
            message: String::new(),
            pad: (0..10).map(|digit| digit.to_string()).collect(),
            show_pad: false,
            show_change: false,
            needs_pin: false,
            allows_select: true,
            account_id: 0,
            seed: 0,
            mode: MODE_HIDDEN,
            entry: ENTRY_CURRENT,
            current: String::new(),
            staged_old: String::new(),
            staged_new: String::new(),
            attempts: 0,
            encryption_keypad: "0123456789".to_string(),
            visual_seed: 0,
            creating: false,
            changing: false,
            keep_message: false,
        }
    }
}

impl PincodeWindowState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn allows_select(&self) -> bool {
        self.allows_select
    }

    /// `KORANGAR_AUTOPILOT_PIN` when it is exactly four digits.
    pub fn autopilot_pin() -> Option<String> {
        let raw = std::env::var("KORANGAR_AUTOPILOT_PIN").ok()?;
        let pin = raw.trim();
        if pin.len() == 4 && pin.bytes().all(|byte| byte.is_ascii_digit()) {
            Some(pin.to_string())
        } else {
            None
        }
    }

    pub fn apply_notice(&mut self, notice: PincodeNotice, autopilot: Option<&str>) -> PincodeEffect {
        match notice {
            PincodeNotice::Login {
                seed,
                account_id,
                state,
                flag,
            } => {
                self.account_id = account_id.0;
                self.set_seed(seed);
                let mut effect = self.apply_login_state(state, flag);
                if let Some(pin) = autopilot
                    && effect.outbound.is_none()
                    && let Some(outbound) = self.autopilot(pin)
                {
                    effect.outbound = Some(outbound);
                    effect.open_window = false;
                }
                effect
            }
            PincodeNotice::MakeResult { state, seed } => {
                self.set_seed(seed);
                self.apply_make_result(state)
            }
            PincodeNotice::EditResult { state, seed } => {
                self.set_seed(seed);
                self.apply_edit_result(state)
            }
        }
    }

    pub fn press_digit(&mut self, index: usize) {
        if !self.show_pad || self.current.len() >= 4 {
            return;
        }
        let Some(label) = self.pad.get(index) else {
            return;
        };
        if label.len() == 1 && label.bytes().all(|byte| byte.is_ascii_digit()) {
            self.current.push_str(label);
            self.refresh_mask();
        }
    }

    pub fn reset_entry(&mut self) {
        if !self.show_pad {
            return;
        }
        self.current.clear();
        self.refresh_mask();
        self.visual_seed = advance_pincode_visual_seed(self.visual_seed);
        self.pad = labels(&pincode_keypad(self.visual_seed));
    }

    pub fn submit(&mut self) -> PincodeEffect {
        let Some(outbound) = self.submit_outbound() else {
            return PincodeEffect::default();
        };
        PincodeEffect {
            outbound: Some(outbound),
            ..PincodeEffect::default()
        }
    }

    pub fn start_change(&mut self) -> PincodeEffect {
        if self.mode != MODE_CHECK {
            return PincodeEffect::default();
        }
        self.mode = MODE_CHANGE;
        self.entry = ENTRY_CURRENT;
        self.changing = true;
        self.show_change = false;
        self.clear_typed();
        self.prompt = "Enter your current PIN.".to_string();
        self.message = "Choose a new PIN.".to_string();
        self.open_effect()
    }

    /// State 7 sends `0x08C5`. Any other locked state reopens the pad.
    pub fn request_or_reopen(&mut self) -> PincodeEffect {
        if self.mode == MODE_BUTTON {
            return PincodeEffect {
                outbound: Some(PincodeOutbound::Available {
                    account_id: AccountId(self.account_id),
                }),
                ..PincodeEffect::default()
            };
        }
        if self.needs_pin {
            return self.open_effect();
        }
        PincodeEffect::default()
    }

    pub fn cancel(&mut self) -> PincodeEffect {
        self.reset();
        PincodeEffect {
            close_window: true,
            disconnect: true,
            ..PincodeEffect::default()
        }
    }

    fn apply_login_state(&mut self, state: u16, flag: Option<u8>) -> PincodeEffect {
        match state {
            0 => self.on_accepted(),
            1 => self.on_ask(),
            2 | 4 => self.on_create(),
            3 => self.on_must_change(),
            5 => self.on_rejected_attempt("That PIN is restricted."),
            6 => self.on_rejected_attempt("You cannot use your social security number as a PIN."),
            7 => self.on_button(),
            8 => self.on_wrong(flag),
            _ => self.on_rejected("The server sent an unknown PIN state."),
        }
    }

    fn on_accepted(&mut self) -> PincodeEffect {
        self.attempts = 0;
        self.creating = false;
        self.changing = false;
        self.keep_message = false;
        self.mode = MODE_HIDDEN;
        self.show_pad = false;
        self.show_change = false;
        self.needs_pin = false;
        self.allows_select = true;
        self.clear_typed();
        self.prompt.clear();
        self.message.clear();
        PincodeEffect {
            close_window: true,
            ..PincodeEffect::default()
        }
    }

    fn on_ask(&mut self) -> PincodeEffect {
        if self.attempts >= MAX_ATTEMPTS {
            return self.lock("Too many PIN attempts.");
        }
        let saved = self.creating;
        self.creating = false;
        self.mode = MODE_CHECK;
        self.entry = ENTRY_CURRENT;
        self.show_pad = true;
        self.show_change = true;
        self.needs_pin = true;
        self.allows_select = false;
        self.clear_typed();
        self.prompt = "Enter your PIN.".to_string();
        if saved {
            self.message = "PIN saved. Enter it to continue.".to_string();
        } else if !self.keep_message {
            self.message.clear();
        }
        self.keep_message = false;
        self.open_effect()
    }

    fn on_create(&mut self) -> PincodeEffect {
        self.mode = MODE_CREATE;
        self.entry = ENTRY_CURRENT;
        self.show_pad = true;
        self.show_change = false;
        self.needs_pin = true;
        self.allows_select = false;
        self.clear_typed();
        self.prompt = "Choose a new PIN.".to_string();
        self.message = "Set a PIN code to continue.".to_string();
        self.open_effect()
    }

    fn on_must_change(&mut self) -> PincodeEffect {
        if self.attempts >= MAX_ATTEMPTS {
            return self.lock("Too many PIN attempts.");
        }
        self.mode = MODE_CHANGE;
        self.entry = ENTRY_CURRENT;
        self.changing = true;
        self.show_pad = true;
        self.show_change = false;
        self.needs_pin = true;
        self.allows_select = false;
        self.clear_typed();
        self.prompt = "Enter your current PIN.".to_string();
        self.message = "You must change your PIN.".to_string();
        self.open_effect()
    }

    fn on_button(&mut self) -> PincodeEffect {
        self.mode = MODE_BUTTON;
        self.show_pad = false;
        self.show_change = false;
        self.needs_pin = true;
        self.allows_select = false;
        self.clear_typed();
        self.prompt = "Open the PIN pad to continue.".to_string();
        self.message.clear();
        self.open_effect()
    }

    fn on_wrong(&mut self, flag: Option<u8>) -> PincodeEffect {
        if flag == Some(FLAG_LOCKED) {
            return self.lock("The PIN is locked.");
        }
        self.attempts += 1;
        if self.attempts >= MAX_ATTEMPTS {
            return self.lock("Too many PIN attempts.");
        }
        let message = if self.creating {
            "Could not save that PIN."
        } else {
            "Incorrect PIN."
        };
        self.on_rejected(message)
    }

    fn on_rejected_attempt(&mut self, message: &str) -> PincodeEffect {
        self.attempts += 1;
        if self.attempts >= MAX_ATTEMPTS {
            return self.lock("Too many PIN attempts.");
        }
        self.on_rejected(message)
    }

    fn on_rejected(&mut self, message: &str) -> PincodeEffect {
        self.message = message.to_string();
        self.keep_message = false;
        if self.mode == MODE_HIDDEN {
            self.mode = MODE_CHECK;
        }
        self.show_pad = self.mode != MODE_BUTTON;
        self.needs_pin = true;
        self.allows_select = false;
        self.entry = ENTRY_CURRENT;
        self.clear_typed();
        if self.mode == MODE_CREATE {
            self.prompt = "Choose a new PIN.".to_string();
        }
        self.open_effect()
    }

    fn lock(&mut self, message: &str) -> PincodeEffect {
        self.message = message.to_string();
        self.needs_pin = false;
        self.allows_select = false;
        self.show_pad = false;
        self.reset();
        self.allows_select = false;
        PincodeEffect {
            close_window: true,
            disconnect: true,
            ..PincodeEffect::default()
        }
    }

    fn apply_make_result(&mut self, state: u16) -> PincodeEffect {
        self.keep_message = true;
        self.message = match state {
            0 => "PIN saved. Enter it to continue.".to_string(),
            1 => "That PIN is already in use.".to_string(),
            2 => "That PIN is restricted.".to_string(),
            3 => "You cannot use your personal number as a PIN.".to_string(),
            _ => "Could not save the PIN.".to_string(),
        };
        if state != 0 {
            self.creating = false;
            self.mode = MODE_CREATE;
            self.entry = ENTRY_CURRENT;
            self.show_pad = true;
            self.needs_pin = true;
            self.allows_select = false;
            self.clear_typed();
            self.prompt = "Choose a new PIN.".to_string();
        }
        self.open_effect()
    }

    fn apply_edit_result(&mut self, state: u16) -> PincodeEffect {
        self.keep_message = true;
        self.message = match state {
            0 => "PIN changed. Enter the new PIN.".to_string(),
            1 => "The current PIN was not accepted.".to_string(),
            2 => "That PIN is restricted.".to_string(),
            3 => "You cannot use your personal number as a PIN.".to_string(),
            _ => "Could not change the PIN.".to_string(),
        };
        if state != 0 {
            self.mode = MODE_CHANGE;
            self.entry = ENTRY_CURRENT;
            self.changing = true;
            self.show_pad = true;
            self.show_change = false;
            self.needs_pin = true;
            self.allows_select = false;
            self.clear_typed();
            self.prompt = "Enter your current PIN.".to_string();
        }
        self.open_effect()
    }

    fn autopilot(&mut self, pin: &str) -> Option<PincodeOutbound> {
        match self.mode {
            MODE_CHECK => {
                self.current = pin.to_string();
                self.submit_outbound()
            }
            MODE_CREATE => {
                self.staged_new = pin.to_string();
                self.entry = ENTRY_CONFIRM;
                self.current = pin.to_string();
                self.submit_outbound()
            }
            _ => None,
        }
    }

    fn submit_outbound(&mut self) -> Option<PincodeOutbound> {
        if !self.show_pad {
            self.message = "Enter 4 digits.".to_string();
            return None;
        }
        if self.current.len() != 4 {
            self.message = "Enter 4 digits.".to_string();
            return None;
        }
        let account_id = AccountId(self.account_id);
        match self.mode {
            MODE_CHECK => {
                let pin = self.encrypt(&self.current.clone())?;
                self.clear_typed();
                Some(PincodeOutbound::Check { account_id, pin })
            }
            MODE_CREATE => match self.entry {
                ENTRY_CURRENT => {
                    self.staged_new = self.current.clone();
                    self.current.clear();
                    self.entry = ENTRY_CONFIRM;
                    self.prompt = "Enter the new PIN again.".to_string();
                    self.message.clear();
                    self.refresh_mask();
                    None
                }
                _ => {
                    if self.current != self.staged_new {
                        self.message = "The PIN codes do not match.".to_string();
                        self.current.clear();
                        self.refresh_mask();
                        return None;
                    }
                    let pin = self.encrypt(&self.staged_new.clone())?;
                    self.creating = true;
                    self.clear_typed();
                    Some(PincodeOutbound::Make { account_id, pin })
                }
            },
            MODE_CHANGE => match self.entry {
                ENTRY_CURRENT => {
                    self.staged_old = self.current.clone();
                    self.current.clear();
                    self.entry = ENTRY_NEW;
                    self.prompt = "Choose a new PIN.".to_string();
                    self.message.clear();
                    self.refresh_mask();
                    None
                }
                ENTRY_NEW => {
                    if self.current == self.staged_old {
                        self.message = "The new PIN must be different.".to_string();
                        self.current.clear();
                        self.refresh_mask();
                        return None;
                    }
                    self.staged_new = self.current.clone();
                    self.current.clear();
                    self.entry = ENTRY_CONFIRM;
                    self.prompt = "Enter the new PIN again.".to_string();
                    self.message.clear();
                    self.refresh_mask();
                    None
                }
                _ => {
                    if self.current != self.staged_new {
                        self.message = "The PIN codes do not match.".to_string();
                        self.current.clear();
                        self.refresh_mask();
                        return None;
                    }
                    let old_pin = self.encrypt(&self.staged_old.clone())?;
                    let new_pin = self.encrypt(&self.staged_new.clone())?;
                    self.changing = true;
                    self.clear_typed();
                    Some(PincodeOutbound::Edit {
                        account_id,
                        old_pin,
                        new_pin,
                    })
                }
            },
            _ => None,
        }
    }

    fn encrypt(&mut self, pin: &str) -> Option<[u8; 4]> {
        let keypad = self.keypad_array()?;
        match encrypt_pincode(&keypad, pin) {
            Some(encrypted) => Some(encrypted),
            None => {
                self.message = "Enter 4 digits.".to_string();
                None
            }
        }
    }

    fn keypad_array(&self) -> Option<[u8; 10]> {
        let bytes = self.encryption_keypad.as_bytes();
        if bytes.len() != 10 {
            return None;
        }
        let mut keypad = [0; 10];
        for (index, byte) in bytes.iter().copied().enumerate() {
            if !byte.is_ascii_digit() {
                return None;
            }
            keypad[index] = byte - b'0';
        }
        Some(keypad)
    }

    fn set_seed(&mut self, seed: u32) {
        self.seed = seed;
        self.visual_seed = seed;
        let keypad = pincode_keypad(seed);
        self.encryption_keypad = keypad.iter().map(|digit| char::from(b'0' + digit)).collect();
        self.pad = labels(&keypad);
    }

    fn clear_typed(&mut self) {
        self.current.clear();
        self.staged_old.clear();
        self.staged_new.clear();
        self.refresh_mask();
    }

    fn refresh_mask(&mut self) {
        self.masked = "*".repeat(self.current.len());
    }

    fn open_effect(&self) -> PincodeEffect {
        PincodeEffect {
            open_window: true,
            ..PincodeEffect::default()
        }
    }
}

fn labels(keypad: &[u8; 10]) -> Vec<String> {
    keypad.iter().map(|digit| digit.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use ragnarok_packets::decrypt_pincode;

    use super::*;

    fn login(seed: u32, account: u32, state: u16) -> PincodeNotice {
        PincodeNotice::Login {
            seed,
            account_id: AccountId(account),
            state,
            flag: None,
        }
    }

    fn check_pin(effect: &PincodeEffect) -> [u8; 4] {
        match effect.outbound {
            Some(PincodeOutbound::Check { pin, .. }) => pin,
            other => panic!("expected check, got {other:?}"),
        }
    }

    #[test]
    fn accepted_state_unlocks_character_select() {
        let mut state = PincodeWindowState::default();
        let effect = state.apply_notice(login(7, 9, 1), None);
        assert!(effect.open_window);
        assert!(!state.allows_select());
        let effect = state.apply_notice(login(7, 9, 0), None);
        assert!(effect.close_window);
        assert!(state.allows_select());
        assert!(!state.needs_pin);
    }

    #[test]
    fn ask_ok_sends_ciphertext_the_server_decrypts() {
        let mut state = PincodeWindowState::default();
        state.apply_notice(login(0, 0x0102_0304, 1), None);
        let keypad = pincode_keypad(0);
        for (index, label) in labels(&keypad).iter().enumerate() {
            if label == "0" {
                state.press_digit(index);
            }
        }
        for digit in ['1', '2', '3'] {
            let index = labels(&keypad).iter().position(|label| label == &digit.to_string()).unwrap();
            state.press_digit(index);
        }
        assert_eq!(state.masked, "****");
        let effect = state.submit();
        let pin = check_pin(&effect);
        assert_eq!(decrypt_pincode(0, pin), *b"0123");
        assert_eq!(pin, *b"1380");
    }

    #[test]
    fn create_requires_a_matching_confirm() {
        let mut state = PincodeWindowState::default();
        state.apply_notice(login(0, 3, 2), None);
        type_pin(&mut state, "2580");
        assert!(state.submit().outbound.is_none());
        assert_eq!(state.prompt, "Enter the new PIN again.");
        type_pin(&mut state, "2581");
        assert!(state.submit().outbound.is_none());
        assert_eq!(state.message, "The PIN codes do not match.");
        type_pin(&mut state, "2580");
        let effect = state.submit();
        match effect.outbound {
            Some(PincodeOutbound::Make { pin, account_id }) => {
                assert_eq!(account_id, AccountId(3));
                assert_eq!(decrypt_pincode(0, pin), *b"2580");
            }
            other => panic!("expected make, got {other:?}"),
        }
    }

    #[test]
    fn state_four_uses_the_create_flow() {
        let mut state = PincodeWindowState::default();
        state.apply_notice(login(1, 1, 4), None);
        assert!(state.show_pad);
        assert!(!state.show_change);
        assert_eq!(state.prompt, "Choose a new PIN.");
    }

    #[test]
    fn change_sends_old_and_new() {
        let mut state = PincodeWindowState::default();
        state.apply_notice(login(0, 8, 3), None);
        type_pin(&mut state, "0123");
        assert!(state.submit().outbound.is_none());
        type_pin(&mut state, "0123");
        assert!(state.submit().outbound.is_none());
        assert_eq!(state.message, "The new PIN must be different.");
        type_pin(&mut state, "9999");
        assert!(state.submit().outbound.is_none());
        type_pin(&mut state, "9999");
        match state.submit().outbound {
            Some(PincodeOutbound::Edit { old_pin, new_pin, .. }) => {
                assert_eq!(decrypt_pincode(0, old_pin), *b"0123");
                assert_eq!(decrypt_pincode(0, new_pin), *b"9999");
            }
            other => panic!("expected edit, got {other:?}"),
        }
    }

    #[test]
    fn third_wrong_pin_disconnects() {
        let mut state = PincodeWindowState::default();
        state.apply_notice(login(1, 1, 1), None);
        for _ in 0..2 {
            let effect = state.apply_notice(
                PincodeNotice::Login {
                    seed: 2,
                    account_id: AccountId(1),
                    state: 8,
                    flag: Some(2),
                },
                None,
            );
            assert!(effect.open_window);
            assert!(!effect.disconnect);
            assert_eq!(state.message, "Incorrect PIN.");
        }
        let effect = state.apply_notice(
            PincodeNotice::Login {
                seed: 3,
                account_id: AccountId(1),
                state: 8,
                flag: Some(2),
            },
            None,
        );
        assert!(effect.disconnect);
        assert!(effect.close_window);
    }

    #[test]
    fn locked_flag_disconnects_immediately() {
        let mut state = PincodeWindowState::default();
        let effect = state.apply_notice(
            PincodeNotice::Login {
                seed: 1,
                account_id: AccountId(1),
                state: 8,
                flag: Some(0),
            },
            None,
        );
        assert!(effect.disconnect);
    }

    #[test]
    fn restricted_and_kssn_states_keep_the_pad_open() {
        let mut state = PincodeWindowState::default();
        state.apply_notice(login(1, 1, 5), None);
        assert_eq!(state.message, "That PIN is restricted.");
        assert!(state.show_pad);
        state.apply_notice(login(1, 1, 6), None);
        assert_eq!(state.message, "You cannot use your social security number as a PIN.");
    }

    #[test]
    fn button_state_requests_the_window() {
        let mut state = PincodeWindowState::default();
        let effect = state.apply_notice(login(4, 6, 7), None);
        assert!(effect.open_window);
        assert!(!state.show_pad);
        assert!(!state.allows_select());
        match state.request_or_reopen().outbound {
            Some(PincodeOutbound::Available { account_id }) => assert_eq!(account_id, AccountId(6)),
            other => panic!("expected available, got {other:?}"),
        }
    }

    #[test]
    fn make_failure_stays_on_create() {
        let mut state = PincodeWindowState::default();
        state.apply_notice(login(9, 1, 2), None);
        let effect = state.apply_notice(PincodeNotice::MakeResult { state: 2, seed: 9 }, None);
        assert!(effect.open_window);
        assert_eq!(state.message, "That PIN is restricted.");
        assert_eq!(state.prompt, "Choose a new PIN.");
    }

    #[test]
    fn edit_failure_returns_to_the_current_pin() {
        let mut state = PincodeWindowState::default();
        state.apply_notice(login(9, 1, 3), None);
        state.apply_notice(PincodeNotice::EditResult { state: 1, seed: 4 }, None);
        assert_eq!(state.message, "The current PIN was not accepted.");
        assert_eq!(state.prompt, "Enter your current PIN.");
    }

    #[test]
    fn reset_moves_labels_without_changing_encryption() {
        let mut state = PincodeWindowState::default();
        state.apply_notice(login(0, 1, 1), None);
        let before = state.encryption_keypad.clone();
        state.reset_entry();
        assert_eq!(state.encryption_keypad, before);
        assert_ne!(state.pad, labels(&pincode_keypad(0)));
        type_pin(&mut state, "0123");
        let pin = check_pin(&state.submit());
        assert_eq!(decrypt_pincode(0, pin), *b"0123");
    }

    #[test]
    fn autopilot_submits_check_and_create() {
        let mut state = PincodeWindowState::default();
        let effect = state.apply_notice(login(0, 1, 1), Some("2580"));
        assert!(!effect.open_window);
        assert_eq!(decrypt_pincode(0, check_pin(&effect)), *b"2580");

        let mut state = PincodeWindowState::default();
        let effect = state.apply_notice(login(0, 1, 4), Some("9999"));
        match effect.outbound {
            Some(PincodeOutbound::Make { pin, .. }) => assert_eq!(decrypt_pincode(0, pin), *b"9999"),
            other => panic!("expected make, got {other:?}"),
        }
    }

    #[test]
    fn cancel_disconnects() {
        let mut state = PincodeWindowState::default();
        state.apply_notice(login(1, 1, 1), None);
        let effect = state.cancel();
        assert!(effect.disconnect && effect.close_window);
        assert!(state.allows_select());
    }

    fn type_pin(state: &mut PincodeWindowState, pin: &str) {
        for digit in pin.chars() {
            let index = state.pad.iter().position(|label| label == &digit.to_string()).unwrap();
            state.press_digit(index);
        }
    }
}
