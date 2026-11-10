use std::cell::UnsafeCell;

use korangar_interface::element::store::ElementStoreMut;
use korangar_interface::element::{Element, ElementBox, ErasedElement, StateElement};
use korangar_interface::layout::{Resolvers, with_single_resolver};
use korangar_interface::window::{CustomWindow, Window};
use ragnarok_packets::EntityId;
use rust_state::{Path, RustState, State};

use super::WindowClass;
use crate::input::InputEvent;
use crate::state::localization::LocalizationPathExt;
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};

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

    /// Add text to the dialog.
    pub fn add_text(&mut self, text: String) {
        use korangar_interface::prelude::*;

        if self.clear_next {
            self.elements.clear();
            self.clear_next = false;
            self.awaiting_next = false;
            self.awaiting_close = false;
            self.awaiting_choice = false;
        }

        self.elements.push(DialogElement::new(
            text! {
                text: text,
            },
            false,
        ));
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
        }
    }
}

/// Wrapper struct for collecting all [`DialogElement::element`]s into a single
/// element.
struct InnerElement<A> {
    dialog_elements_path: A,
}

impl<A> Element<ClientState> for InnerElement<A>
where
    A: Path<ClientState, Vec<DialogElement>>,
{
    type LayoutInfo = ();

    fn create_layout_info(&mut self, state: &State<ClientState>, mut store: ElementStoreMut, resolvers: &mut dyn Resolvers<ClientState>) {
        with_single_resolver(resolvers, |resolver| {
            state
                .get(&self.dialog_elements_path)
                .iter()
                .enumerate()
                .for_each(|(index, dialog_element)| {
                    // We only create this mutable reference for the lifetime of this scope, and
                    // since nothing is captured from the element this is safe.
                    let element = unsafe { &mut *dialog_element.element.get() };

                    element.create_layout_info(state, store.child_store(index as u64), resolver)
                });
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        store: korangar_interface::element::store::ElementStore<'a>,
        _: &'a Self::LayoutInfo,
        layout: &mut korangar_interface::layout::WindowLayout<'a, ClientState>,
    ) {
        state
            .get(&self.dialog_elements_path)
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
            title: client_state().localization().dialog_window_title(),
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            elements: (
                InnerElement {
                    dialog_elements_path: self.window_state_path.elements(),
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
    }
}
