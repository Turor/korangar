//! Emotion picker. Alt+1 through Alt+9 still send the first nine emotions.
//! The buttons send the same [`crate::input::InputEvent::SendEmotion`] event.

use korangar_interface::window::{CustomWindow, Window};

use super::WindowClass;
use crate::input::InputEvent;
use crate::state::ClientState;
use crate::state::theme::InterfaceThemeType;

const EMOTES: &[(&str, &str, u8)] = &[
    ("Surprise", "Surprise (Alt+1)", 0),
    ("Question", "Question (Alt+2)", 1),
    ("Delight", "Delight (Alt+3)", 2),
    ("Heart", "Heart (Alt+4)", 3),
    ("Sweat", "Sweat (Alt+5)", 4),
    ("Idea", "Idea (Alt+6)", 5),
    ("Fret", "Fret (Alt+7)", 6),
    ("Anger", "Anger (Alt+8)", 7),
    ("Zeny", "Zeny (Alt+9)", 8),
];

pub struct EmotesWindow;

impl CustomWindow<ClientState> for EmotesWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Emotes)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        window! {
            title: "Emotes",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            minimum_width: 220.0,
            maximum_width: 280.0,
            elements: scroll_view! {
                children: (
                    button! { text: EMOTES[0].0, tooltip: EMOTES[0].1, event: InputEvent::SendEmotion { emotion: EMOTES[0].2 } },
                    button! { text: EMOTES[1].0, tooltip: EMOTES[1].1, event: InputEvent::SendEmotion { emotion: EMOTES[1].2 } },
                    button! { text: EMOTES[2].0, tooltip: EMOTES[2].1, event: InputEvent::SendEmotion { emotion: EMOTES[2].2 } },
                    button! { text: EMOTES[3].0, tooltip: EMOTES[3].1, event: InputEvent::SendEmotion { emotion: EMOTES[3].2 } },
                    button! { text: EMOTES[4].0, tooltip: EMOTES[4].1, event: InputEvent::SendEmotion { emotion: EMOTES[4].2 } },
                    button! { text: EMOTES[5].0, tooltip: EMOTES[5].1, event: InputEvent::SendEmotion { emotion: EMOTES[5].2 } },
                    button! { text: EMOTES[6].0, tooltip: EMOTES[6].1, event: InputEvent::SendEmotion { emotion: EMOTES[6].2 } },
                    button! { text: EMOTES[7].0, tooltip: EMOTES[7].1, event: InputEvent::SendEmotion { emotion: EMOTES[7].2 } },
                    button! { text: EMOTES[8].0, tooltip: EMOTES[8].1, event: InputEvent::SendEmotion { emotion: EMOTES[8].2 } },
                ),
            },
        }
    }
}
