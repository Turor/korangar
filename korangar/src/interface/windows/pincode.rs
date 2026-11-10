use korangar_interface::window::{CustomWindow, Window};
use rust_state::{ManuallyAssertExt, VecIndexExt};

use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::state::pincode::PincodeWindowStatePathExt;
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};

/// Mouse-only PIN pad. Digits are shown as asterisks. The number order comes
/// from the server seed, matching roBrowser's `PincodeWindow`.
pub struct PincodeWindow;

impl CustomWindow<ClientState> for PincodeWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Pincode)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let pad = client_state().pincode_window().pad();

        window! {
            title: "PIN code",
            class: Self::window_class(),
            theme: InterfaceThemeType::Menu,
            closable: true,
            minimum_width: 280.0,
            maximum_width: 360.0,
            elements: (
                text! { text: client_state().pincode_window().prompt() },
                text! { text: client_state().pincode_window().masked() },
                text! { text: client_state().pincode_window().message() },
                korangar_interface::components::either::Either::component_new(
                    client_state().pincode_window().show_pad(),
                    fragment! {
                        gaps: theme().window().gaps(),
                        children: (
                            split! {
                                gaps: theme().window().gaps(),
                                children: (
                                    button! {
                                        text: pad.index(1).manually_asserted(),
                                        event: InputEvent::PincodeDigit { index: 1 },
                                    },
                                    button! {
                                        text: pad.index(2).manually_asserted(),
                                        event: InputEvent::PincodeDigit { index: 2 },
                                    },
                                    button! {
                                        text: pad.index(3).manually_asserted(),
                                        event: InputEvent::PincodeDigit { index: 3 },
                                    },
                                ),
                            },
                            split! {
                                gaps: theme().window().gaps(),
                                children: (
                                    button! {
                                        text: pad.index(4).manually_asserted(),
                                        event: InputEvent::PincodeDigit { index: 4 },
                                    },
                                    button! {
                                        text: pad.index(5).manually_asserted(),
                                        event: InputEvent::PincodeDigit { index: 5 },
                                    },
                                    button! {
                                        text: pad.index(6).manually_asserted(),
                                        event: InputEvent::PincodeDigit { index: 6 },
                                    },
                                ),
                            },
                            split! {
                                gaps: theme().window().gaps(),
                                children: (
                                    button! {
                                        text: pad.index(7).manually_asserted(),
                                        event: InputEvent::PincodeDigit { index: 7 },
                                    },
                                    button! {
                                        text: pad.index(8).manually_asserted(),
                                        event: InputEvent::PincodeDigit { index: 8 },
                                    },
                                    button! {
                                        text: pad.index(9).manually_asserted(),
                                        event: InputEvent::PincodeDigit { index: 9 },
                                    },
                                ),
                            },
                            split! {
                                gaps: theme().window().gaps(),
                                children: (
                                    button! {
                                        text: "Reset",
                                        event: InputEvent::PincodeReset,
                                    },
                                    button! {
                                        text: pad.index(0).manually_asserted(),
                                        event: InputEvent::PincodeDigit { index: 0 },
                                    },
                                    button! {
                                        text: "OK",
                                        event: InputEvent::PincodeSubmit,
                                    },
                                ),
                            },
                            korangar_interface::components::either::Either::component_new(
                                client_state().pincode_window().show_change(),
                                button! {
                                    text: "Change",
                                    event: InputEvent::PincodeChange,
                                },
                                (),
                            ),
                            button! {
                                text: "Cancel",
                                event: InputEvent::PincodeCancel,
                            },
                        ),
                    },
                    button! {
                        text: "Enter PIN",
                        event: InputEvent::PincodeRequest,
                    },
                ),
            ),
        }
    }
}
