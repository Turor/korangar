//! Line-oriented autopilot commands. See `docs/autopilot.md`.

use crate::autopilot::events::{scripted_event, toggle_event, window_command};
use crate::autopilot::keycode::parse_key_spec;
use crate::input::InputEvent;
use crate::interface::windows::WindowClass;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClickKind {
    Left,
    Right,
    Double,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DumpPath {
    Inventory,
    Hotbar,
    ChatMessages,
    Windows,
    Player,
}

impl DumpPath {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Inventory => "inventory",
            Self::Hotbar => "hotbar",
            Self::ChatMessages => "chat_messages",
            Self::Windows => "windows",
            Self::Player => "player",
        }
    }

    fn parse(name: &str) -> Result<Self, String> {
        match name {
            "inventory" => Ok(Self::Inventory),
            "hotbar" => Ok(Self::Hotbar),
            "chat_messages" => Ok(Self::ChatMessages),
            "windows" => Ok(Self::Windows),
            "player" => Ok(Self::Player),
            other => Err(format!(
                "unknown dump path {other} (inventory, hotbar, chat_messages, windows, player)"
            )),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Command {
    Chat(String),
    Window {
        class: WindowClass,
        open: bool,
        event: InputEvent,
    },
    Toggle(InputEvent),
    Event(InputEvent),
    Wait(u64),
    Move {
        x: u16,
        y: u16,
    },
    Click {
        x: f32,
        y: f32,
        kind: ClickKind,
    },
    Key {
        code: winit::keyboard::KeyCode,
        alt: bool,
        ctrl: bool,
        shift: bool,
    },
    Dump(DumpPath),
    Quit,
}

pub fn parse_command(line: &str) -> Result<Command, String> {
    let line = line.trim();
    if line.is_empty() {
        return Err("empty command".to_owned());
    }
    let (head, rest) = split_head(line);
    match head {
        "chat" => {
            if rest.is_empty() {
                return Err("chat needs text".to_owned());
            }
            Ok(Command::Chat(rest.to_owned()))
        }
        "open" => parse_window(rest, true),
        "close" => parse_window(rest, false),
        "toggle" => Ok(Command::Toggle(toggle_event(rest)?)),
        "event" => Ok(Command::Event(scripted_event(rest)?)),
        "wait" => {
            let ms: u64 = rest.parse().map_err(|_| format!("wait needs milliseconds, got {rest}"))?;
            Ok(Command::Wait(ms))
        }
        "move" => {
            let mut parts = rest.split_whitespace();
            let x = parts
                .next()
                .ok_or("move needs x y")?
                .parse::<u16>()
                .map_err(|_| "move x is not a tile coordinate".to_owned())?;
            let y = parts
                .next()
                .ok_or("move needs x y")?
                .parse::<u16>()
                .map_err(|_| "move y is not a tile coordinate".to_owned())?;
            if parts.next().is_some() {
                return Err("move takes only x y".to_owned());
            }
            Ok(Command::Move { x, y })
        }
        "click" => parse_click(rest, ClickKind::Left),
        "rclick" => parse_click(rest, ClickKind::Right),
        "dclick" => parse_click(rest, ClickKind::Double),
        "key" => {
            let key = parse_key_spec(rest)?;
            Ok(Command::Key {
                code: key.code,
                alt: key.alt,
                ctrl: key.ctrl,
                shift: key.shift,
            })
        }
        "dump" => Ok(Command::Dump(DumpPath::parse(rest)?)),
        "quit" => {
            if !rest.is_empty() {
                return Err("quit takes no arguments".to_owned());
            }
            Ok(Command::Quit)
        }
        other => Err(format!("unknown command {other}")),
    }
}

fn parse_window(name: &str, open: bool) -> Result<Command, String> {
    if name.is_empty() {
        return Err("window class required".to_owned());
    }
    let Some(class) = crate::autopilot::events::parse_window_class(name) else {
        return Err(format!("unknown window class {name}"));
    };
    let command = window_command(class, open)?;
    Ok(Command::Window {
        class: command.class,
        open: command.open,
        event: command.event,
    })
}

fn parse_click(rest: &str, kind: ClickKind) -> Result<Command, String> {
    let mut parts = rest.split_whitespace();
    let x = parts
        .next()
        .ok_or("click needs x y in interface pixels")?
        .parse::<f32>()
        .map_err(|_| "click x is not a number".to_owned())?;
    let y = parts
        .next()
        .ok_or("click needs x y in interface pixels")?
        .parse::<f32>()
        .map_err(|_| "click y is not a number".to_owned())?;
    if parts.next().is_some() {
        return Err("click takes only x y".to_owned());
    }
    if !x.is_finite() || !y.is_finite() {
        return Err("click coordinates must be finite".to_owned());
    }
    Ok(Command::Click { x, y, kind })
}

fn split_head(line: &str) -> (&str, &str) {
    match line.split_once(char::is_whitespace) {
        Some((head, rest)) => (head, rest.trim()),
        None => (line, ""),
    }
}

#[cfg(test)]
mod tests {
    use super::{ClickKind, Command, DumpPath, parse_command};
    use crate::input::InputEvent;
    use crate::interface::windows::WindowClass;

    #[test]
    fn parses_minimum_commands() {
        assert!(matches!(parse_command("chat @where").unwrap(), Command::Chat(text) if text == "@where"));
        match parse_command("open Inventory").unwrap() {
            Command::Window { class, open, .. } => {
                assert_eq!(class, WindowClass::Inventory);
                assert!(open);
            }
            other => panic!("unexpected {other:?}"),
        }
        match parse_command("close Inventory").unwrap() {
            Command::Window { open, .. } => assert!(!open),
            other => panic!("unexpected {other:?}"),
        }
        assert!(matches!(
            parse_command("toggle ToggleInventoryWindow").unwrap(),
            Command::Toggle(InputEvent::ToggleInventoryWindow)
        ));
        assert!(matches!(
            parse_command("event ToggleMenuWindow").unwrap(),
            Command::Event(InputEvent::ToggleMenuWindow)
        ));
        assert!(matches!(
            parse_command(r#"event SendMessage("hello")"#).unwrap(),
            Command::Event(InputEvent::SendMessage { text }) if text == "hello"
        ));
        assert!(matches!(parse_command("wait 250").unwrap(), Command::Wait(250)));
        assert!(matches!(parse_command("move 10 20").unwrap(), Command::Move { x: 10, y: 20 }));
        assert!(matches!(
            parse_command("click 1.5 2").unwrap(),
            Command::Click { x, y, kind: ClickKind::Left } if x == 1.5 && y == 2.0
        ));
        assert!(matches!(parse_command("rclick 3 4").unwrap(), Command::Click {
            kind: ClickKind::Right,
            ..
        }));
        assert!(matches!(parse_command("dclick 3 4").unwrap(), Command::Click {
            kind: ClickKind::Double,
            ..
        }));
        assert!(matches!(parse_command("key alt+KeyE").unwrap(), Command::Key { alt: true, .. }));
        assert!(matches!(
            parse_command("dump chat_messages").unwrap(),
            Command::Dump(DumpPath::ChatMessages)
        ));
        assert!(matches!(parse_command("quit").unwrap(), Command::Quit));
    }

    #[test]
    fn rejects_login_event_and_unknown_dump() {
        assert!(parse_command("event LogIn").is_err());
        assert!(parse_command("dump secrets").is_err());
        assert!(parse_command("toggle PlayerMove").is_err());
        assert!(parse_command("nope").is_err());
    }
}
