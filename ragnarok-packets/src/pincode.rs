//! Second-password (PIN) cipher and character-server packets.
//!
//! Hercules `pincode->decrypt` (`src/char/pincode.c`) and roBrowser
//! `generateKeypad` / `encryptPincode` (`PincodeWindow.js`) share one
//! permutation. `baseSeed` is `0x881234` and `multiplier` is `0x3498`.
//! The keypad is the inverse of the server table: encrypting a digit and
//! then decrypting it returns that digit.
//!
//! Layouts follow Hercules `pincode.c` and the 20250716 length table:
//! `0x08B8`/`0x08BA` 10, `0x08BE` 14, `0x08C5` 6, `0x08BB`/`0x08BF` 8,
//! `0x08B9` 12, `0x0AE9` 13 (flag is the low byte only).

use ragnarok_macros::{CharacterServer, ClientPacket, Packet, ServerPacket};

use crate::AccountId;

/// Hercules `pincode->baseSeed`.
pub const PINCODE_BASE_SEED: u32 = 0x0088_1234;
/// Hercules `pincode->multiplier`.
pub const PINCODE_MULTIPLIER: u32 = 0x3498;
/// Digits in a Hercules PIN. The wire field is 4 bytes.
pub const PINCODE_LENGTH: usize = 4;

/// Fisher-Yates table used by Hercules `pincode_decrypt`.
///
/// `tab[encrypted_digit]` is the plaintext digit.
pub fn pincode_decrypt_table(seed: u32) -> [u8; 10] {
    let mut table = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
    let mut user_seed = seed;
    for index in 1..10 {
        user_seed = PINCODE_BASE_SEED.wrapping_add(user_seed.wrapping_mul(PINCODE_MULTIPLIER));
        let position = (user_seed % (index as u32 + 1)) as usize;
        if index != position {
            table.swap(index, position);
        }
    }
    table
}

/// roBrowser `generateKeypad`: inverse of [`pincode_decrypt_table`].
///
/// `keypad[plaintext_digit]` is the digit the client writes on the wire.
/// `keypad[location]` is also the label drawn at that pad location.
pub fn pincode_keypad(seed: u32) -> [u8; 10] {
    let table = pincode_decrypt_table(seed);
    let mut keypad = [0; 10];
    for (index, value) in table.into_iter().enumerate() {
        keypad[value as usize] = index as u8;
    }
    keypad
}

/// One step of roBrowser `advanceVisualSeed`. The encryption keypad stays put.
pub fn advance_pincode_visual_seed(seed: u32) -> u32 {
    PINCODE_BASE_SEED.wrapping_add(seed.wrapping_mul(PINCODE_MULTIPLIER))
}

/// Hercules `pincode_decrypt` for a 4-byte ASCII pin.
///
/// A byte outside `'0'..='9'` is treated as digit 0, matching the server.
pub fn decrypt_pincode(seed: u32, encrypted: [u8; PINCODE_LENGTH]) -> [u8; PINCODE_LENGTH] {
    let table = pincode_decrypt_table(seed);
    let mut plain = [0; PINCODE_LENGTH];
    for (index, byte) in encrypted.into_iter().enumerate() {
        let digit = if byte.is_ascii_digit() { byte - b'0' } else { 0 };
        plain[index] = b'0' + table[digit as usize];
    }
    plain
}

/// roBrowser `encryptPincode` for a 4-digit PIN, including leading zeros.
pub fn encrypt_pincode(keypad: &[u8; 10], pin: &str) -> Option<[u8; PINCODE_LENGTH]> {
    let bytes = pin.as_bytes();
    if bytes.len() != PINCODE_LENGTH || !bytes.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let mut encrypted = [0; PINCODE_LENGTH];
    for (index, byte) in bytes.iter().copied().enumerate() {
        encrypted[index] = b'0' + keypad[(byte - b'0') as usize];
    }
    Some(encrypted)
}

/// `CH_SECOND_PASSWD_ACK` (`0x08B8`). Check the current PIN.
#[derive(Debug, Clone, Packet, ClientPacket, CharacterServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x08B8)]
pub struct CheckSecondPasswdPacket {
    pub account_id: AccountId,
    pub pin: [u8; PINCODE_LENGTH],
}

/// `CH_MAKE_SECOND_PASSWD` (`0x08BA`). Set the first PIN.
#[derive(Debug, Clone, Packet, ClientPacket, CharacterServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x08BA)]
pub struct MakeSecondPasswdPacket {
    pub account_id: AccountId,
    pub pin: [u8; PINCODE_LENGTH],
}

/// `CH_EDIT_SECOND_PASSWD` (`0x08BE`). Change the PIN.
#[derive(Debug, Clone, Packet, ClientPacket, CharacterServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x08BE)]
pub struct EditSecondPasswdPacket {
    pub account_id: AccountId,
    pub old_pin: [u8; PINCODE_LENGTH],
    pub new_pin: [u8; PINCODE_LENGTH],
}

/// `CH_AVAILABLE_SECOND_PASSWD` (`0x08C5`). Ask the server to open the PIN
/// flow.
///
/// Hercules answers with `HC_SECOND_PASSWD_LOGIN` state "not set" (`2`).
#[derive(Debug, Clone, Packet, ClientPacket, CharacterServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x08C5)]
pub struct AvailableSecondPasswdPacket {
    pub account_id: AccountId,
}

/// `HC_MAKE_SECOND_PASSWD` (`0x08BB`).
#[derive(Debug, Clone, Packet, ServerPacket, CharacterServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x08BB)]
pub struct MakeSecondPasswdResultPacket {
    pub state: u16,
    pub seed: u32,
}

/// `HC_EDIT_SECOND_PASSWD` (`0x08BF`). The seed is a new value.
#[derive(Debug, Clone, Packet, ServerPacket, CharacterServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x08BF)]
pub struct EditSecondPasswdResultPacket {
    pub state: u16,
    pub seed: u32,
}

/// `HC_SECOND_PASSWD_LOGIN` successor used for a wrong PIN on packetver >=
/// 20180124.
///
/// Hercules writes 13 bytes: seed, account, state, and the low byte of the
/// flag.
#[derive(Debug, Clone, Packet, ServerPacket, CharacterServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0AE9)]
pub struct SecondPasswdLogin2Packet {
    pub seed: u32,
    pub account_id: AccountId,
    pub state: u16,
    pub flag: u8,
}

#[cfg(test)]
mod tests {
    use ragnarok_bytes::{ByteReader, ByteWriter};

    use super::*;
    use crate::{LoginPincodePacket, PacketExt};

    const ACCOUNT: AccountId = AccountId(0x0102_0304);

    #[test]
    fn encrypt_matches_hercules_decrypt_fixtures() {
        let cases = [
            (0x0000_0000, "0000", "1111"),
            (0x0000_0000, "0123", "1380"),
            (0x0000_0000, "9999", "4444"),
            (0x0000_0000, "2580", "8751"),
            (0x0000_0001, "0123", "1359"),
            (0x0000_FFFF, "0123", "2315"),
            (0x0000_FFFF, "2580", "1692"),
            (0x1234_5678, "0123", "1520"),
            (0x1234_5678, "2580", "2791"),
        ];

        for (seed, pin, encrypted) in cases {
            let keypad = pincode_keypad(seed);
            let wire = encrypt_pincode(&keypad, pin).unwrap();
            assert_eq!(wire, encrypted.as_bytes());
            assert_eq!(decrypt_pincode(seed, wire), pin.as_bytes());
        }
    }

    #[test]
    fn non_digit_ciphertext_decrypts_as_zero() {
        let plain = decrypt_pincode(0, *b"12a4");
        let zero_digit = pincode_decrypt_table(0)[0];
        assert_eq!(plain[2], b'0' + zero_digit);
    }

    #[test]
    fn encrypt_rejects_pins_that_are_not_four_digits() {
        let keypad = pincode_keypad(1);
        assert!(encrypt_pincode(&keypad, "123").is_none());
        assert!(encrypt_pincode(&keypad, "12345").is_none());
        assert!(encrypt_pincode(&keypad, "12a4").is_none());
    }

    #[test]
    fn check_packet_is_ten_bytes() {
        let packet = CheckSecondPasswdPacket::new(ACCOUNT, *b"1380");
        let mut writer = ByteWriter::new();
        assert_eq!(packet.packet_to_bytes(&mut writer).unwrap(), 10);
        assert_eq!(writer.as_slice(), &[0xB8, 0x08, 0x04, 0x03, 0x02, 0x01, b'1', b'3', b'8', b'0']);
    }

    #[test]
    fn make_packet_is_ten_bytes() {
        let packet = MakeSecondPasswdPacket::new(ACCOUNT, *b"2315");
        let mut writer = ByteWriter::new();
        assert_eq!(packet.packet_to_bytes(&mut writer).unwrap(), 10);
        assert_eq!(writer.as_slice()[0], 0xBA);
        assert_eq!(writer.as_slice()[1], 0x08);
        assert_eq!(&writer.as_slice()[6..], b"2315");
    }

    #[test]
    fn edit_packet_lays_old_then_new_at_offsets_6_and_10() {
        let packet = EditSecondPasswdPacket::new(ACCOUNT, *b"1380", *b"8751");
        let mut writer = ByteWriter::new();
        assert_eq!(packet.packet_to_bytes(&mut writer).unwrap(), 14);
        assert_eq!(&writer.as_slice()[0..2], &[0xBE, 0x08]);
        assert_eq!(&writer.as_slice()[2..6], &[0x04, 0x03, 0x02, 0x01]);
        assert_eq!(&writer.as_slice()[6..10], b"1380");
        assert_eq!(&writer.as_slice()[10..14], b"8751");
    }

    #[test]
    fn available_packet_is_account_only() {
        let packet = AvailableSecondPasswdPacket::new(ACCOUNT);
        let mut writer = ByteWriter::new();
        assert_eq!(packet.packet_to_bytes(&mut writer).unwrap(), 6);
        assert_eq!(writer.as_slice(), &[0xC5, 0x08, 0x04, 0x03, 0x02, 0x01]);
    }

    #[test]
    fn login_state_packet_is_seed_account_state() {
        let bytes = [0xB9, 0x08, 0x34, 0x12, 0x00, 0x00, 0x04, 0x03, 0x02, 0x01, 0x01, 0x00];
        let mut reader = ByteReader::without_metadata(&bytes);
        let packet = LoginPincodePacket::packet_from_bytes(&mut reader).unwrap();
        assert_eq!(packet.pincode_seed, 0x1234);
        assert_eq!(packet.account_id, ACCOUNT);
        assert_eq!(packet.state, 1);
    }

    #[test]
    fn make_result_is_state_then_seed() {
        let bytes = [0xBB, 0x08, 0x02, 0x00, 0x78, 0x56, 0x34, 0x12];
        let mut reader = ByteReader::without_metadata(&bytes);
        let packet = MakeSecondPasswdResultPacket::packet_from_bytes(&mut reader).unwrap();
        assert_eq!(packet.state, 2);
        assert_eq!(packet.seed, 0x1234_5678);
    }

    #[test]
    fn edit_result_is_state_then_seed() {
        let bytes = [0xBF, 0x08, 0x00, 0x00, 0xFF, 0xFF, 0x00, 0x00];
        let mut reader = ByteReader::without_metadata(&bytes);
        let packet = EditSecondPasswdResultPacket::packet_from_bytes(&mut reader).unwrap();
        assert_eq!(packet.state, 0);
        assert_eq!(packet.seed, 0xFFFF);
    }

    #[test]
    fn wrong_pin_packet_is_thirteen_bytes_with_a_one_byte_flag() {
        let bytes = [0xE9, 0x0A, 0x34, 0x12, 0x00, 0x00, 0x04, 0x03, 0x02, 0x01, 0x08, 0x00, 0x02];
        let mut reader = ByteReader::without_metadata(&bytes);
        let packet = SecondPasswdLogin2Packet::packet_from_bytes(&mut reader).unwrap();
        assert_eq!(packet.seed, 0x1234);
        assert_eq!(packet.account_id, ACCOUNT);
        assert_eq!(packet.state, 8);
        assert_eq!(packet.flag, 2);
        assert_eq!(bytes.len(), 13);
    }
}
