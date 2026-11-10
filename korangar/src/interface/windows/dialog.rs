use std::cell::UnsafeCell;

use korangar_interface::element::store::ElementStoreMut;
use korangar_interface::element::{Element, ElementBox, ErasedElement, StateElement};
use korangar_interface::layout::area::{Area, PartialArea};
use korangar_interface::layout::{Resolver, Resolvers, with_single_resolver};
use korangar_interface::prelude::{ButtonThemePathExt, WindowThemePathExt};
use korangar_interface::window::{CustomWindow, Window};
use ragnarok_packets::EntityId;
use rust_state::{Path, RustState, State};

use super::WindowClass;
use crate::graphics::{Color, ShadowPadding};
use crate::input::InputEvent;
use crate::state::localization::LocalizationPathExt;
use crate::state::theme::{InterfaceThemePathExt, InterfaceThemeType, ThemeTokensPathExt};
use crate::state::vn_dialog::{self, ChoiceKeys, advance_reveal, choice_grid, choice_key_outcome};
use crate::state::{ClientState, ClientStatePathExt, client_state, client_theme};

/// A small wrapper struct that serves two purposes:
/// - Making the elements nicer to construct by putting the [`UnsafeCell::new`]
///   and [`Box::new`] behind a function call.
/// - Storing information about which elements are next buttons since we need to
///   be able to remove those individually.
#[derive(RustState, StateElement)]
pub struct DialogElement {
    /// Stores the UI element.
    // TODO: Unfortunately this has to be an unsafe cell as of now. Ideally this can be changed
    // later.
    #[hidden_element]
    element: UnsafeCell<ElementBox<ClientState>>,
    is_next_button: bool,
}

impl DialogElement {
    /// Creates a new dialog element.
    #[inline(always)]
    fn new<E>(element: E, is_next_button: bool) -> Self
    where
        E: Element<ClientState> + 'static,
    {
        Self {
            element: UnsafeCell::new(ErasedElement::new(element)),
            is_next_button,
        }
    }
}

/// Internal state of the dialog window.
#[derive(RustState, StateElement)]
pub struct DialogWindowState {
    /// All current dialog elements.
    elements: Vec<DialogElement>,
    /// The entity id of the NPC the player is talking to.
    npc_id: EntityId,
    /// Whether or not the elements should be cleared the next time
    /// [`start`](Self::start) is called.
    clear_next: bool,
    /// Enter or Space presses Next.
    awaiting_next: bool,
    /// Enter or Space presses Close when Next is not showing.
    awaiting_close: bool,
    /// A choice menu is showing. Keyboard Next/Close stays idle.
    awaiting_choice: bool,
    /// Numeric or string input is waiting for OK or Cancel.
    input_open: bool,
    /// The open input is a number, so the window shows up and down.
    input_number: bool,
    /// Text currently in the input box.
    input_text: String,
    /// Focus the input box once after it opens.
    input_needs_focus: bool,
    /// Full text of the current page, including `^RRGGBB` codes.
    page_text: String,
    /// Prefix of `page_text` the typewriter has revealed.
    shown_text: String,
    revealed_bytes: usize,
    reveal_carry_ms: f32,
    last_reveal_ms: Option<u32>,
    /// Name plate above the text. Entity details fill this in.
    npc_name: String,
    /// An item prompt is waiting for an explicit choice.
    prompt_blocks: bool,
    /// Screen-derived width. Minimum and maximum are both this value.
    band_width: f32,
    /// Screen-derived height, including the name plate.
    band_height: f32,
    /// Bottom edge of the band window, in interface coordinates. The menu row
    /// stands on it.
    window_bottom: f32,
    /// Option the keyboard highlights while a menu is open.
    choice_focus: usize,
}

/// Values taken from the input box when the player confirms or cancels.
pub struct NpcInputSubmission {
    pub npc_id: EntityId,
    pub number: bool,
    pub text: String,
}

impl DialogWindowState {
    /// Initialize the dialog. This is important so we have the correct entity
    /// id when sending packets to the server.
    pub fn initialize(&mut self, npc_id: EntityId) -> &mut Self {
        self.npc_id = npc_id;
        self
    }

    pub fn npc_id(&self) -> EntityId {
        self.npc_id
    }

    pub fn is_awaiting_next(&self) -> bool {
        self.awaiting_next
    }

    pub fn is_awaiting_close(&self) -> bool {
        self.awaiting_close
    }

    pub fn is_awaiting_choice(&self) -> bool {
        self.awaiting_choice
    }

    pub fn input_is_open(&self) -> bool {
        self.input_open
    }

    /// `true` once, the frame after a number or string input opens.
    pub fn take_input_focus(&mut self) -> bool {
        let focus = self.input_needs_focus;
        self.input_needs_focus = false;
        focus
    }

    pub fn open_number_input(&mut self) {
        self.input_open = true;
        self.input_number = true;
        self.input_text = "0".to_owned();
        self.input_needs_focus = true;
    }

    pub fn open_text_input(&mut self) {
        self.input_open = true;
        self.input_number = false;
        self.input_text.clear();
        self.input_needs_focus = true;
    }

    /// Clears the open input and returns what the player typed.
    pub fn take_input(&mut self) -> Option<NpcInputSubmission> {
        if !self.input_open {
            return None;
        }

        self.input_open = false;
        self.input_needs_focus = false;
        Some(NpcInputSubmission {
            npc_id: self.npc_id,
            number: self.input_number,
            text: std::mem::take(&mut self.input_text),
        })
    }

    pub fn has_page(&self) -> bool {
        !self.page_text.is_empty()
    }

    pub fn is_revealing(&self) -> bool {
        self.revealed_bytes < self.page_text.len()
    }

    /// Menus, inputs, and a cutin with no page block stray clicks. A cutin
    /// drawn beside a next or close page does not.
    pub fn blocks_advance(&self, cutin_visible: bool) -> bool {
        let cutin_without_page = cutin_visible
            && self.page_text.is_empty()
            && !self.awaiting_next
            && !self.awaiting_close
            && !self.awaiting_choice
            && !self.input_open;
        vn_dialog::blocks_stray_click(self.awaiting_choice, self.input_open, self.prompt_blocks, cutin_without_page)
    }

    pub fn set_window_bottom(&mut self, bottom: f32) {
        self.window_bottom = bottom;
    }

    /// Arrow, Tab, Enter, Space and digit keys on an open menu. Returns the
    /// option to send (one based, as the server expects).
    pub fn apply_choice_keys(&mut self, keys: ChoiceKeys) -> Option<i8> {
        if !self.awaiting_choice || self.elements.is_empty() || !keys.any() {
            return None;
        }
        let (focus, pick) = choice_key_outcome(self.choice_focus, self.elements.len(), keys);
        self.choice_focus = focus;
        pick.map(|index| index as i8 + 1)
    }

    pub fn set_band(&mut self, width: f32, height: f32) {
        self.band_width = width;
        self.band_height = height;
    }

    pub fn set_npc_name(&mut self, name: String) {
        if self.npc_name != name {
            self.npc_name = name;
        }
    }

    /// The first tick after a new page spends no time, so a long frame cannot
    /// dump the whole page.
    pub fn tick_typewriter(&mut self, now_ms: u32, chars_per_sec: f32) {
        let delta_ms = match self.last_reveal_ms.replace(now_ms) {
            Some(last) => now_ms.wrapping_sub(last),
            None => 0,
        };
        let (end, carry) = advance_reveal(
            &self.page_text,
            self.revealed_bytes,
            self.reveal_carry_ms,
            delta_ms,
            vn_dialog::typewriter_rate(chars_per_sec),
        );
        self.revealed_bytes = end;
        self.reveal_carry_ms = carry;
        self.shown_text = self.page_text[..end].to_owned();
    }

    pub fn complete_reveal(&mut self) {
        self.revealed_bytes = self.page_text.len();
        self.reveal_carry_ms = 0.0;
        self.shown_text.clone_from(&self.page_text);
    }

    /// Enter or Space finishes the reveal and does not advance.
    pub fn take_confirm_key(&mut self, characters: &[char]) -> bool {
        if !self.is_revealing() || self.blocks_advance(false) {
            return false;
        }
        let pressed = characters
            .iter()
            .any(|character| *character == '\n' || *character == '\r' || *character == ' ');
        if !pressed {
            return false;
        }
        self.complete_reveal();
        true
    }

    /// Add text to the dialog.
    pub fn add_text(&mut self, text: String) {
        if self.clear_next {
            self.elements.clear();
            self.clear_next = false;
            self.awaiting_next = false;
            self.awaiting_close = false;
            self.awaiting_choice = false;
            self.page_text.clear();
            self.reset_reveal();
        }

        if !self.page_text.is_empty() {
            self.page_text.push('\n');
        }
        self.page_text.push_str(&text);
        let end = self.revealed_bytes.min(self.page_text.len());
        self.shown_text = self.page_text[..end].to_owned();
    }

    fn reset_reveal(&mut self) {
        self.revealed_bytes = 0;
        self.reveal_carry_ms = 0.0;
        self.last_reveal_ms = None;
        self.shown_text.clear();
    }

    /// Add add next button to the dialog.
    ///
    /// This also sets the internal state to clear the dialog the next time
    /// [`start`](Self::start) is called.
    pub fn add_next_button(&mut self) {
        use korangar_interface::prelude::*;

        let npc_id = self.npc_id;

        self.elements.push(DialogElement::new(
            button! {
                text: client_state().localization().next_button_text(),
                event: move |_: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                    queue.queue(InputEvent::NextDialog { npc_id });
                },
            },
            true,
        ));

        self.clear_next = true;
        self.awaiting_next = true;
        self.awaiting_choice = false;
    }

    /// Add a close button to the dialog.
    ///
    /// This also removes any existing "Next"-buttons.
    ///
    /// I am unsure why that's the behavior of the official client.
    pub fn add_close_button(&mut self) {
        use korangar_interface::prelude::*;

        self.elements.retain(|element| !element.is_next_button);
        self.awaiting_next = false;
        self.awaiting_close = true;
        self.awaiting_choice = false;

        let npc_id = self.npc_id;

        self.elements.push(DialogElement::new(
            button! {
                text: client_state().localization().close_button_text(),
                event: move |_: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                    queue.queue(InputEvent::CloseDialog { npc_id });
                },
            },
            false,
        ));
    }

    /// Add multiple buttons, one for each choice.
    ///
    /// This also removes any existing "Next"-buttons.
    ///
    /// I am unsure why that's the behavior of the official client.
    pub fn add_choice_buttons(&mut self, choices: Vec<String>) {
        use korangar_interface::prelude::*;

        self.elements.retain(|element| !element.is_next_button);
        self.awaiting_next = false;
        self.awaiting_choice = true;
        self.choice_focus = 0;

        let npc_id = self.npc_id;

        choices.into_iter().enumerate().for_each(|(index, text)| {
            self.elements.push(DialogElement::new(
                button! {
                    text: text,
                    event: move |_: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                        queue.queue(InputEvent::ChooseDialogOption { npc_id, option: index as i8 + 1 });
                    },
                },
                false,
            ))
        });
    }

    /// End the dialog.
    ///
    /// This has no side effects.
    pub fn end(&mut self) {
        self.elements.clear();
        self.clear_next = false;
        self.awaiting_next = false;
        self.awaiting_close = false;
        self.awaiting_choice = false;
        self.input_open = false;
        self.input_needs_focus = false;
        self.input_text.clear();
        self.page_text.clear();
        self.prompt_blocks = false;
        self.reset_reveal();
    }
}

impl Default for DialogWindowState {
    fn default() -> Self {
        Self {
            elements: Default::default(),
            // Arguably not very clean but avoids using an Option.
            npc_id: EntityId(0),
            clear_next: false,
            awaiting_next: false,
            awaiting_close: false,
            awaiting_choice: false,
            input_open: false,
            input_number: false,
            input_text: String::new(),
            input_needs_focus: false,
            page_text: String::new(),
            shown_text: String::new(),
            revealed_bytes: 0,
            reveal_carry_ms: 0.0,
            last_reveal_ms: None,
            npc_name: "NPC".to_owned(),
            prompt_blocks: false,
            band_width: 640.0,
            band_height: 180.0,
            window_bottom: 0.0,
            choice_focus: 0,
        }
    }
}

/// Wrapper struct for collecting all [`DialogElement::element`]s into a single
/// element.
struct InnerElement<A> {
    window_state_path: A,
}

impl<A> Element<ClientState> for InnerElement<A>
where
    A: Path<ClientState, DialogWindowState>,
{
    /// The keyboard focus ring of an open menu.
    type LayoutInfo = Option<Area>;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        mut store: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            let dialog = state.get(&self.window_state_path);

            // Next, Close and menu options all sit in rows along the bottom
            // of the band. Anything else (number or text input) keeps the
            // plain stacked layout.
            let bottom_row = (dialog.awaiting_choice || dialog.awaiting_next || dialog.awaiting_close) && !dialog.input_open;
            if !bottom_row || dialog.elements.is_empty() {
                dialog.elements.iter().enumerate().for_each(|(index, dialog_element)| {
                    // We only create this mutable reference for the lifetime of this scope, and
                    // since nothing is captured from the element this is safe.
                    let element = unsafe { &mut *dialog_element.element.get() };

                    element.create_layout_info(state, store.child_store(index as u64), resolver)
                });
                return None;
            }

            let (columns, rows) = choice_grid(dialog.elements.len());
            let button_height: f32 = *state.get(&client_theme().button().height());
            let gaps: f32 = *state.get(&client_theme().window().gaps());
            let border: f32 = *state.get(&client_theme().window().border());
            let rows_height = rows as f32 * button_height + rows.saturating_sub(1) as f32 * gaps;

            let available = resolver.push_available_area();
            let spacer = (dialog.window_bottom - border - rows_height - available.top).max(0.0);
            let row_top = available.top + spacer;
            let cell_width = (available.width - gaps * columns.saturating_sub(1) as f32) / columns as f32;
            let focus = dialog.choice_focus.min(dialog.elements.len() - 1);
            let mut focus_area = None;

            {
                let text_layouter = resolver.get_text_layouter();
                dialog.elements.iter().enumerate().for_each(|(index, dialog_element)| {
                    let area = Area {
                        left: available.left + (index % columns) as f32 * (cell_width + gaps),
                        top: row_top + (index / columns) as f32 * (button_height + gaps),
                        width: cell_width,
                        height: button_height,
                    };
                    if dialog.awaiting_choice && index == focus {
                        focus_area = Some(area);
                    }

                    let mut cell = Resolver::new(
                        PartialArea {
                            left: area.left,
                            top: area.top,
                            width: area.width,
                            height: None,
                        },
                        gaps,
                        text_layouter,
                    );
                    // Same safety argument as above.
                    let element = unsafe { &mut *dialog_element.element.get() };
                    element.create_layout_info(state, store.child_store(index as u64), &mut cell);
                });
            }

            resolver.commit_used_height(spacer + rows_height);
            focus_area
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        store: korangar_interface::element::store::ElementStore<'a>,
        focus_area: &'a Self::LayoutInfo,
        layout: &mut korangar_interface::layout::WindowLayout<'a, ClientState>,
    ) {
        if let Some(area) = focus_area {
            // A ring in the theme accent, drawn under the button so its fill
            // leaves only the edge showing.
            let ring = 2.0;
            layout.add_rectangle(
                Area {
                    left: area.left - ring,
                    top: area.top - ring,
                    width: area.width + ring * 2.0,
                    height: area.height + ring * 2.0,
                },
                *state.get(&client_theme().button().corner_diameter()),
                *state.get(&client_theme().tokens().accent()),
                Color::TRANSPARENT,
                ShadowPadding::uniform(0.0),
            );
        }

        state
            .get(&self.window_state_path)
            .elements
            .iter()
            .enumerate()
            .for_each(|(index, dialog_element)| {
                // There are no mutable references at this point in time and the immutable
                // reference will be dropped after the interface is rendered, making this safe.
                let element = unsafe { &*dialog_element.element.get() };

                element.lay_out(state, store.child_store(index as u64), &(), layout)
            });
    }
}

/// A window representing a dialog with an NPC.
pub struct DialogWindow<A> {
    /// Path to the [`DialogWindowState`].
    window_state_path: A,
}

impl<A> DialogWindow<A> {
    /// Creates a new dialog window.
    ///
    /// This does not modify the [`DialogWindowState`].
    pub fn new(window_state_path: A) -> Self {
        Self { window_state_path }
    }
}

impl<A> CustomWindow<ClientState> for DialogWindow<A>
where
    A: Path<ClientState, DialogWindowState>,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Dialog)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        window! {
            title: self.window_state_path.npc_name(),
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            // No colour override: the band uses the shared window background
            // (`panel` at the theme `window_alpha`, 1.0 = opaque).
            closable: true,
            resizable: false,
            minimum_width: self.window_state_path.band_width(),
            maximum_width: self.window_state_path.band_width(),
            minimum_height: self.window_state_path.band_height(),
            maximum_height: self.window_state_path.band_height(),
            elements: (
                text! {
                    text: self.window_state_path.shown_text(),
                },
                InnerElement {
                    window_state_path: self.window_state_path,
                },
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use ragnarok_packets::EntityId;

    use super::DialogWindowState;

    #[test]
    fn next_page_clears_keyboard_flags_until_the_next_button() {
        let mut dialog = DialogWindowState::default();
        dialog.initialize(EntityId(4)).add_next_button();
        assert!(dialog.is_awaiting_next());

        dialog.add_text("page".to_owned());
        assert!(!dialog.is_awaiting_next());
        assert!(!dialog.is_awaiting_close());

        dialog.add_close_button();
        assert!(!dialog.is_awaiting_next());
        assert!(dialog.is_awaiting_close());
    }

    #[test]
    fn choices_block_next_and_end_clears_input() {
        let mut dialog = DialogWindowState::default();
        dialog.initialize(EntityId(1));
        dialog.add_choice_buttons(vec!["A".to_owned()]);
        assert!(dialog.is_awaiting_choice());
        assert!(!dialog.is_awaiting_next());

        dialog.open_text_input();
        assert!(dialog.input_is_open());
        assert!(dialog.take_input_focus());
        assert!(!dialog.take_input_focus());
        let taken = dialog.take_input().expect("input");
        assert!(!taken.number);
        assert!(taken.text.is_empty());
        assert!(!dialog.input_is_open());

        dialog.open_number_input();
        dialog.end();
        assert!(!dialog.input_is_open());
        assert!(dialog.take_input().is_none());
        assert!(!dialog.has_page());
    }

    #[test]
    fn typewriter_completes_on_demand_and_the_next_page_starts_over() {
        let mut dialog = DialogWindowState::default();
        dialog.add_text("Hello".to_owned());
        assert!(dialog.is_revealing());
        assert!(dialog.shown_text.is_empty());

        dialog.tick_typewriter(1_000, 100.0);
        dialog.tick_typewriter(1_050, 100.0);
        assert_eq!(dialog.shown_text, "Hello");
        assert!(!dialog.is_revealing());

        dialog.add_next_button();
        assert!(dialog.is_awaiting_next());
        assert!(!dialog.blocks_advance(true));

        dialog.add_text("^ff8800Slow".to_owned());
        assert!(dialog.is_revealing());
        assert!(!dialog.is_awaiting_next());
        dialog.complete_reveal();
        assert_eq!(dialog.shown_text, "^ff8800Slow");

        dialog.add_choice_buttons(vec!["Stay".to_owned()]);
        assert!(dialog.blocks_advance(false));

        dialog.prompt_blocks = true;
        dialog.awaiting_choice = false;
        assert!(dialog.blocks_advance(false));
    }
}
