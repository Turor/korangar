//! New-item glow, Sell Junk, and item lock, headless.
//!
//! [`offline_item_flags`] feeds the same flag decisions the client match arms
//! call, drives them with autopilot events, and encodes a packet only when
//! the decision allows one. The live check is
//! [`super::LogicSession::item_flags_on_test`].

use ragnarok_bytes::ByteWriter;
use ragnarok_packets::{
    AddRodexItemPacket, AddTradeItemPacket, DropItemPacket, InventoryIndex, MoveItemToStoragePacket, PacketExt, SellItemsPacket,
    SoldItemInformation,
};

use crate::autopilot::{Command, parse_command};
use crate::input::InputEvent;
use crate::state::item_flags::{self, FlagSend, ItemFlags, ItemKey, StackRef};

/// What the offline run observed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfflineItemFlagsReport {
    /// Debug text of the injected events.
    pub scripted: Vec<String>,
    /// Glow of a stack that arrived before the login list.
    pub snapshot_glow: u8,
    /// Glow of a pickup after the login list, before hover.
    pub pickup_glow: u8,
    /// Glow of that pickup after the hover fade.
    pub faded_glow: u8,
    /// Sell cart after Sell Junk, before any confirm. Whole stacks only.
    pub sell_cart: Vec<(u16, u16)>,
    /// Bytes from drop, trade, mail, sell, and vend while the knife is locked.
    pub refused_outbound: Vec<u8>,
    /// `CZ_MOVE_ITEM_FROM_BODY_TO_STORE2` for the locked knife.
    pub storage_packet: Vec<u8>,
    pub messages: Vec<String>,
    /// The knife is still locked after its inventory index changes.
    pub followed_locked: bool,
    pub reloaded_junk: bool,
    pub reloaded_locked: bool,
    pub reloaded_new: bool,
    /// The other character's rows stayed, and they did not receive this lock.
    pub other_character_kept: bool,
}

fn encode<P: PacketExt>(packet: &P) -> Result<Vec<u8>, String> {
    let mut writer = ByteWriter::new();
    packet
        .packet_to_bytes(&mut writer)
        .map_err(|_| "packet did not encode".to_owned())?;
    Ok(writer.into_inner())
}

fn stack(index: u16, item_id: u32, name: &str, amount: u16, equipped: bool) -> StackRef {
    StackRef {
        index,
        item_id,
        refine: 0,
        cards: [0; 4],
        name: name.to_owned(),
        amount,
        equipped,
    }
}

struct Shop {
    flags: ItemFlags,
    stacks: Vec<StackRef>,
    sellable: Vec<(u16, u16)>,
    cart: Vec<(u16, u16)>,
    outbound: Vec<u8>,
    messages: Vec<String>,
}

impl Shop {
    fn stack(&self, index: u16) -> StackRef {
        self.stacks
            .iter()
            .find(|stack| stack.index == index)
            .cloned()
            .unwrap_or_else(|| stack(index, 0, "", 1, false))
    }

    /// Layout recorded the slot, then the next input frame reads that hover.
    fn hover(&mut self, index: u16) {
        let stack = self.stack(index);
        self.flags.prepare_layout();
        self.flags.note_hover_at(stack.key(), stack.index, stack.name, 0);
        self.flags.begin_input_frame();
    }

    fn push_messages(&mut self, messages: Vec<String>) {
        self.messages.extend(messages);
    }

    fn apply(&mut self, event: InputEvent) -> Result<(), String> {
        match event {
            InputEvent::ToggleHoveredJunk => {
                self.flags.toggle_hovered_junk();
            }
            InputEvent::ToggleHoveredLock => {
                self.flags.toggle_hovered_lock();
            }
            InputEvent::SellJunk => {
                let (adds, messages) = item_flags::plan_sell_junk(&self.flags, &self.stacks, &self.sellable);
                self.push_messages(messages);
                for add in adds {
                    if let Some(line) = self.cart.iter_mut().find(|line| line.0 == add.index) {
                        line.1 = add.amount;
                    } else {
                        self.cart.push((add.index, add.amount));
                    }
                }
            }
            InputEvent::ConfirmItemDrop => {
                let stack = self.stack(2);
                let attempt = item_flags::attempt_drop(&self.flags, &stack, 1);
                self.push_messages(attempt.messages);
                if let Some(FlagSend::Drop { index, amount }) = attempt.send {
                    self.outbound.extend(encode(&DropItemPacket {
                        inventory_index: InventoryIndex(index),
                        amount,
                    })?);
                }
            }
            InputEvent::TradeAddItem { index, amount } => {
                let stack = self.stack(index);
                let attempt = item_flags::attempt_trade(&self.flags, &stack, amount);
                self.push_messages(attempt.messages);
                if let Some(FlagSend::Trade { index, amount }) = attempt.send {
                    self.outbound.extend(encode(&AddTradeItemPacket {
                        index: index.saturating_add(2),
                        amount: u32::from(amount),
                    })?);
                }
            }
            InputEvent::MailAttachItem { index, amount } => {
                let count = amount.unwrap_or(1);
                let stack = self.stack(index);
                let attempt = item_flags::attempt_mail(&self.flags, &stack, count);
                self.push_messages(attempt.messages);
                if let Some(FlagSend::Mail { index, count }) = attempt.send {
                    self.outbound.extend(encode(&AddRodexItemPacket {
                        index: InventoryIndex(index),
                        count,
                    })?);
                }
            }
            InputEvent::SellItems { items } => {
                let lines = items.iter().map(|item| (item.inventory_index.0, item.amount)).collect::<Vec<_>>();
                let attempt = item_flags::attempt_sell(&self.flags, &self.stacks, &lines);
                self.push_messages(attempt.messages);
                if let Some(FlagSend::Sell { .. }) = attempt.send {
                    self.outbound.extend(encode(&SellItemsPacket { items })?);
                }
            }
            _ => {}
        }
        Ok(())
    }
}

fn scripted(line: &str) -> Result<(String, InputEvent), String> {
    match parse_command(line)? {
        Command::Event(event) => Ok((format!("{event:?}"), event)),
        other => Err(format!("{line} parsed as {other:?}")),
    }
}

/// Snapshot, pickup, Sell Junk, and locked refusals. Nothing is written to
/// `client/item_flags.ron`.
pub fn offline_item_flags() -> Result<OfflineItemFlagsReport, String> {
    let lines = [
        "event ToggleHoveredJunk",
        "event ToggleHoveredLock",
        "event SellJunk",
        "event ConfirmItemDrop",
        "event TradeAddItem(index: 2, amount: 1)",
        "event MailAttachItem(2, 1)",
    ];
    let mut scripted_names = Vec::new();
    let mut parsed = Vec::new();
    for line in lines {
        let (name, event) = scripted(line)?;
        scripted_names.push(name);
        parsed.push(event);
    }

    let potion = stack(1, 501, "Red Potion", 10, false);
    let knife = stack(2, 1201, "Knife", 1, false);
    let jellopy = stack(3, 909, "Jellopy", 5, false);
    let armor = stack(4, 2301, "Armor", 1, true);
    let apple = stack(5, 512, "Apple", 3, false);

    let mut shop = Shop {
        flags: ItemFlags::default(),
        stacks: vec![potion.clone(), knife.clone(), jellopy.clone(), armor.clone(), apple.clone()],
        sellable: vec![(1, 10), (2, 1), (4, 1), (5, 3)],
        cart: Vec::new(),
        outbound: Vec::new(),
        messages: Vec::new(),
    };

    shop.flags.note_arrival(potion.key());
    let snapshot_glow = shop.flags.glow_alpha_at(potion.key(), 0);
    shop.flags.note_snapshot();
    shop.flags.note_arrival(jellopy.key());
    let pickup_glow = shop.flags.glow_alpha_at(jellopy.key(), 0);
    shop.flags.note_hover_at(jellopy.key(), jellopy.index, jellopy.name.clone(), 1_000);
    shop.flags.advance_to(1_300);
    let faded_glow = shop.flags.glow_alpha_at(jellopy.key(), 1_300);
    shop.flags.note_arrival(apple.key());

    shop.hover(potion.index);
    shop.apply(parsed[0].clone())?;
    shop.hover(knife.index);
    shop.apply(parsed[0].clone())?;
    shop.hover(armor.index);
    shop.apply(parsed[0].clone())?;
    shop.apply(parsed[2].clone())?;
    let sell_cart = shop.cart.clone();

    shop.hover(knife.index);
    shop.apply(parsed[1].clone())?;
    shop.apply(parsed[2].clone())?;
    shop.apply(parsed[3].clone())?;
    shop.apply(parsed[4].clone())?;
    shop.apply(parsed[5].clone())?;
    let sell_lines = shop
        .cart
        .iter()
        .map(|(index, amount)| SoldItemInformation {
            inventory_index: InventoryIndex(*index),
            amount: *amount,
        })
        .collect();
    shop.apply(InputEvent::SellItems { items: sell_lines })?;
    let vend = item_flags::attempt_vend(&shop.flags, &shop.stack(knife.index));
    shop.push_messages(vend.messages);
    let refused_outbound = shop.outbound.clone();

    let storage = item_flags::attempt_store(&shop.stack(knife.index), 1);
    let storage_packet = match storage.send {
        Some(FlagSend::Store { index, amount }) => encode(&MoveItemToStoragePacket {
            inventory_index: InventoryIndex(index),
            amount: u32::from(amount),
        })?,
        _ => Vec::new(),
    };

    let moved = stack(40, 1201, "Knife", 1, false);
    let followed_locked = item_flags::attempt_drop(&shop.flags, &moved, 1)
        .messages
        .iter()
        .any(|message| message == "Knife is locked");

    shop.flags.assign_character(4);
    let previous = "(characters: [(character_id: 9, items: [(item_id: 700, refine: 0, cards: (0, 0, 0, 0), junk: true, locked: false, \
                    is_new: false)])])";
    let text = shop.flags.save_ron(previous);
    let mut loaded = ItemFlags::default();
    loaded.load_ron(4, &text);
    let mut other = ItemFlags::default();
    other.load_ron(9, &text);

    Ok(OfflineItemFlagsReport {
        scripted: scripted_names,
        snapshot_glow,
        pickup_glow,
        faded_glow,
        sell_cart,
        refused_outbound,
        storage_packet,
        messages: shop.messages,
        followed_locked,
        reloaded_junk: loaded.is_junk(ItemKey::new(501, 0, [0; 4])),
        reloaded_locked: loaded.is_locked(ItemKey::new(1201, 0, [0; 4])),
        reloaded_new: loaded.is_new(ItemKey::new(512, 0, [0; 4])),
        other_character_kept: other.is_junk(ItemKey::new(700, 0, [0; 4])) && !other.is_locked(ItemKey::new(1201, 0, [0; 4])),
    })
}
