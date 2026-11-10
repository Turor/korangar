#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
pub enum MessageColor {
    Rgb {
        red: u8,
        green: u8,
        blue: u8,
    },
    Broadcast,
    Server,
    Error,
    Information,
    /// Nearby public speech (`0x008D`).
    Public,
    /// Private message (`0x0097` / `0x09DE`).
    Whisper,
    /// Party chat (`ZC_NOTIFY_CHAT_PARTY`).
    Party,
    /// Guild chat. The guild task pushes this when guild messages arrive.
    Guild,
}
