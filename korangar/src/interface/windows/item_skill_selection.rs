//! List window for identify, card insert, warp memo, auto spell, crafting,
//! Whitesmith refine, and change cart. Rows use item icons when a texture
//! exists. Auto Spell and Auto Shadow Spell rows draw the first frame of
//! `아이템\{file_name}.spr`, the same sprite the skill tree loads. Map and cart
//! rows are named text. Cart ACT sprites are not drawn.

use std::sync::Arc;

use korangar_interface::Interface;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{BaseLayoutInfo, Element};
use korangar_interface::event::{ClickHandler, EventQueue};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{MouseButton, Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{HorizontalAlignment, TextThemePathExt, VerticalAlignment};
use korangar_interface::window::{CustomWindow, Window};
use korangar_networking::{MessageColor, NetworkingSystem};
use ragnarok_packets::handler::PacketCallback;
use ragnarok_packets::{InventoryIndex, ItemId, SkillId};
use rust_state::{Path, State};

use super::WindowClass;
use crate::graphics::{Color, CornerDiameter, ShadowPadding, Texture};
use crate::input::InputEvent;
use crate::loaders::{AsyncLoader, FontSize, ImageType, OverflowBehavior};
use crate::renderer::LayoutExt;
use crate::state::inventory::InventoryPathExt;
use crate::state::item_skill_selection::{
    ItemSkillSelectionStatePathExt, SelectionCommand, SelectionKind, SelectionRow, card_message, catalog_label, change_cart_types,
    identify_chat, making_title, produce_message, skill_icon_sprite_path, skill_label, warp_destinations, warp_title,
    weapon_refine_applies_refine, weapon_refine_message,
};
use crate::state::theme::{InterfaceThemePathExt, InterfaceThemeType, ThemeTokensPathExt};
use crate::state::{ChatMessage, ClientState, ClientStatePathExt, client_state, client_theme, this_player};
use crate::world::{
    ItemName, ItemNameKey, ItemResource, ItemResourceKey, Library, SkillListInformation, Table, inventory_icon_texture_path,
};

type GameInterface = Interface<'static, ClientState>;

const ROW_HEIGHT: f32 = 32.0;

struct RowClick {
    index: u32,
    confirm: bool,
}

impl ClickHandler<ClientState> for RowClick {
    fn handle_click(&self, _: &State<ClientState>, queue: &mut EventQueue<ClientState>) {
        queue.queue(InputEvent::SelectItemSkillRow { index: self.index });
        if self.confirm {
            queue.queue(InputEvent::ConfirmItemSkillSelection);
        }
    }
}

struct SelectionList<R, S> {
    rows_path: R,
    selected_path: S,
    select_handlers: Vec<RowClick>,
    confirm_handlers: Vec<RowClick>,
}

impl<R, S> SelectionList<R, S> {
    fn new(rows_path: R, selected_path: S) -> Self {
        Self {
            rows_path,
            selected_path,
            select_handlers: Vec::new(),
            confirm_handlers: Vec::new(),
        }
    }
}

impl<R, S> Element<ClientState> for SelectionList<R, S>
where
    R: Path<ClientState, Vec<SelectionRow>> + Copy,
    S: Path<ClientState, u32> + Copy,
{
    type LayoutInfo = BaseLayoutInfo;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            let len = state.get(&self.rows_path).len();
            if self.select_handlers.len() != len {
                self.select_handlers = (0..len)
                    .map(|index| RowClick {
                        index: index as u32,
                        confirm: false,
                    })
                    .collect();
                self.confirm_handlers = (0..len)
                    .map(|index| RowClick {
                        index: index as u32,
                        confirm: true,
                    })
                    .collect();
            }
            let height = (len.max(1) as f32) * ROW_HEIGHT;
            BaseLayoutInfo {
                area: resolver.with_height(height),
            }
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let rows = state.get(&self.rows_path);
        let selected = *state.get(&self.selected_path);
        let count = rows.len();
        if count == 0 || self.select_handlers.len() != count {
            return;
        }

        let neutral = *state.get(&client_theme().tokens().border());
        let hover = *state.get(&client_theme().tokens().hover());
        let accent = *state.get(&client_theme().tokens().accent());
        let fill = *state.get(&client_theme().tokens().raised());
        let shadow = *state.get(&client_theme().tokens().panel());
        let text_color = *state.get(&client_theme().text().color());
        let corner = CornerDiameter::uniform((*state.get(&client_theme().tokens().corner_radius())).min(6.0));
        let row_height = layout_info.area.height / count as f32;

        for (index, row) in rows.iter().enumerate() {
            let row_area = Area {
                left: layout_info.area.left,
                top: layout_info.area.top + row_height * index as f32,
                width: layout_info.area.width,
                height: row_height,
            };
            let hovered = row_area.check().run(layout);
            let border = if selected == index as u32 {
                accent
            } else if hovered {
                hover
            } else {
                neutral
            };
            layout.add_rectangle(row_area, corner, border, shadow, ShadowPadding::diagonal(0.0, 0.0));
            let inset = 2.0;
            layout.add_rectangle(
                Area {
                    left: row_area.left + inset,
                    top: row_area.top + inset,
                    width: (row_area.width - inset * 2.0).max(1.0),
                    height: (row_area.height - inset * 2.0).max(1.0),
                },
                corner,
                fill,
                shadow,
                ShadowPadding::diagonal(0.0, 0.0),
            );

            let mut text_left = row_area.left + 8.0;
            if let Some(texture) = row.texture.as_ref() {
                let side = 24.0_f32.min(row_area.height - 4.0);
                let icon = Area {
                    left: row_area.left + 4.0,
                    top: row_area.top + (row_area.height - side) / 2.0,
                    width: side,
                    height: side,
                };
                layout.add_texture(icon, texture.clone(), Color::WHITE, false);
                text_left = icon.left + side + 6.0;
            }

            layout.add_text(
                Area {
                    left: text_left,
                    top: row_area.top,
                    width: (row_area.left + row_area.width - text_left).max(1.0),
                    height: row_area.height,
                },
                &row.label,
                FontSize(14.0),
                text_color,
                text_color,
                HorizontalAlignment::Left { offset: 0.0, border: 0.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );

            if hovered {
                layout.register_click_handler(MouseButton::Left, &self.select_handlers[index]);
                layout.register_click_handler(MouseButton::DoubleLeft, &self.confirm_handlers[index]);
            }
        }
    }
}

pub struct ItemSkillSelectionWindow;

impl CustomWindow<ClientState> for ItemSkillSelectionWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::ItemSkillSelection)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let selection = client_state().item_skill_selection();

        window! {
            title: selection.title(),
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            resizable: true,
            minimum_width: 280.0,
            minimum_height: 220.0,
            elements: (
                text! { text: selection.status() },
                scroll_view! {
                    children: SelectionList::new(selection.rows(), selection.selected()),
                },
                text! { text: "Double-click a row, or press OK." },
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        button! {
                            text: "Cancel",
                            event: InputEvent::CancelItemSkillSelection,
                        },
                        button! {
                            text: "OK",
                            event: InputEvent::ConfirmItemSkillSelection,
                        },
                    ),
                },
            ),
        }
    }
}

struct BagEntry {
    index: u16,
    item_id: u32,
    name: String,
    texture: Option<Arc<Texture>>,
}

fn bag(state: &State<ClientState>) -> Vec<BagEntry> {
    state
        .follow(client_state().inventory().items())
        .iter()
        .map(|item| BagEntry {
            index: item.index.0,
            item_id: item.item_id.0,
            name: item.metadata.name.clone(),
            texture: item.metadata.texture.clone(),
        })
        .collect()
}

fn catalog_face(library: &Library, loader: &AsyncLoader, item_id: ItemId) -> (String, Option<Arc<Texture>>) {
    let name = library
        .get::<ItemName>(ItemNameKey {
            item_id,
            is_identified: true,
        })
        .to_string();
    let texture = ItemResource::try_get(library, ItemResourceKey {
        item_id,
        is_identified: true,
    })
    .and_then(|resource| {
        inventory_icon_texture_path(resource.as_str())
            .ok()
            .and_then(|path| loader.request_item_sprite_load(item_id, resource.as_str(), &path, ImageType::Color))
    });
    (catalog_label(&name, item_id.0), texture)
}

fn skill_name(library: &Library, skill_id: i32) -> String {
    let Ok(id) = u16::try_from(skill_id) else {
        return skill_label("", skill_id);
    };
    let info = library.get::<SkillListInformation>(SkillId(id));
    skill_label(&info.name, skill_id)
}

fn skill_icon(library: &Library, loader: &AsyncLoader, skill_id: i32) -> Option<Arc<Texture>> {
    let Ok(id) = u16::try_from(skill_id) else {
        return None;
    };
    let info = library.get::<SkillListInformation>(SkillId(id));
    let path = skill_icon_sprite_path(&info.file_name)?;
    loader
        .request_skill_sprite_load(SkillId(id), &path)
        .and_then(|sprite| sprite.textures.first().cloned())
}

fn skill_row(library: &Library, loader: &AsyncLoader, skill_id: i32) -> SelectionRow {
    let mut row = SelectionRow::named(skill_name(library, skill_id));
    row.value = skill_id;
    row.texture = skill_icon(library, loader, skill_id);
    row
}

fn push_chat(state: &mut State<ClientState>, text: String) {
    state
        .follow_mut(client_state().chat_messages())
        .push(ChatMessage::new(text, MessageColor::Information));
}

fn close_window(state: &mut State<ClientState>, interface: &mut GameInterface) {
    interface.close_window_with_class(WindowClass::ItemSkillSelection);
    state.follow_mut(client_state().item_skill_selection()).clear();
}

fn show_or_tell(state: &mut State<ClientState>, interface: &mut GameInterface, empty_message: &str) {
    let empty = state.follow(client_state().item_skill_selection()).rows.is_empty();
    if empty {
        push_chat(state, empty_message.to_owned());
        close_window(state, interface);
        return;
    }
    if !interface.is_window_with_class_open(WindowClass::ItemSkillSelection) {
        interface.open_window(ItemSkillSelectionWindow);
    }
}

fn dispatch<C: PacketCallback + Send>(network: &mut NetworkingSystem<C>, command: SelectionCommand) {
    let result = match command {
        SelectionCommand::Identify { wire_index } => network.request_item_identify(wire_index),
        SelectionCommand::InsertCard { card_index, equip_index } => {
            network.request_item_composition(InventoryIndex(card_index), InventoryIndex(equip_index))
        }
        SelectionCommand::Warp { skill_id, map_name } => network.select_warp_point(SkillId(skill_id), map_name),
        SelectionCommand::AutoSpell { skill_id } => network.select_auto_spell(skill_id),
        SelectionCommand::Shadow { flag, skill_id } => network.select_skill_menu(flag, skill_id),
        SelectionCommand::Arrow { item_id } => network.request_making_arrow(ItemId(item_id)),
        SelectionCommand::Produce { item_id, material } => {
            network.request_produce_mix(ItemId(item_id), [ItemId(material[0]), ItemId(material[1]), ItemId(material[2])])
        }
        SelectionCommand::Making { make_type, item_id } => network.request_making_item(make_type, ItemId(item_id)),
        SelectionCommand::WeaponRefine { wire_index } => network.request_weapon_refine(wire_index),
        SelectionCommand::ChangeCart { cart_type } => network.request_change_cart(cart_type),
        SelectionCommand::Silent => Ok(()),
    };
    let _ = result;
}

fn identify_index(wire: u16) -> Option<InventoryIndex> {
    // `0xFFFF` is the cancel ack (`idx == -1` on the server, then written back as
    // -1).
    if wire < 2 || wire == 0xFFFF {
        None
    } else {
        Some(InventoryIndex(wire - 2))
    }
}

fn apply_identified(state: &mut State<ClientState>, library: &Library, loader: &AsyncLoader, index: InventoryIndex) {
    let Some(item_id) = state.follow_mut(client_state().inventory()).mark_identified(index) else {
        return;
    };
    let (name, texture) = catalog_face(library, loader, item_id);
    let description = library.item_description(item_id, true);
    state
        .follow_mut(client_state().inventory())
        .set_identified_presentation(index, name, description, texture);
}

pub(crate) fn open_identify_list(state: &mut State<ClientState>, interface: &mut GameInterface, indexes: Vec<InventoryIndex>) {
    let wanted: Vec<u16> = indexes.into_iter().map(|index| index.0).collect();
    let rows = bag(state)
        .into_iter()
        .filter(|entry| wanted.contains(&entry.index))
        .map(|entry| SelectionRow::inventory(entry.name, entry.index, entry.item_id, entry.texture))
        .collect();
    state
        .follow_mut(client_state().item_skill_selection())
        .show(SelectionKind::Identify, "Identify", rows);
    show_or_tell(state, interface, "No unidentified items.");
}

pub(crate) fn on_identify_result(
    state: &mut State<ClientState>,
    interface: &mut GameInterface,
    library: &Library,
    loader: &AsyncLoader,
    wire: u16,
    result: u8,
) {
    // Cancel sends `0xFFFF`. Hercules answers with that index and result 1.
    if wire == 0xFFFF {
        close_window(state, interface);
        return;
    }
    let success = result == 0;
    let index = identify_index(wire);
    if success && let Some(index) = index {
        apply_identified(state, library, loader, index);
    }
    let name = index
        .and_then(|index| {
            state
                .follow(client_state().inventory().items())
                .iter()
                .find(|item| item.index == index)
                .map(|item| item.metadata.name.clone())
        })
        .unwrap_or_else(|| "item".to_owned());
    if let Some(text) = identify_chat(wire, result, &name) {
        push_chat(state, text);
    }
    close_window(state, interface);
}

pub(crate) fn begin_card_insert<C: PacketCallback + Send>(
    state: &mut State<ClientState>,
    network: &mut NetworkingSystem<C>,
    index: InventoryIndex,
) {
    let item_id = state
        .follow(client_state().inventory().items())
        .iter()
        .find(|item| item.index == index)
        .map(|item| item.item_id.0)
        .unwrap_or(0);
    state
        .follow_mut(client_state().item_skill_selection())
        .remember_card(index.0, item_id);
    let _ = network.request_item_composition_list(index);
}

pub(crate) fn open_composition_list(state: &mut State<ClientState>, interface: &mut GameInterface, indexes: Vec<InventoryIndex>) {
    let wanted: Vec<u16> = indexes.into_iter().map(|index| index.0).collect();
    let rows = bag(state)
        .into_iter()
        .filter(|entry| wanted.contains(&entry.index))
        .map(|entry| SelectionRow::inventory(entry.name, entry.index, entry.item_id, entry.texture))
        .collect();
    state
        .follow_mut(client_state().item_skill_selection())
        .show(SelectionKind::CardInsert, "Insert Card", rows);
    show_or_tell(state, interface, "No equipment can take this card.");
}

pub(crate) fn on_composition_result(
    state: &mut State<ClientState>,
    interface: &mut GameInterface,
    equip_index: InventoryIndex,
    result: u8,
) {
    let card_item_id = state.follow(client_state().item_skill_selection()).card_item_id;
    if result == 0 && card_item_id != 0 {
        state.follow_mut(client_state().inventory()).insert_card(equip_index, card_item_id);
    }
    push_chat(state, card_message(result == 0).to_owned());
    close_window(state, interface);
}

pub(crate) fn open_warp_list(
    state: &mut State<ClientState>,
    interface: &mut GameInterface,
    library: &Library,
    skill_id: SkillId,
    maps: Vec<String>,
) {
    let rows = warp_destinations(maps, |map| {
        let label = library.map_display_name(map);
        if label.is_empty() { map.to_owned() } else { label }
    });
    let selection = state.follow_mut(client_state().item_skill_selection());
    selection.skill_id = skill_id.0;
    selection.show(SelectionKind::Warp, warp_title(skill_id.0), rows);
    show_or_tell(state, interface, "No warp destinations.");
}

pub(crate) fn open_auto_spell_list(
    state: &mut State<ClientState>,
    interface: &mut GameInterface,
    library: &Library,
    loader: &AsyncLoader,
    skill_ids: Vec<i32>,
) {
    let rows = skill_ids
        .into_iter()
        .filter(|skill_id| *skill_id != 0)
        .map(|skill_id| skill_row(library, loader, skill_id))
        .collect();
    state
        .follow_mut(client_state().item_skill_selection())
        .show(SelectionKind::AutoSpell, "Auto Spell", rows);
    show_or_tell(state, interface, "No spells to select.");
}

pub(crate) fn open_skill_select_list(
    state: &mut State<ClientState>,
    interface: &mut GameInterface,
    library: &Library,
    loader: &AsyncLoader,
    flag: i32,
    skill_ids: Vec<i16>,
) {
    let rows = skill_ids
        .into_iter()
        .filter(|skill_id| *skill_id != 0)
        .map(|skill_id| skill_row(library, loader, i32::from(skill_id)))
        .collect();
    let selection = state.follow_mut(client_state().item_skill_selection());
    selection.shadow_flag = flag;
    selection.show(SelectionKind::ShadowSpell, "Auto Shadow Spell", rows);
    show_or_tell(state, interface, "No spells to select.");
}

pub(crate) fn open_arrow_list(
    state: &mut State<ClientState>,
    interface: &mut GameInterface,
    library: &Library,
    loader: &AsyncLoader,
    item_ids: Vec<ItemId>,
) {
    let rows = item_ids
        .into_iter()
        .map(|item_id| {
            let (label, texture) = catalog_face(library, loader, item_id);
            SelectionRow::catalog(label, item_id.0, texture)
        })
        .collect();
    state
        .follow_mut(client_state().item_skill_selection())
        .show(SelectionKind::Arrow, "Arrow Crafting", rows);
    show_or_tell(state, interface, "No arrows to craft.");
}

pub(crate) fn open_makable_list(
    state: &mut State<ClientState>,
    interface: &mut GameInterface,
    library: &Library,
    loader: &AsyncLoader,
    items: Vec<(ItemId, [ItemId; 3])>,
) {
    let rows = items
        .into_iter()
        .map(|(item_id, material)| {
            let (label, texture) = catalog_face(library, loader, item_id);
            let mut row = SelectionRow::catalog(label, item_id.0, texture);
            row.material0 = material[0].0;
            row.material1 = material[1].0;
            row.material2 = material[2].0;
            row
        })
        .collect();
    state
        .follow_mut(client_state().item_skill_selection())
        .show(SelectionKind::Produce, "Create item", rows);
    show_or_tell(state, interface, "Nothing can be made.");
}

pub(crate) fn on_produce_result(state: &mut State<ClientState>, library: &Library, loader: &AsyncLoader, result: u16, item_id: ItemId) {
    let (name, _) = catalog_face(library, loader, item_id);
    push_chat(state, produce_message(result, &name));
}

pub(crate) fn open_making_list(
    state: &mut State<ClientState>,
    interface: &mut GameInterface,
    library: &Library,
    loader: &AsyncLoader,
    make_type: u16,
    item_ids: Vec<ItemId>,
) {
    let rows = item_ids
        .into_iter()
        .map(|item_id| {
            let (label, texture) = catalog_face(library, loader, item_id);
            SelectionRow::catalog(label, item_id.0, texture)
        })
        .collect();
    let selection = state.follow_mut(client_state().item_skill_selection());
    selection.make_type = make_type;
    selection.show(SelectionKind::Making, making_title(make_type), rows);
    show_or_tell(state, interface, "Nothing can be made.");
}

pub(crate) fn open_weapon_refine_list(
    state: &mut State<ClientState>,
    interface: &mut GameInterface,
    library: &Library,
    loader: &AsyncLoader,
    items: Vec<korangar_networking::WeaponRefineOffer>,
) {
    let owned = bag(state);
    let rows = items
        .into_iter()
        .map(|offer| {
            let found = owned.iter().find(|entry| entry.index == offer.index.0);
            let (name, texture) = match found {
                Some(entry) => (entry.name.clone(), entry.texture.clone()),
                None => catalog_face(library, loader, offer.item_id),
            };
            SelectionRow::inventory(format!("+{} {name}", offer.refine), offer.index.0, offer.item_id.0, texture)
        })
        .collect();
    state
        .follow_mut(client_state().item_skill_selection())
        .show(SelectionKind::WeaponRefine, "Refine Weapon", rows);
    show_or_tell(state, interface, "No equipment can be refined.");
}

pub(crate) fn on_weapon_refine_result(
    state: &mut State<ClientState>,
    library: &Library,
    loader: &AsyncLoader,
    result: i32,
    item_id: ItemId,
) {
    let (name, _) = catalog_face(library, loader, item_id);
    let message = weapon_refine_message(result, &name);
    if weapon_refine_applies_refine(result) {
        let index = state
            .follow_mut(client_state().item_skill_selection())
            .note_weapon_refine_success(item_id.0);
        if let Some(index) = index {
            let inventory_index = InventoryIndex(index);
            let next = state
                .follow(client_state().inventory())
                .refinement_level(inventory_index)
                .unwrap_or(0)
                .saturating_add(1);
            state.follow_mut(client_state().inventory()).set_refinement(inventory_index, next);
        }
    }
    state.follow_mut(client_state().item_skill_selection()).set_status(message.clone());
    push_chat(state, message);
}

pub(crate) fn open_change_cart(state: &mut State<ClientState>, interface: &mut GameInterface) {
    let level = state.try_follow(this_player()).map(|player| player.base_level).unwrap_or(1);
    let level = i32::try_from(level).unwrap_or(i32::MAX);
    let rows = change_cart_types(level)
        .into_iter()
        .map(|cart| {
            let mut row = SelectionRow::named(format!("Cart {cart}"));
            row.value = i32::from(cart);
            row
        })
        .collect();
    state
        .follow_mut(client_state().item_skill_selection())
        .show(SelectionKind::ChangeCart, "Change Cart", rows);
    show_or_tell(state, interface, "No carts are available.");
}

pub(crate) fn on_select(state: &mut State<ClientState>, index: u32) {
    state.follow_mut(client_state().item_skill_selection()).select(index);
}

pub(crate) fn on_confirm<C: PacketCallback + Send>(
    state: &mut State<ClientState>,
    network: &mut NetworkingSystem<C>,
    interface: &mut GameInterface,
) {
    let (command, close) = {
        let selection = state.follow(client_state().item_skill_selection());
        (selection.confirm(), selection.closes_after_confirm())
    };
    let Some(command) = command else {
        state
            .follow_mut(client_state().item_skill_selection())
            .set_status("Select an entry.");
        return;
    };
    dispatch(network, command);
    if close {
        close_window(state, interface);
    }
}

/// The title-bar X removes the window inside the interface and does not emit
/// an event. Escape closes through `Client::on_close_top_window`. When this
/// window was open and is now gone, send the protocol cancel.
pub(crate) fn on_window_dismissed<C: PacketCallback + Send>(
    was_open: bool,
    state: &mut State<ClientState>,
    network: &mut NetworkingSystem<C>,
    interface: &mut GameInterface,
) {
    if !was_open || interface.is_window_with_class_open(WindowClass::ItemSkillSelection) {
        return;
    }
    let pending = {
        let selection = state.follow(client_state().item_skill_selection());
        !selection.title.is_empty() || !selection.rows.is_empty()
    };
    if pending {
        on_cancel(state, network, interface);
    }
}

pub(crate) fn on_cancel<C: PacketCallback + Send>(
    state: &mut State<ClientState>,
    network: &mut NetworkingSystem<C>,
    interface: &mut GameInterface,
) {
    let command = state.follow(client_state().item_skill_selection()).cancel();
    dispatch(network, command);
    close_window(state, interface);
}

pub(crate) fn close_for_map_change<C: PacketCallback + Send>(
    state: &mut State<ClientState>,
    network: &mut NetworkingSystem<C>,
    interface: &mut GameInterface,
) {
    if interface.is_window_with_class_open(WindowClass::ItemSkillSelection) {
        on_cancel(state, network, interface);
    } else {
        state.follow_mut(client_state().item_skill_selection()).clear();
    }
}
