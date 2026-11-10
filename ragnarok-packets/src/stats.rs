//! Wire helpers for status values.
//!
//! `ZC_LONGLONGPAR_CHANGE` (`0x0ACB`) is `u16` id + `i64` amount.
//! `ZC_PAR_CHANGE` (`0x00B0`) is `u16` id + `u32` amount.
//! `ZC_COUPLESTATUS` (`0x0141`) is `u32` id + `i32` base + `i32` bonus. The
//! stat reader has already consumed the low 16 bits of that id.

use ragnarok_bytes::{ByteReader, ConversionResult, FromBytes};

pub(crate) fn read_wide_value(byte_reader: &mut ByteReader) -> ConversionResult<u64> {
    let save = byte_reader.create_save_point();
    match u64::from_bytes(byte_reader) {
        Ok(value) => Ok(value),
        Err(_) => {
            byte_reader.restore_save_point(save);
            u32::from_bytes(byte_reader).map(u64::from)
        }
    }
}

pub(crate) fn read_couple_status(byte_reader: &mut ByteReader) -> ConversionResult<(i32, i32)> {
    let _status_id_high = u16::from_bytes(byte_reader)?;
    let base = i32::from_bytes(byte_reader)?;
    let bonus = i32::from_bytes(byte_reader)?;
    Ok((base, bonus))
}

#[cfg(test)]
mod tests {
    use ragnarok_bytes::{ByteReader, ByteWriter};

    use crate::{
        PacketExt, RequestStatUpPacket, RequestTraitStatUpPacket, StatType, StatUpType, UpdateStatPacket, UpdateStatPacket1,
        UpdateStatPacket2,
    };

    fn read_packet<T: PacketExt>(bytes: &[u8]) -> T {
        let mut reader = ByteReader::without_metadata(bytes);
        T::packet_from_bytes(&mut reader).unwrap()
    }

    #[test]
    fn long_parameter_keeps_64_bit_experience_and_zeny() {
        let exp = 9_000_000_000_000u64;
        let mut exp_bytes = vec![0xCB, 0x0A, 0x01, 0x00];
        exp_bytes.extend_from_slice(&exp.to_le_bytes());
        let packet = read_packet::<UpdateStatPacket2>(&exp_bytes);
        assert_eq!(packet.stat_type, StatType::BaseExperience(exp));

        let zeny = 5_000_000_000u64;
        let mut zeny_bytes = vec![0xCB, 0x0A, 20, 0x00];
        zeny_bytes.extend_from_slice(&zeny.to_le_bytes());
        let packet = read_packet::<UpdateStatPacket2>(&zeny_bytes);
        assert_eq!(packet.stat_type, StatType::Zeny(zeny));
    }

    #[test]
    fn short_zeny_packet_still_reads_u32() {
        let packet = read_packet::<UpdateStatPacket>(&[0xB0, 0x00, 20, 0x00, 0xE8, 0x03, 0x00, 0x00]);
        assert_eq!(packet.stat_type, StatType::Zeny(1000));
    }

    #[test]
    fn couple_status_reads_trait_base_and_signed_bonus() {
        // 0x0141, SP_POW as u32, base 110, bonus -3.
        let bytes = [0x41, 0x01, 0xDB, 0x00, 0x00, 0x00, 0x6E, 0x00, 0x00, 0x00, 0xFD, 0xFF, 0xFF, 0xFF];
        let packet = read_packet::<UpdateStatPacket1>(&bytes);
        assert_eq!(packet.stat_type, StatType::Power(110, -3));
    }

    #[test]
    fn base_stat_up_encodes_00bb() {
        let packet = RequestStatUpPacket::new(StatUpType::Dexterity { amount: 3 });
        let mut writer = ByteWriter::new();
        let size = packet.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(size, 5);
        assert_eq!(writer.as_slice(), &[0xBB, 0x00, 17, 0x00, 3]);
    }

    #[test]
    fn trait_stat_up_encodes_0b24() {
        let stat = StatUpType::Power { amount: 2 };
        let packet = RequestTraitStatUpPacket::from_stat(&stat).unwrap();
        assert!(stat.is_trait());
        assert_eq!(stat.status_id(), 219);
        let mut writer = ByteWriter::new();
        let size = packet.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(size, 6);
        assert_eq!(writer.as_slice(), &[0x24, 0x0B, 0xDB, 0x00, 0x02, 0x00]);
        assert!(RequestTraitStatUpPacket::from_stat(&StatUpType::Strength { amount: 1 }).is_none());
    }
}
