//! Character-server PIN notices. The cipher lives in
//! `ragnarok_packets::pincode`.

use ragnarok_packets::AccountId;

/// A PIN packet from the character server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PincodeNotice {
    /// `0x08B9`, or `0x0AE9` when `flag` is set.
    Login {
        seed: u32,
        account_id: AccountId,
        state: u16,
        /// `None` for `0x08B9`. `Some(0)` locked, `Some(2)` wrong, on `0x0AE9`.
        flag: Option<u8>,
    },
    /// `0x08BB`.
    MakeResult { state: u16, seed: u32 },
    /// `0x08BF`. `seed` replaces the keypad seed.
    EditResult { state: u16, seed: u32 },
}
