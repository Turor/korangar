//! Storage, cart and trade amount prompt, headless.
//!
//! [`offline_item_moves`] runs the prompt the way the client does for each
//! drop target: a 30-stack asks how many, the typed amount is checked, and
//! the request that goes out is encoded to wire bytes with `ragnarok-packets`.
//! The live check is [`super::LogicSession::item_move_prompts_on_test`].

use ragnarok_bytes::ByteWriter;
use ragnarok_packets::{
    AddTradeItemPacket, InventoryIndex, MoveItemFromBodyToCartPacket, MoveItemFromCartToBodyPacket, MoveItemFromStoragePacket,
    MoveItemToStoragePacket, PacketExt,
};

use crate::autopilot::{Command, parse_command};
use crate::state::item_move::{ItemMovePrompt, ItemMoveStep, ItemMoveTarget};

/// One drop target's run through the prompt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemMoveCase {
    pub target: ItemMoveTarget,
    pub prompt: String,
    /// Amount text the window starts with.
    pub default_amount: String,
    /// What a bad amount ("31") answered.
    pub rejected: String,
    /// The request for 12 of index 9.
    pub request: Vec<u8>,
}

/// What the offline run observed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfflineItemMoveReport {
    /// Debug names of the events the autopilot lines produced.
    pub scripted: Vec<String>,
    pub cases: Vec<ItemMoveCase>,
    /// A single knife dropped on storage sends at once.
    pub single: ItemMoveStep,
    /// The request for that knife.
    pub single_request: Vec<u8>,
    /// Cancel leaves nothing to confirm.
    pub cancelled_cleanly: bool,
}

fn encode<P: PacketExt>(packet: &P) -> Result<Vec<u8>, String> {
    let mut writer = ByteWriter::new();
    packet
        .packet_to_bytes(&mut writer)
        .map_err(|_| "packet did not encode".to_owned())?;
    Ok(writer.into_inner())
}

/// The bytes `Client::send_item_move` puts on the wire for this move.
pub fn item_move_request(target: ItemMoveTarget, index: u16, amount: u16) -> Result<Vec<u8>, String> {
    let index_packet = InventoryIndex(index);
    match target {
        ItemMoveTarget::ToStorage => encode(&MoveItemToStoragePacket {
            inventory_index: index_packet,
            amount: u32::from(amount),
        }),
        ItemMoveTarget::FromStorage => encode(&MoveItemFromStoragePacket {
            storage_index: index_packet,
            amount: u32::from(amount),
        }),
        ItemMoveTarget::ToCart => encode(&MoveItemFromBodyToCartPacket {
            index: index_packet,
            amount: i32::from(amount),
        }),
        ItemMoveTarget::FromCart => encode(&MoveItemFromCartToBodyPacket {
            index: index_packet,
            amount: i32::from(amount),
        }),
        // `NetworkingSystem::add_trade_item` adds the +2 itself.
        ItemMoveTarget::ToTrade => encode(&AddTradeItemPacket {
            index: index.saturating_add(2),
            amount: u32::from(amount),
        }),
    }
}

fn script(line: &str) -> Result<String, String> {
    match parse_command(line)? {
        Command::Event(event) | Command::Toggle(event) => Ok(format!("{event:?}")),
        _ => Err(format!("{line} is not an event")),
    }
}

/// Run every drop target through the prompt against encoded requests.
pub fn offline_item_moves() -> Result<OfflineItemMoveReport, String> {
    let scripted = ["event ConfirmItemMove", "event CancelItemMove"]
        .iter()
        .map(|line| script(line))
        .collect::<Result<Vec<_>, _>>()?;

    let targets = [
        ItemMoveTarget::ToStorage,
        ItemMoveTarget::FromStorage,
        ItemMoveTarget::ToCart,
        ItemMoveTarget::FromCart,
        ItemMoveTarget::ToTrade,
    ];
    let mut cases = Vec::new();
    for target in targets {
        let mut prompt = ItemMovePrompt::default();
        if prompt.offer(target, 9, 30, "Red Potion") != ItemMoveStep::AskAmount {
            return Err(format!("{target:?}: a stack of 30 did not ask"));
        }
        let text = prompt.prompt.clone();
        let default_amount = prompt.amount_text.clone();
        prompt.amount_text = "31".to_owned();
        let rejected = prompt.confirm().err().ok_or("31 of 30 was accepted")?;
        prompt.amount_text = "12".to_owned();
        let (confirmed, index, amount) = prompt.confirm()?;
        if confirmed != target {
            return Err(format!("{target:?} confirmed as {confirmed:?}"));
        }
        cases.push(ItemMoveCase {
            target,
            prompt: text,
            default_amount,
            rejected,
            request: item_move_request(target, index, amount)?,
        });
    }

    let mut prompt = ItemMovePrompt::default();
    let single = prompt.offer(ItemMoveTarget::ToStorage, 4, 1, "Knife");
    let single_request = match single {
        ItemMoveStep::Send { target, index, amount } => item_move_request(target, index, amount)?,
        _ => Vec::new(),
    };

    prompt.offer(ItemMoveTarget::ToCart, 2, 5, "Jellopy");
    prompt.cancel();
    let cancelled_cleanly = !prompt.is_asking() && prompt.confirm().is_err();

    Ok(OfflineItemMoveReport {
        scripted,
        cases,
        single,
        single_request,
        cancelled_cleanly,
    })
}
