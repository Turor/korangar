//! Right-click item menu and Alt+right-click quick-move, headless.
//!
//! The offline run opens a storage fixture, injects Alt+right-click through
//! [`InputSystem`], and applies [`react_to_secondary`] plus the client's
//! amount prompt. Confirm encodes the same bytes `Client::send_item_move`
//! sends. The live check is [`super::LogicSession::item_menu_on_test`].

use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use korangar_networking::InventoryItemDetails;
use ragnarok_bytes::ByteWriter;
use ragnarok_packets::{
    AccountId, ClientTick, EquipPosition, EquippableItemFlags, InventoryIndex, ItemId, ItemOptions, PacketExt, RegularItemFlags,
    UseItemPacket,
};
use winit::keyboard::KeyCode;

use crate::autopilot::{Command, parse_command};
use crate::graphics::Color;
use crate::input::{InputEvent, InputSystem};
use crate::interface::components::item_tooltip::{pinned_details_body, tooltip_text};
use crate::interface::item_menu_flow::{SlotReaction, react_to_secondary};
use crate::interface::item_move_flow::{confirm_amount, offer_amount, stack_amount};
use crate::interface::resource::ItemSource;
use crate::interface::windows::{format_item_details, item_details_key};
use crate::state::item_compare::ItemTooltipCatalog;
use crate::state::item_menu::{ItemMenuState, ItemSlotFlags};
use crate::state::item_move::{ItemMovePrompt, ItemMoveStep};
use crate::state::quick_move::{QUICK_MOVE_NO_TARGET, QuickMoveWindows, SlotSecondary, classify_secondary};
use crate::state::vn_dialog::ChoiceKeys;
use crate::world::ResourceMetadata;

/// What the offline run observed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfflineItemMenuReport {
    /// Alt+right-click through the input system classified as quick-move.
    pub injected_alt_right: bool,
    /// Plain right-click classified as the menu.
    pub injected_right: bool,
    /// Debug text of the autopilot lines.
    pub scripted: Vec<String>,
    pub stack_prompt: String,
    pub stack_default: String,
    /// `0x0364` for 30 of index 9 after confirming the prefilled amount.
    pub stack_request: Vec<u8>,
    /// `0x0364` for one knife, sent without a prompt.
    pub knife_request: Vec<u8>,
    /// Gear is a stack of 1, so it also sends at once.
    pub gear_request: Vec<u8>,
    /// Only the cart is open: `0x0126` for the knife.
    pub cart_request: Vec<u8>,
    pub no_target_hint: String,
    /// Bytes the no-target click caused the handler to send.
    pub no_target_request: Vec<u8>,
    pub menu_labels: Vec<String>,
    /// `0x0439` produced by picking Use.
    pub use_request: Vec<u8>,
    /// Details opened a pinned window.
    pub details_opened: bool,
    /// Body of that window, including Item Finder lines.
    pub details_body: String,
    /// Arrow down then Enter picks this row.
    pub keyboard_pick: usize,
    /// Picking Link to chat queued `LinkItemInChat`.
    pub link_queued: bool,
}

fn encode<P: PacketExt>(packet: &P) -> Result<Vec<u8>, String> {
    let mut writer = ByteWriter::new();
    packet
        .packet_to_bytes(&mut writer)
        .map_err(|_| "packet did not encode".to_owned())?;
    Ok(writer.into_inner())
}

fn script(line: &str) -> Result<String, String> {
    match parse_command(line)? {
        Command::Event(event) | Command::Toggle(event) => Ok(format!("{event:?}")),
        _ => Err(format!("{line} is not an event")),
    }
}

struct Fixture {
    input: InputSystem,
    tick: u32,
    inventory: Vec<korangar_networking::InventoryItem<ResourceMetadata>>,
    windows: QuickMoveWindows,
    prompt: ItemMovePrompt,
    sent: Vec<Vec<u8>>,
    hints: Vec<String>,
    events: Vec<InputEvent>,
    menu: ItemMenuState,
    details: Vec<(u64, String, String)>,
    catalog: ItemTooltipCatalog,
    gold: Color,
    accent: Color,
}

impl Fixture {
    fn new() -> Self {
        let mut catalog = ItemTooltipCatalog::default();
        catalog.insert_item(501, "Red Potion".to_owned(), 0, 7, 0, "Every Job".to_owned());
        Self {
            input: InputSystem::new(Arc::new(AtomicU64::new(0))),
            tick: 0,
            inventory: Vec::new(),
            windows: QuickMoveWindows::default(),
            prompt: ItemMovePrompt::default(),
            sent: Vec::new(),
            hints: Vec::new(),
            events: Vec::new(),
            menu: ItemMenuState::default(),
            details: Vec::new(),
            catalog,
            gold: Color::rgb_u8(200, 162, 90),
            accent: Color::rgb_u8(232, 213, 163),
        }
    }

    fn insert(&mut self, item: korangar_networking::InventoryItem<ResourceMetadata>) {
        self.inventory.push(item);
    }

    fn take_sent(&mut self) -> Vec<u8> {
        self.sent.pop().unwrap_or_default()
    }

    fn inject(&mut self, alt: bool) -> (bool, bool) {
        self.input.expire_synthesized();
        self.input.advance_synthetic_drag();
        // Releasing a button that was never down marks a press. Flush that
        // frame before the click, and stay outside the double-click window.
        self.tick = self.tick.wrapping_add(1_000);
        let _ = self.input.update_delta(ClientTick(self.tick));
        if alt {
            self.input.synthesize_key(KeyCode::AltLeft, false, false, false);
        }
        self.input
            .synthesize_click(20.0, 20.0, korangar_interface::layout::MouseButton::Right);
        self.tick = self.tick.wrapping_add(1_000);
        let report = self.input.update_delta(ClientTick(self.tick));
        let right = report.mouse_click == Some(korangar_interface::layout::MouseButton::Right);
        let alt_down = self.input.key_down(KeyCode::AltLeft);
        (right, alt_down)
    }

    /// Inject the click, then run the same reaction the client runs.
    fn click_slot(&mut self, index: InventoryIndex, alt: bool) -> Result<SlotSecondary, String> {
        let (right, alt_down) = self.inject(alt);
        let kind = classify_secondary(right, alt_down);
        let item = self
            .inventory
            .iter()
            .find(|item| item.index == index)
            .cloned()
            .ok_or_else(|| format!("no item at {}", index.0))?;
        let alt = match kind {
            SlotSecondary::QuickMove => true,
            SlotSecondary::OpenMenu => false,
            SlotSecondary::Ignore => return Ok(kind),
        };
        let reaction = react_to_secondary(
            ItemSource::Inventory,
            &item,
            &item.metadata.name,
            alt,
            self.windows,
            false,
            ItemSlotFlags::default(),
        );
        self.apply_reaction(reaction, &item)?;
        Ok(kind)
    }

    fn apply_reaction(
        &mut self,
        reaction: SlotReaction,
        item: &korangar_networking::InventoryItem<ResourceMetadata>,
    ) -> Result<(), String> {
        match reaction {
            SlotReaction::Ignored => {}
            SlotReaction::Hint(text) => self.hints.push(text.to_owned()),
            SlotReaction::Move(target) => {
                let step = offer_amount(&mut self.prompt, target, item.index.0, stack_amount(item), &item.metadata.name);
                self.note_step(step)?;
            }
            SlotReaction::Mail { index } => {
                self.events.push(InputEvent::MailAttachItem { index, amount: None });
            }
            SlotReaction::OpenMenu {
                title,
                origin,
                index,
                rows,
            } => self.menu.open(title, origin, index, rows),
        }
        Ok(())
    }

    fn note_step(&mut self, step: ItemMoveStep) -> Result<(), String> {
        if let ItemMoveStep::Send { target, index, amount } = step {
            self.sent.push(crate::e2e::item_move_request(target, index, amount)?);
        }
        Ok(())
    }

    /// The amount window's OK, through [`confirm_amount`].
    fn confirm(&mut self) -> Result<(), String> {
        let (target, index, amount) = confirm_amount(&mut self.prompt)?;
        self.sent.push(crate::e2e::item_move_request(target, index, amount)?);
        Ok(())
    }

    fn pick(&mut self, row: usize) -> Result<(), String> {
        let entry = self.menu.entry_at(row).ok_or("menu row is missing")?;
        let origin = self.menu.origin();
        let index = self.menu.index();
        let effect = crate::state::item_menu::effect_of(entry, origin);
        self.menu.close();
        let item_index = InventoryIndex(index);
        let item = self
            .inventory
            .iter()
            .find(|item| item.index == item_index)
            .cloned()
            .ok_or("picked item is not in the fixture")?;
        match effect {
            crate::state::item_menu::ItemMenuEffect::UseItem => {
                self.sent.push(encode(&UseItemPacket::new(item_index, AccountId(0)))?);
            }
            crate::state::item_menu::ItemMenuEffect::Details => {
                let text = tooltip_text(
                    &item,
                    ItemSource::Inventory,
                    &self.catalog,
                    &self.inventory,
                    false,
                    self.gold,
                    self.accent,
                );
                let body = pinned_details_body(&text);
                let thin = format_item_details(&item, self.gold);
                if body == thin {
                    return Err("details body is the thin formatter, not the inventory tooltip".to_owned());
                }
                self.details
                    .push((item_details_key(ItemSource::Inventory, &item), item.metadata.name.clone(), body));
            }
            crate::state::item_menu::ItemMenuEffect::Link => {
                self.events.push(InputEvent::LinkItemInChat { item });
            }
            crate::state::item_menu::ItemMenuEffect::Move(target) => {
                let step = offer_amount(&mut self.prompt, target, item.index.0, stack_amount(&item), &item.metadata.name);
                self.note_step(step)?;
            }
            _ => {}
        }
        Ok(())
    }
}

fn stack(name: &str, index: u16, item_id: u32, amount: u16, item_type: u8) -> korangar_networking::InventoryItem<ResourceMetadata> {
    korangar_networking::InventoryItem {
        metadata: ResourceMetadata {
            texture: None,
            name: name.to_owned(),
            description: String::new(),
        },
        index: InventoryIndex(index),
        item_id: ItemId(item_id),
        item_type,
        slot: [0; 4],
        hire_expiration_date: 0,
        details: InventoryItemDetails::Regular {
            amount,
            equipped_position: EquipPosition::empty(),
            flags: RegularItemFlags::IDENTIFIED,
        },
    }
}

fn gear_item() -> korangar_networking::InventoryItem<ResourceMetadata> {
    let option = || ItemOptions {
        index: 0,
        value: 0,
        parameter: 0,
    };
    korangar_networking::InventoryItem {
        metadata: ResourceMetadata {
            texture: None,
            name: "Knife".to_owned(),
            description: String::new(),
        },
        index: InventoryIndex(3),
        item_id: ItemId(1201),
        item_type: 5,
        slot: [0; 4],
        hire_expiration_date: 0,
        details: InventoryItemDetails::Equippable {
            equip_position: EquipPosition::RIGHT_HAND,
            equipped_position: EquipPosition::empty(),
            bind_on_equip_type: 0,
            w_item_sprite_number: 0,
            option_count: 0,
            option_data: [option(), option(), option(), option(), option()],
            forge_data: None,
            refinement_level: 0,
            enchantment_level: 0,
            flags: EquippableItemFlags::IDENTIFIED,
        },
    }
}

/// Storage quick-move, cart quick-move, the closed-window hint, and the menu.
pub fn offline_item_menu() -> Result<OfflineItemMenuReport, String> {
    let scripted = [
        "event ItemMenuPick(0)",
        "event ItemMenuPick(1)",
        "event DismissItemMenu",
        "event ConfirmItemMove",
    ]
    .iter()
    .map(|line| script(line))
    .collect::<Result<Vec<_>, _>>()?;

    let mut world = Fixture::new();
    world.insert(stack("Red Potion", 9, 501, 30, 0));
    world.insert(stack("Knife", 4, 1201, 1, 0));
    world.insert(gear_item());
    world.insert(stack("Red Potion", 2, 501, 1, 0));
    world.windows.storage = true;

    let stack_kind = world.click_slot(InventoryIndex(9), true)?;
    let injected_alt_right = stack_kind == SlotSecondary::QuickMove;
    if !world.prompt.is_asking() {
        return Err("a 30-stack with storage open did not ask".to_owned());
    }
    let stack_prompt = world.prompt.prompt.clone();
    let stack_default = world.prompt.amount_text.clone();
    if !world.sent.is_empty() {
        return Err("the stack sent before confirm".to_owned());
    }
    world.confirm()?;
    let stack_request = world.take_sent();

    let knife_kind = world.click_slot(InventoryIndex(4), true)?;
    if knife_kind != SlotSecondary::QuickMove {
        return Err("knife click was not a quick-move".to_owned());
    }
    if world.prompt.is_asking() {
        return Err("a single knife asked for an amount".to_owned());
    }
    let knife_request = world.take_sent();

    world.click_slot(InventoryIndex(3), true)?;
    if world.prompt.is_asking() {
        return Err("gear asked for an amount".to_owned());
    }
    let gear_request = world.take_sent();

    world.windows = QuickMoveWindows {
        cart: true,
        ..QuickMoveWindows::default()
    };
    world.click_slot(InventoryIndex(4), true)?;
    let cart_request = world.take_sent();

    world.windows = QuickMoveWindows::default();
    let sent_before = world.sent.len();
    world.click_slot(InventoryIndex(4), true)?;
    let no_target_hint = world.hints.last().cloned().unwrap_or_default();
    if no_target_hint != QUICK_MOVE_NO_TARGET {
        return Err(format!("no-target hint was {no_target_hint}"));
    }
    let no_target_request = world.sent[sent_before..].iter().flatten().copied().collect::<Vec<_>>();

    let menu_kind = world.click_slot(InventoryIndex(2), false)?;
    let injected_right = menu_kind == SlotSecondary::OpenMenu;
    let menu_labels = world.menu.entries.iter().map(|row| row.label.clone()).collect::<Vec<_>>();
    if world
        .menu
        .apply_keys(ChoiceKeys {
            next: true,
            ..ChoiceKeys::default()
        })
        .is_some()
    {
        return Err("arrow down picked a row".to_owned());
    }
    let keyboard_pick = world
        .menu
        .apply_keys(ChoiceKeys {
            confirm: true,
            ..ChoiceKeys::default()
        })
        .ok_or("enter did not pick")?;

    world.click_slot(InventoryIndex(2), false)?;
    world.pick(0)?;
    let use_request = world.take_sent();
    world.click_slot(InventoryIndex(2), false)?;
    let details_row = world
        .menu
        .entries
        .iter()
        .position(|row| row.label == "Details")
        .ok_or("menu has no Details row")?;
    world.pick(details_row)?;
    let details_opened = !world.details.is_empty();
    let details_body = world.details.last().map(|(_, _, body)| body.clone()).unwrap_or_default();
    world.click_slot(InventoryIndex(2), false)?;
    let link_row = world
        .menu
        .entries
        .iter()
        .position(|row| row.label == "Link to chat")
        .ok_or("menu has no Link to chat row")?;
    world.pick(link_row)?;
    let link_queued = world.events.iter().any(|event| matches!(event, InputEvent::LinkItemInChat { .. }));

    Ok(OfflineItemMenuReport {
        injected_alt_right,
        injected_right,
        scripted,
        stack_prompt,
        stack_default,
        stack_request,
        knife_request,
        gear_request,
        cart_request,
        no_target_hint,
        no_target_request,
        menu_labels,
        use_request,
        details_opened,
        details_body,
        keyboard_pick,
        link_queued,
    })
}
