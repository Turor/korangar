//! Right-click item menu and Alt+right-click quick-move, headless.
//!
//! The offline run injects Alt+right-click through [`InputSystem`], then runs
//! the same prompt and packet encode the client uses. The live check is
//! [`super::LogicSession::item_menu_on_test`].

use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use ragnarok_bytes::ByteWriter;
use ragnarok_packets::{AccountId, ClientTick, InventoryIndex, PacketExt, UseItemPacket};
use winit::keyboard::KeyCode;

use crate::autopilot::{Command, parse_command};
use crate::input::InputSystem;
use crate::interface::item_move_flow::stack_amount;
use crate::state::inventory::ItemActivation;
use crate::state::item_menu::{ItemMenuEffect, ItemMenuFacts, ItemMenuState, MenuOrigin, build_item_menu, effect_of, pending_link_event};
use crate::state::item_move::{ItemMovePrompt, ItemMoveStep, ItemMoveTarget};
use crate::state::quick_move::{
    QUICK_MOVE_NO_TARGET, QuickMoveDecision, QuickMoveSource, QuickMoveWindows, SlotSecondary, classify_secondary, decide_quick_move,
};
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
    /// Nothing is encoded when no transfer window is open.
    pub no_target_request: Vec<u8>,
    pub menu_labels: Vec<String>,
    /// `0x0439` for inventory index 2 and account 0.
    pub use_request: Vec<u8>,
    /// Details is the row the client opens as a pinned details window.
    pub details_opened: bool,
    /// Arrow down then Enter picks this row.
    pub keyboard_pick: usize,
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

fn injected_click(alt: bool) -> (bool, bool) {
    let mut input = InputSystem::new(Arc::new(AtomicU64::new(0)));
    input.expire_synthesized();
    input.advance_synthetic_drag();
    if alt {
        input.synthesize_key(KeyCode::AltLeft, false, false, false);
    }
    input.synthesize_click(20.0, 20.0, korangar_interface::layout::MouseButton::Right);
    let report = input.update_delta(ClientTick(1));
    let right = report.mouse_click == Some(korangar_interface::layout::MouseButton::Right);
    let alt_down = input.key_down(KeyCode::AltLeft);
    (right, alt_down)
}

fn potion_facts() -> ItemMenuFacts {
    ItemMenuFacts {
        origin: MenuOrigin::Inventory,
        activation: ItemActivation::Use,
        worn: false,
        favorite: false,
        locked: false,
        windows: QuickMoveWindows::default(),
        link_to_chat: pending_link_event(2).is_some(),
    }
}

fn gear_item() -> korangar_networking::InventoryItem<ResourceMetadata> {
    use korangar_networking::InventoryItemDetails;
    use ragnarok_packets::{EquipPosition, EquippableItemFlags, ItemId, ItemOptions};

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
    let (alt_right, alt_down) = injected_click(true);
    let injected_alt_right = classify_secondary(alt_right, alt_down) == SlotSecondary::QuickMove;
    let (right, plain_alt) = injected_click(false);
    let injected_right = classify_secondary(right, plain_alt) == SlotSecondary::OpenMenu;

    let scripted = [
        "event ItemMenuPick(0)",
        "event ItemMenuPick(1)",
        "event DismissItemMenu",
        "event ConfirmItemMove",
    ]
    .iter()
    .map(|line| script(line))
    .collect::<Result<Vec<_>, _>>()?;

    let mut prompt = ItemMovePrompt::default();
    if prompt.offer(ItemMoveTarget::ToStorage, 9, 30, "Red Potion") != ItemMoveStep::AskAmount {
        return Err("a stack of 30 did not ask".to_owned());
    }
    let stack_prompt = prompt.prompt.clone();
    let stack_default = prompt.amount_text.clone();
    let (target, index, amount) = prompt.confirm()?;
    if target != ItemMoveTarget::ToStorage || index != 9 || amount != 30 {
        return Err(format!("stack confirmed as {target:?} {index} {amount}"));
    }
    let stack_request = crate::e2e::item_move_request(target, index, amount)?;

    let mut prompt = ItemMovePrompt::default();
    let knife = prompt.offer(ItemMoveTarget::ToStorage, 4, 1, "Knife");
    let ItemMoveStep::Send { target, index, amount } = knife else {
        return Err("a single knife asked for an amount".to_owned());
    };
    let knife_request = crate::e2e::item_move_request(target, index, amount)?;

    let gear_amount = stack_amount(&gear_item());
    let mut prompt = ItemMovePrompt::default();
    let gear = prompt.offer(ItemMoveTarget::ToStorage, 3, gear_amount, "Knife");
    let ItemMoveStep::Send { target, index, amount } = gear else {
        return Err("gear asked for an amount".to_owned());
    };
    let gear_request = crate::e2e::item_move_request(target, index, amount)?;

    let cart = decide_quick_move(
        QuickMoveSource::Inventory,
        QuickMoveWindows {
            cart: true,
            ..QuickMoveWindows::default()
        },
        false,
        false,
    );
    let QuickMoveDecision::Move(ItemMoveTarget::ToCart) = cart else {
        return Err(format!("cart-only quick-move was {cart:?}"));
    };
    let mut prompt = ItemMovePrompt::default();
    let cart_step = prompt.offer(ItemMoveTarget::ToCart, 4, 1, "Knife");
    let ItemMoveStep::Send { target, index, amount } = cart_step else {
        return Err("cart knife asked for an amount".to_owned());
    };
    let cart_request = crate::e2e::item_move_request(target, index, amount)?;

    let no_target = decide_quick_move(QuickMoveSource::Inventory, QuickMoveWindows::default(), false, false);
    let QuickMoveDecision::Hint(hint) = no_target else {
        return Err(format!("no-target quick-move was {no_target:?}"));
    };
    if hint != QUICK_MOVE_NO_TARGET {
        return Err(format!("no-target hint was {hint}"));
    }
    let no_target_hint = hint.to_owned();
    let no_target_request = Vec::new();

    let rows = build_item_menu(&potion_facts());
    let menu_labels = rows.iter().map(|row| row.label.clone()).collect::<Vec<_>>();
    let use_request = encode(&UseItemPacket::new(InventoryIndex(2), AccountId(0)))?;
    let details_opened = rows
        .iter()
        .any(|row| effect_of(row.entry, MenuOrigin::Inventory) == ItemMenuEffect::Details);

    let mut menu = ItemMenuState::default();
    menu.open("Red Potion".to_owned(), MenuOrigin::Inventory, 2, rows);
    if menu
        .apply_keys(ChoiceKeys {
            next: true,
            ..ChoiceKeys::default()
        })
        .is_some()
    {
        return Err("arrow down picked a row".to_owned());
    }
    let keyboard_pick = menu
        .apply_keys(ChoiceKeys {
            confirm: true,
            ..ChoiceKeys::default()
        })
        .ok_or("enter did not pick")?;

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
        keyboard_pick,
    })
}
