//! NPC barter shops for PACKETVER 20250716.
//!
//! Hercules `packets_struct.h` (compiled as 20250716 MAIN, so
//! `PACKETVER_MAIN_NUM >= 20250402`) sends the catalog on `0x0B78` and the
//! expanded catalog on `0x0B79`. The older `0x0B0E` and `0x0B56` layouts are
//! still accepted. Purchase and close stay on `0x0B0F`, `0x0B12`, `0x0B57`,
//! and `0x0B58`. The buy result for both barter shops is
//! `ZC_PC_PURCHASE_RESULT` (`0x00CA`), not the market ack `0x0B4E`.

use ragnarok_bytes::{
    ByteConvertable, ByteReader, ByteWriter, ConversionError, ConversionResult, ConversionResultExt, FixedByteSize, FromBytes, ToBytes,
};

use crate::{ClientPacket, InventoryIndex, ItemId, MapServer, MapServerPacket, Packet, PacketHeader, ServerPacket};

/// One `PACKET_ZC_NPC_BARTER_MARKET_ITEMINFO_sub` row.
///
/// `0x0B78` rows are 31 bytes (view sprite and equip location). `0x0B0E` rows
/// are 25 bytes and leave those two fields at zero. `amount == u32::MAX` is
/// an unlimited stock (`qty == -1` on the server).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
pub struct BarterShopItem {
    pub item_id: ItemId,
    pub item_type: u8,
    pub amount: u32,
    pub currency_id: ItemId,
    pub currency_amount: u32,
    pub weight: u32,
    pub shop_index: u32,
    pub view_sprite: u16,
    pub location: u32,
}

/// `ZC_NPC_BARTER_MARKET_ITEMINFO` for PACKETVER before 20210203 (`0x0B0E`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
pub struct BarterShopPacket {
    pub items: Vec<BarterShopItem>,
}

/// `ZC_NPC_BARTER_MARKET_ITEMINFO` for PACKETVER 20250716 MAIN (`0x0B78`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
pub struct BarterShop2021Packet {
    pub items: Vec<BarterShopItem>,
}

/// One currency inside an expanded barter row. 12 bytes when item ids are
/// `u32`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
pub struct ExpandedBarterCurrency {
    pub item_id: ItemId,
    pub refine_level: u16,
    pub amount: u32,
    pub item_type: u16,
}

/// One expanded barter row. `refine_level` is the reward refine from the
/// 20250402 layout and is zero on the older `0x0B56` packet.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
pub struct ExpandedBarterItem {
    pub item_id: ItemId,
    pub item_type: u16,
    pub amount: u32,
    pub weight: u32,
    pub shop_index: u32,
    pub zeny: u32,
    pub view_sprite: u16,
    pub location: u32,
    pub refine_level: u32,
    pub currencies: Vec<ExpandedBarterCurrency>,
}

/// `ZC_NPC_EXPANDED_BARTER_MARKET_ITEMINFO` (`0x0B56`), 26-byte row header.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
pub struct ExpandedBarterShopPacket {
    pub items: Vec<ExpandedBarterItem>,
}

/// `ZC_NPC_EXPANDED_BARTER_MARKET_ITEMINFO` (`0x0B79`) with the 20250402
/// reward refine field. Row header is 36 bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
pub struct ExpandedBarterShop2025Packet {
    pub items: Vec<ExpandedBarterItem>,
}

/// One `CZ_NPC_BARTER_MARKET_PURCHASE` line (`0x0B0F`). 14 bytes.
/// `inventory_index` is written as the server slot plus 2.
#[derive(Debug, Clone, PartialEq, Eq, ByteConvertable, FixedByteSize)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
pub struct BarterPurchaseEntry {
    pub item_id: ItemId,
    pub amount: u32,
    pub inventory_index: InventoryIndex,
    pub shop_index: u32,
}

/// `CZ_NPC_BARTER_MARKET_PURCHASE` (`0x0B0F`).
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0B0F)]
#[variable_length]
pub struct BarterPurchasePacket {
    #[repeating_remaining]
    pub items: Vec<BarterPurchaseEntry>,
}

/// `CZ_NPC_BARTER_MARKET_CLOSE` (`0x0B12`).
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0B12)]
pub struct CloseBarterPacket {}

/// One `CZ_NPC_EXPANDED_BARTER_MARKET_PURCHASE` line (`0x0B57`). 12 bytes.
/// The server finds the materials; there is no inventory index.
#[derive(Debug, Clone, PartialEq, Eq, ByteConvertable, FixedByteSize)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
pub struct ExpandedBarterPurchaseEntry {
    pub item_id: ItemId,
    pub shop_index: u32,
    pub amount: u32,
}

/// `CZ_NPC_EXPANDED_BARTER_MARKET_PURCHASE` (`0x0B57`).
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0B57)]
#[variable_length]
pub struct ExpandedBarterPurchasePacket {
    #[repeating_remaining]
    pub items: Vec<ExpandedBarterPurchaseEntry>,
}

/// `CZ_NPC_EXPANDED_BARTER_MARKET_CLOSE` (`0x0B58`).
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0B58)]
pub struct CloseExpandedBarterPacket {}

/// `ZC_PC_PURCHASE_RESULT` (`0x00CA`). Barter success is `12`.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x00CA)]
pub struct NpcBuyResultPacket {
    pub result: u8,
}

const SIMPLE_ROW_2019: usize = 25;
const SIMPLE_ROW_2021: usize = 31;
const EXPANDED_HEAD_2019: usize = 26;
const EXPANDED_HEAD_2025: usize = 36;
const CURRENCY_ROW: usize = 12;
const MAX_BARTER_ITEMS: usize = 128;
const MAX_CURRENCIES: usize = 16;

fn read_simple_item(reader: &mut ByteReader<'_>, with_view: bool) -> ConversionResult<BarterShopItem> {
    let item_id = ItemId::from_bytes(reader).trace::<BarterShopItem>()?;
    let item_type = u8::from_bytes(reader).trace::<BarterShopItem>()?;
    let amount = u32::from_bytes(reader).trace::<BarterShopItem>()?;
    let currency_id = ItemId::from_bytes(reader).trace::<BarterShopItem>()?;
    let currency_amount = u32::from_bytes(reader).trace::<BarterShopItem>()?;
    let weight = u32::from_bytes(reader).trace::<BarterShopItem>()?;
    let shop_index = u32::from_bytes(reader).trace::<BarterShopItem>()?;
    let (view_sprite, location) = if with_view {
        (
            u16::from_bytes(reader).trace::<BarterShopItem>()?,
            u32::from_bytes(reader).trace::<BarterShopItem>()?,
        )
    } else {
        (0, 0)
    };
    Ok(BarterShopItem {
        item_id,
        item_type,
        amount,
        currency_id,
        currency_amount,
        weight,
        shop_index,
        view_sprite,
        location,
    })
}

fn write_simple_item(item: &BarterShopItem, writer: &mut ByteWriter, with_view: bool) -> ConversionResult<usize> {
    let mut written = 0;
    written += item.item_id.to_bytes(writer)?;
    written += item.item_type.to_bytes(writer)?;
    written += item.amount.to_bytes(writer)?;
    written += item.currency_id.to_bytes(writer)?;
    written += item.currency_amount.to_bytes(writer)?;
    written += item.weight.to_bytes(writer)?;
    written += item.shop_index.to_bytes(writer)?;
    if with_view {
        written += item.view_sprite.to_bytes(writer)?;
        written += item.location.to_bytes(writer)?;
    }
    Ok(written)
}

fn read_simple_list(byte_reader: &mut ByteReader<'_>, row_size: usize, with_view: bool) -> ConversionResult<Vec<BarterShopItem>> {
    let packet_length = u16::from_bytes(byte_reader).trace::<BarterShopPacket>()? as usize;
    if packet_length < 4 {
        return Err(ConversionError::from_message("barter shop packet is shorter than its header"));
    }
    let body = packet_length - 4;
    if !body.is_multiple_of(row_size) {
        return Err(ConversionError::from_message("barter shop length does not match its rows"));
    }
    let count = body / row_size;
    if count > MAX_BARTER_ITEMS {
        return Err(ConversionError::from_message("barter shop has too many items"));
    }
    let mut items = Vec::with_capacity(count);
    for _ in 0..count {
        items.push(read_simple_item(byte_reader, with_view)?);
    }
    if !byte_reader.is_empty() {
        return Err(ConversionError::from_message("barter shop has trailing bytes"));
    }
    Ok(items)
}

fn write_simple_list(items: &[BarterShopItem], byte_writer: &mut ByteWriter, with_view: bool) -> ConversionResult<usize> {
    if items.len() > MAX_BARTER_ITEMS {
        return Err(ConversionError::from_message("barter shop has too many items"));
    }
    let start = byte_writer.len();
    0u16.to_bytes(byte_writer)?;
    for item in items {
        write_simple_item(item, byte_writer, with_view)?;
    }
    let written = byte_writer.len() - start;
    let packet_length = (written + 2) as u16;
    byte_writer.overwrite_at(start, packet_length.to_le_bytes())?;
    Ok(written)
}

fn read_currency(reader: &mut ByteReader<'_>) -> ConversionResult<ExpandedBarterCurrency> {
    Ok(ExpandedBarterCurrency {
        item_id: ItemId::from_bytes(reader).trace::<ExpandedBarterCurrency>()?,
        refine_level: u16::from_bytes(reader).trace::<ExpandedBarterCurrency>()?,
        amount: u32::from_bytes(reader).trace::<ExpandedBarterCurrency>()?,
        item_type: u16::from_bytes(reader).trace::<ExpandedBarterCurrency>()?,
    })
}

fn write_currency(currency: &ExpandedBarterCurrency, writer: &mut ByteWriter) -> ConversionResult<usize> {
    let mut written = 0;
    written += currency.item_id.to_bytes(writer)?;
    written += currency.refine_level.to_bytes(writer)?;
    written += currency.amount.to_bytes(writer)?;
    written += currency.item_type.to_bytes(writer)?;
    if written != CURRENCY_ROW {
        return Err(ConversionError::from_message("barter currency row has the wrong size"));
    }
    Ok(written)
}

fn read_expanded_item(reader: &mut ByteReader<'_>, with_view_and_refine: bool) -> ConversionResult<ExpandedBarterItem> {
    let item_id = ItemId::from_bytes(reader).trace::<ExpandedBarterItem>()?;
    let item_type = u16::from_bytes(reader).trace::<ExpandedBarterItem>()?;
    let amount = u32::from_bytes(reader).trace::<ExpandedBarterItem>()?;
    let weight = u32::from_bytes(reader).trace::<ExpandedBarterItem>()?;
    let shop_index = u32::from_bytes(reader).trace::<ExpandedBarterItem>()?;
    let zeny = u32::from_bytes(reader).trace::<ExpandedBarterItem>()?;
    let (view_sprite, location) = if with_view_and_refine {
        (
            u16::from_bytes(reader).trace::<ExpandedBarterItem>()?,
            u32::from_bytes(reader).trace::<ExpandedBarterItem>()?,
        )
    } else {
        (0, 0)
    };
    let currency_count = u32::from_bytes(reader).trace::<ExpandedBarterItem>()?;
    let refine_level = if with_view_and_refine {
        u32::from_bytes(reader).trace::<ExpandedBarterItem>()?
    } else {
        0
    };
    if currency_count as usize > MAX_CURRENCIES {
        return Err(ConversionError::from_message("expanded barter row has too many currencies"));
    }
    let mut currencies = Vec::with_capacity(currency_count as usize);
    for _ in 0..currency_count {
        currencies.push(read_currency(reader)?);
    }
    Ok(ExpandedBarterItem {
        item_id,
        item_type,
        amount,
        weight,
        shop_index,
        zeny,
        view_sprite,
        location,
        refine_level,
        currencies,
    })
}

fn write_expanded_item(item: &ExpandedBarterItem, writer: &mut ByteWriter, with_view_and_refine: bool) -> ConversionResult<usize> {
    if item.currencies.len() > MAX_CURRENCIES {
        return Err(ConversionError::from_message("expanded barter row has too many currencies"));
    }
    let mut written = 0;
    written += item.item_id.to_bytes(writer)?;
    written += item.item_type.to_bytes(writer)?;
    written += item.amount.to_bytes(writer)?;
    written += item.weight.to_bytes(writer)?;
    written += item.shop_index.to_bytes(writer)?;
    written += item.zeny.to_bytes(writer)?;
    if with_view_and_refine {
        written += item.view_sprite.to_bytes(writer)?;
        written += item.location.to_bytes(writer)?;
    }
    written += (item.currencies.len() as u32).to_bytes(writer)?;
    if with_view_and_refine {
        written += item.refine_level.to_bytes(writer)?;
    }
    for currency in &item.currencies {
        written += write_currency(currency, writer)?;
    }
    let fixed = written - item.currencies.len() * CURRENCY_ROW;
    let expected = if with_view_and_refine {
        EXPANDED_HEAD_2025
    } else {
        EXPANDED_HEAD_2019
    };
    if fixed != expected {
        return Err(ConversionError::from_message("expanded barter row header has the wrong size"));
    }
    Ok(written)
}

fn read_expanded_list(byte_reader: &mut ByteReader<'_>, with_view_and_refine: bool) -> ConversionResult<Vec<ExpandedBarterItem>> {
    let start = byte_reader.get_offset();
    let packet_length = u16::from_bytes(byte_reader).trace::<ExpandedBarterShopPacket>()? as usize;
    if packet_length < 8 {
        return Err(ConversionError::from_message(
            "expanded barter packet is shorter than its header",
        ));
    }
    let items_count = i32::from_bytes(byte_reader).trace::<ExpandedBarterShopPacket>()?;
    if items_count < 0 || items_count as usize > MAX_BARTER_ITEMS {
        return Err(ConversionError::from_message("expanded barter item count is invalid"));
    }
    let mut items = Vec::with_capacity(items_count as usize);
    for _ in 0..items_count {
        items.push(read_expanded_item(byte_reader, with_view_and_refine)?);
    }
    // `packet_length` counts the opcode the dispatcher already consumed.
    if byte_reader.get_offset() - start + 2 != packet_length || !byte_reader.is_empty() {
        return Err(ConversionError::from_message(
            "expanded barter packet length does not match its body",
        ));
    }
    Ok(items)
}

fn write_expanded_list(items: &[ExpandedBarterItem], byte_writer: &mut ByteWriter, with_view_and_refine: bool) -> ConversionResult<usize> {
    if items.len() > MAX_BARTER_ITEMS {
        return Err(ConversionError::from_message("expanded barter item count is invalid"));
    }
    let start = byte_writer.len();
    0u16.to_bytes(byte_writer)?;
    (items.len() as i32).to_bytes(byte_writer)?;
    for item in items {
        write_expanded_item(item, byte_writer, with_view_and_refine)?;
    }
    let written = byte_writer.len() - start;
    let packet_length = (written + 2) as u16;
    byte_writer.overwrite_at(start, packet_length.to_le_bytes())?;
    Ok(written)
}

macro_rules! packet_element {
    () => {
        #[cfg(feature = "packet-to-state-element")]
        fn to_element<App: korangar_interface::application::Application>(
            self_path: impl rust_state::Path<App, Self>,
            name: String,
        ) -> Box<dyn korangar_interface::element::Element<App, LayoutInfo = ()>> {
            korangar_interface::element::ErasedElement::new(<Self as korangar_interface::element::StateElement<App>>::to_element(
                self_path, name,
            ))
        }
    };
}

impl Packet for BarterShopPacket {
    const HEADER: PacketHeader = PacketHeader(0x0B0E);
    const IS_PING: bool = false;

    packet_element!();

    fn payload_from_bytes(byte_reader: &mut ByteReader) -> ConversionResult<Self> {
        Ok(Self {
            items: read_simple_list(byte_reader, SIMPLE_ROW_2019, false)?,
        })
    }

    fn payload_to_bytes(&self, byte_writer: &mut ByteWriter) -> ConversionResult<usize> {
        write_simple_list(&self.items, byte_writer, false)
    }
}

impl ServerPacket for BarterShopPacket {}
impl MapServerPacket for BarterShopPacket {}

impl Packet for BarterShop2021Packet {
    const HEADER: PacketHeader = PacketHeader(0x0B78);
    const IS_PING: bool = false;

    packet_element!();

    fn payload_from_bytes(byte_reader: &mut ByteReader) -> ConversionResult<Self> {
        Ok(Self {
            items: read_simple_list(byte_reader, SIMPLE_ROW_2021, true)?,
        })
    }

    fn payload_to_bytes(&self, byte_writer: &mut ByteWriter) -> ConversionResult<usize> {
        write_simple_list(&self.items, byte_writer, true)
    }
}

impl ServerPacket for BarterShop2021Packet {}
impl MapServerPacket for BarterShop2021Packet {}

impl Packet for ExpandedBarterShopPacket {
    const HEADER: PacketHeader = PacketHeader(0x0B56);
    const IS_PING: bool = false;

    packet_element!();

    fn payload_from_bytes(byte_reader: &mut ByteReader) -> ConversionResult<Self> {
        Ok(Self {
            items: read_expanded_list(byte_reader, false)?,
        })
    }

    fn payload_to_bytes(&self, byte_writer: &mut ByteWriter) -> ConversionResult<usize> {
        write_expanded_list(&self.items, byte_writer, false)
    }
}

impl ServerPacket for ExpandedBarterShopPacket {}
impl MapServerPacket for ExpandedBarterShopPacket {}

impl Packet for ExpandedBarterShop2025Packet {
    const HEADER: PacketHeader = PacketHeader(0x0B79);
    const IS_PING: bool = false;

    packet_element!();

    fn payload_from_bytes(byte_reader: &mut ByteReader) -> ConversionResult<Self> {
        Ok(Self {
            items: read_expanded_list(byte_reader, true)?,
        })
    }

    fn payload_to_bytes(&self, byte_writer: &mut ByteWriter) -> ConversionResult<usize> {
        write_expanded_list(&self.items, byte_writer, true)
    }
}

impl ServerPacket for ExpandedBarterShop2025Packet {}
impl MapServerPacket for ExpandedBarterShop2025Packet {}

#[cfg(test)]
mod tests {
    use ragnarok_bytes::{ByteReader, ByteWriter};

    use super::{
        BarterPurchaseEntry, BarterPurchasePacket, BarterShop2021Packet, BarterShopItem, BarterShopPacket, CloseBarterPacket,
        CloseExpandedBarterPacket, EXPANDED_HEAD_2019, EXPANDED_HEAD_2025, ExpandedBarterCurrency, ExpandedBarterItem,
        ExpandedBarterPurchaseEntry, ExpandedBarterPurchasePacket, ExpandedBarterShop2025Packet, ExpandedBarterShopPacket,
        NpcBuyResultPacket, SIMPLE_ROW_2019, SIMPLE_ROW_2021,
    };
    use crate::{InventoryIndex, ItemId, PacketExt};

    fn white_herb() -> BarterShopItem {
        BarterShopItem {
            item_id: ItemId(509),
            item_type: 0,
            amount: 100,
            currency_id: ItemId(501),
            currency_amount: 2,
            weight: 70,
            shop_index: 0,
            view_sprite: 0,
            location: 0,
        }
    }

    fn apple() -> BarterShopItem {
        BarterShopItem {
            item_id: ItemId(512),
            item_type: 0,
            amount: u32::MAX,
            currency_id: ItemId(502),
            currency_amount: 1,
            weight: 20,
            shop_index: 4,
            view_sprite: 3,
            location: 0,
        }
    }

    #[test]
    fn simple_barter_20250716_row_is_31_bytes() {
        let packet = BarterShop2021Packet {
            items: vec![white_herb(), apple()],
        };
        let mut writer = ByteWriter::new();
        packet.packet_to_bytes(&mut writer).unwrap();
        let bytes = writer.into_inner();
        assert_eq!(bytes.len(), 4 + SIMPLE_ROW_2021 * 2);
        assert_eq!(&bytes[..4], &[0x78, 0x0B, 0x42, 0x00]);

        let mut reader = ByteReader::without_metadata(&bytes);
        let decoded = BarterShop2021Packet::packet_from_bytes(&mut reader).unwrap();
        assert!(reader.is_empty());
        assert_eq!(decoded.items[0].item_id, ItemId(509));
        assert_eq!(decoded.items[0].currency_id, ItemId(501));
        assert_eq!(decoded.items[0].currency_amount, 2);
        assert_eq!(decoded.items[0].amount, 100);
        assert_eq!(decoded.items[0].shop_index, 0);
        assert_eq!(decoded.items[1].amount, u32::MAX);
        assert_eq!(decoded.items[1].view_sprite, 3);
        assert_eq!(decoded.items[1].shop_index, 4);
    }

    #[test]
    fn simple_barter_2019_row_is_25_bytes() {
        let mut item = white_herb();
        item.view_sprite = 9;
        item.location = 4;
        let packet = BarterShopPacket { items: vec![item] };
        let mut writer = ByteWriter::new();
        packet.packet_to_bytes(&mut writer).unwrap();
        let bytes = writer.into_inner();
        assert_eq!(bytes.len(), 4 + SIMPLE_ROW_2019);
        assert_eq!(&bytes[..2], &[0x0E, 0x0B]);

        let mut reader = ByteReader::without_metadata(&bytes);
        let decoded = BarterShopPacket::packet_from_bytes(&mut reader).unwrap();
        assert!(reader.is_empty());
        assert_eq!(decoded.items[0].item_id, ItemId(509));
        assert_eq!(decoded.items[0].view_sprite, 0);
        assert_eq!(decoded.items[0].location, 0);
        assert_eq!(decoded.items[0].currency_amount, 2);
    }

    fn orange_potion() -> ExpandedBarterItem {
        ExpandedBarterItem {
            item_id: ItemId(502),
            item_type: 0,
            amount: 100_000,
            weight: 50,
            shop_index: 0,
            zeny: 2,
            view_sprite: 7,
            location: 0,
            refine_level: 0,
            currencies: vec![
                ExpandedBarterCurrency {
                    item_id: ItemId(513),
                    refine_level: 0,
                    amount: 2,
                    item_type: 0,
                },
                ExpandedBarterCurrency {
                    item_id: ItemId(512),
                    refine_level: 0,
                    amount: 1,
                    item_type: 0,
                },
            ],
        }
    }

    #[test]
    fn expanded_barter_20250716_includes_refine_and_currencies() {
        let packet = ExpandedBarterShop2025Packet {
            items: vec![orange_potion()],
        };
        let mut writer = ByteWriter::new();
        packet.packet_to_bytes(&mut writer).unwrap();
        let bytes = writer.into_inner();
        assert_eq!(bytes.len(), 8 + EXPANDED_HEAD_2025 + 12 * 2);
        assert_eq!(&bytes[..2], &[0x79, 0x0B]);
        assert_eq!(u16::from_le_bytes([bytes[2], bytes[3]]) as usize, bytes.len());
        assert_eq!(i32::from_le_bytes(bytes[4..8].try_into().unwrap()), 1);

        let mut reader = ByteReader::without_metadata(&bytes);
        let decoded = ExpandedBarterShop2025Packet::packet_from_bytes(&mut reader).unwrap();
        assert!(reader.is_empty());
        assert_eq!(decoded.items.len(), 1);
        assert_eq!(decoded.items[0].item_id, ItemId(502));
        assert_eq!(decoded.items[0].zeny, 2);
        assert_eq!(decoded.items[0].view_sprite, 7);
        assert_eq!(decoded.items[0].refine_level, 0);
        assert_eq!(decoded.items[0].currencies.len(), 2);
        assert_eq!(decoded.items[0].currencies[0].item_id, ItemId(513));
        assert_eq!(decoded.items[0].currencies[0].amount, 2);
        assert_eq!(decoded.items[0].currencies[1].amount, 1);
    }

    #[test]
    fn expanded_barter_2019_omits_view_and_reward_refine() {
        let packet = ExpandedBarterShopPacket {
            items: vec![orange_potion()],
        };
        let mut writer = ByteWriter::new();
        packet.packet_to_bytes(&mut writer).unwrap();
        let bytes = writer.into_inner();
        assert_eq!(bytes.len(), 8 + EXPANDED_HEAD_2019 + 12 * 2);
        assert_eq!(&bytes[..2], &[0x56, 0x0B]);

        let mut reader = ByteReader::without_metadata(&bytes);
        let decoded = ExpandedBarterShopPacket::packet_from_bytes(&mut reader).unwrap();
        assert!(reader.is_empty());
        assert_eq!(decoded.items[0].zeny, 2);
        assert_eq!(decoded.items[0].view_sprite, 0);
        assert_eq!(decoded.items[0].currencies[1].item_id, ItemId(512));
    }

    #[test]
    fn purchase_and_close_match_hercules_layout() {
        let purchase = BarterPurchasePacket {
            items: vec![BarterPurchaseEntry {
                item_id: ItemId(509),
                amount: 3,
                inventory_index: InventoryIndex(4),
                shop_index: 1,
            }],
        };
        let mut writer = ByteWriter::new();
        purchase.packet_to_bytes(&mut writer).unwrap();
        let bytes = writer.into_inner();
        assert_eq!(bytes.len(), 4 + 14);
        assert_eq!(bytes, [
            0x0F, 0x0B, 18, 0, // header, length 18
            0xFD, 0x01, 0x00, 0x00, // item 509
            0x03, 0x00, 0x00, 0x00, // amount 3
            0x06, 0x00, // inventory index 4 stored as 6
            0x01, 0x00, 0x00, 0x00, // shop index 1
        ]);

        let expanded = ExpandedBarterPurchasePacket {
            items: vec![ExpandedBarterPurchaseEntry {
                item_id: ItemId(502),
                shop_index: 6,
                amount: 2,
            }],
        };
        let mut writer = ByteWriter::new();
        expanded.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 4 + 12);
        assert_eq!(&writer.as_slice()[..2], &[0x57, 0x0B]);
        assert_eq!(&writer.as_slice()[4..8], &502u32.to_le_bytes());
        assert_eq!(&writer.as_slice()[8..12], &6u32.to_le_bytes());
        assert_eq!(&writer.as_slice()[12..16], &2u32.to_le_bytes());

        let mut close = ByteWriter::new();
        CloseBarterPacket {}.packet_to_bytes(&mut close).unwrap();
        assert_eq!(close.as_slice(), &[0x12, 0x0B]);
        let mut close = ByteWriter::new();
        CloseExpandedBarterPacket {}.packet_to_bytes(&mut close).unwrap();
        assert_eq!(close.as_slice(), &[0x58, 0x0B]);
    }

    #[test]
    fn npc_buy_result_is_three_bytes() {
        let bytes = [0xCA, 0x00, 12];
        let mut reader = ByteReader::without_metadata(&bytes);
        let packet = NpcBuyResultPacket::packet_from_bytes(&mut reader).unwrap();
        assert!(reader.is_empty());
        assert_eq!(packet.result, 12);
    }

    #[test]
    fn rejected_when_the_row_size_does_not_divide() {
        let mut bytes = vec![0x78, 0x0B, 10, 0];
        bytes.extend_from_slice(&[0u8; 6]);
        let mut reader = ByteReader::without_metadata(&bytes);
        assert!(BarterShop2021Packet::packet_from_bytes(&mut reader).is_err());
    }
}
