//! Gear window.
//!
//! Left half, top to bottom: square equipment slots around a character
//! portrait, a costume strip, a shadow strip, then saved switch-set loadouts.
//! Right half: base stats with point buttons and combat stats with a base +
//! bonus split. Inventory is its own window. Base level and job level are not
//! shown here. While this window is open the world player sprite is not drawn,
//! so it cannot cover the stats column.

use std::cell::{Cell, UnsafeCell};

use korangar_interface::application::Application;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{Element, ElementBox, ErasedElement};
use korangar_interface::event::{ClickHandler, Event, EventQueue, InputHandler};
use korangar_interface::layout::area::{Area, PartialArea};
use korangar_interface::layout::{MouseButton, Resolver, Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{HorizontalAlignment, VerticalAlignment};
use korangar_interface::window::{CustomWindow, Window};
use korangar_networking::{InventoryItem, InventoryItemDetails};
use ragnarok_packets::{EquipPosition, StatUpType};
use rust_state::{ManuallyAssertExt, Path, PathExt, Selector, State, VecIndexExt};

use crate::ItemSource;
use crate::graphics::{Color, CornerDiameter, ShadowPadding};
use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::loaders::{FontSize, OverflowBehavior};
use crate::renderer::LayoutExt;
use crate::state::equip_switch::{EquipLoadoutPathExt, EquipSwitch, EquipSwitchPathExt};
use crate::state::theme::{InterfaceThemePathExt, InterfaceThemeType, ThemeTokensPathExt};
use crate::state::{ClientState, ClientStatePathExt, client_theme, this_entity, this_player};
use crate::world::{AnimationData, AnimationFramePart, Entity, PlayerPathExt, ResourceMetadata};

/// Square equipment socket: one item icon plus a small border. Width and height
/// stay equal. The socket is not stretched to fill a grid cell.
pub(crate) const GEAR_SLOT_SIDE: f32 = 44.0;

/// Item BMP drawn centered inside the square. Inventory icons are 24×24.
pub(crate) const GEAR_ICON_SIDE: f32 = 24.0;

/// Small caption under each square. The full name stays on the tooltip.
pub(crate) const GEAR_SLOT_LABEL_HEIGHT: f32 = 12.0;

/// Gap between paperdoll rows and between a square and its neighbors.
pub(crate) const GEAR_SLOT_GAP: f32 = 4.0;

/// Center column. The portrait is one frame spanning every body row.
pub(crate) const GEAR_PORTRAIT_WIDTH: f32 = 120.0;

/// Costume and shadow wrap onto another row after this many squares so captions
/// do not share a horizontal band.
pub(crate) const STRIP_SLOTS_PER_ROW: usize = 4;

/// First-open Gear size at the 1280×720 reference. The left half is the
/// paperdoll width; the right half is the packed stat cluster.
pub(crate) const GEAR_WINDOW_WIDTH: f32 = 480.0;
/// Tall enough for the left column at 44px sockets, short enough to sit above
/// chat at 1024×576. Not scaled with the client size.
pub(crate) const GEAR_WINDOW_HEIGHT: f32 = 500.0;

/// Name, base, bonus, and the point button. They sit in this cluster.
pub(crate) const BASE_STAT_PART_WIDTHS: [f32; 4] = [42.0, 32.0, 40.0, 52.0];

/// Combat name and the base + bonus text.
pub(crate) const COMBAT_STAT_PART_WIDTHS: [f32; 2] = [48.0, 72.0];

pub(crate) const STAT_PART_GAP: f32 = 4.0;

/// Width of a packed stat row. Parts stay adjacent. The rest of the right half
/// stays empty.
pub(crate) fn packed_width(parts: &[f32], gap: f32) -> f32 {
    let n = parts.len();
    parts.iter().copied().sum::<f32>() + gap * n.saturating_sub(1) as f32
}

/// Gear is equipment and stats. Inventory opens on its own.
pub(crate) const GEAR_EMBEDS_INVENTORY: bool = false;
pub(crate) const GEAR_COLUMN_COUNT: usize = 2;

/// The preview cell draws the character, not a text stub.
pub(crate) const PREVIEW_RENDERS_CHARACTER: bool = true;

/// Add and remove for the saved-set row. The dropdown is the rest of the row.
pub(crate) const LOADOUT_CONTROLS: [&str; 2] = ["+", "-"];

/// Dropdown takes two thirds of the set row. + and − split the remaining third,
/// which is half the dropdown's width.
pub(crate) fn set_row_widths(row_width: f32) -> (f32, f32, f32) {
    let dropdown = row_width * 2.0 / 3.0;
    let button = (row_width - dropdown) / 2.0;
    (dropdown, button, button)
}

/// Theme metrics the left column is stacked with. Kept here so a test can
/// prove the loadout buttons fit in [`GEAR_WINDOW_HEIGHT`].
const SECTION_TITLE_HEIGHT: f32 = 14.0;
const CONTROL_HEIGHT: f32 = 20.0;
const COLUMN_GAP: f32 = 4.0;
const WINDOW_TITLE_HEIGHT: f32 = 22.0;
const WINDOW_TITLE_GAP: f32 = 2.0;
const WINDOW_BORDER: f32 = 8.0;

/// One square slot. The caption is painted inside the bottom of the square so
/// a row does not grow past [`GEAR_SLOT_SIDE`] and neighboring captions stay
/// in their own columns.
pub(crate) fn gear_slot_row_height() -> f32 {
    GEAR_SLOT_SIDE
}

/// Caption glyphs at FontSize 9 stay inside the square (~5px each).
pub(crate) fn slot_caption_fits(label: &str) -> bool {
    label.chars().count() as f32 * 5.0 <= GEAR_SLOT_SIDE
}

/// Inner height of the left column: titles, the tall portrait, costume, shadow,
/// and the loadout name box, buttons, and one empty-set line.
pub(crate) fn gear_left_column_content_height() -> f32 {
    // Equipment, Costume, Shadow. Sets is the control row under Shadow, not a
    // title.
    let titles = 3.0 * SECTION_TITLE_HEIGHT;
    let paperdoll = paperdoll_block_height();
    let strips = strip_block_height(COSTUME_SLOTS.len()) + strip_block_height(SHADOW_SLOTS.len());
    let controls = CONTROL_HEIGHT;
    let children = 7.0;
    let gaps = (children - 1.0) * COLUMN_GAP;
    titles + paperdoll + strips + controls + gaps
}

/// Body area under the title bar and inside the window border.
pub(crate) fn gear_content_budget(window_height: f32) -> f32 {
    window_height - WINDOW_TITLE_HEIGHT - WINDOW_TITLE_GAP - WINDOW_BORDER * 2.0
}

/// Keeps the layout numbers live in the library, not only in unit tests.
fn layout_contract_holds() -> bool {
    paperdoll_geometry(0.0, 0.0).ammo.is_square()
        && !GEAR_EMBEDS_INVENTORY
        && GEAR_COLUMN_COUNT == 2
        && PREVIEW_RENDERS_CHARACTER
        && PORTRAIT_SPANS_ALL_ROWS
        && PAPERDOLL_COLUMN_COUNT == 3
        && INVENTORY_ROWS == 4
        && INVENTORY_COLUMNS == 10
        && EQUIP_LABEL_MAX_CHARS >= 8
        && SECTION_TITLE_HEIGHT > 0.0
        && CONTROL_HEIGHT > 0.0
        && WINDOW_TITLE_HEIGHT + WINDOW_TITLE_GAP + WINDOW_BORDER > 0.0
        && packed_width(&BASE_STAT_PART_WIDTHS, STAT_PART_GAP) < GEAR_WINDOW_WIDTH
        && gear_left_column_content_height() <= gear_content_budget(GEAR_WINDOW_HEIGHT)
        && slot_caption_fits("S.L")
        && slot_caption_fits("S.R")
}

/// Controlled player stays in the world pass while Gear is open. The paperdoll
/// is an extra preview, not a replacement for that sprite.
pub(crate) fn draw_player_in_world(_gear_window_open: bool) -> bool {
    true
}

/// Top-left of a sprite part whose `offset` is the part center.
pub(crate) fn sprite_part_top_left(offset_x: i32, offset_y: i32, width: i32, height: i32) -> (i32, i32) {
    let half_w = (width - 1) / 2;
    let half_h = (height - 1) / 2;
    (offset_x - half_w, offset_y - half_h)
}

/// Fit one sprite part into a square portrait, preserving aspect.
#[allow(clippy::too_many_arguments)]
pub(crate) fn fit_sprite_part(
    union_min_x: i32,
    union_min_y: i32,
    union_w: i32,
    union_h: i32,
    part_x: i32,
    part_y: i32,
    part_w: i32,
    part_h: i32,
    box_left: f32,
    box_top: f32,
    box_size: f32,
) -> (f32, f32, f32, f32) {
    let union_w = (union_w.max(1)) as f32;
    let union_h = (union_h.max(1)) as f32;
    let scale = (box_size / union_w).min(box_size / union_h);
    let drawn_w = union_w * scale;
    let drawn_h = union_h * scale;
    let origin_x = box_left + (box_size - drawn_w) / 2.0;
    let origin_y = box_top + (box_size - drawn_h) / 2.0;
    (
        origin_x + (part_x - union_min_x) as f32 * scale,
        origin_y + (part_y - union_min_y) as f32 * scale,
        part_w.max(0) as f32 * scale,
        part_h.max(0) as f32 * scale,
    )
}

/// Gear does not repeat the character-overview level lines.
pub(crate) const GEAR_SHOWS_LEVELS: bool = false;
const _: () = assert!(!GEAR_SHOWS_LEVELS);

/// Combat rows on the right half, in display order.
pub(crate) const COMBAT_STAT_LABELS: [&str; 8] = ["ATK", "MATK", "DEF", "MDEF", "HIT", "FLEE", "CRIT", "ASPD"];
const _: usize = COMBAT_STAT_LABELS.len();

/// One body row. The center column is the portrait, not a third slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PaperdollRow {
    pub left: EquipPosition,
    pub right: EquipPosition,
}

pub(crate) const PAPERDOLL_COLUMN_COUNT: usize = 3;

/// The portrait is one frame behind every body row, not a single grid cell.
pub(crate) const PORTRAIT_SPANS_ALL_ROWS: bool = true;

/// Costume sockets in one horizontal strip under the preview.
pub(crate) const COSTUME_SLOTS: [EquipPosition; 4] = [
    EquipPosition::COSTUME_HEAD_TOP,
    EquipPosition::COSTUME_HEAD_MIDDLE,
    EquipPosition::COSTUME_HEAD_LOWER,
    EquipPosition::COSTUME_GARMENT,
];

/// Shadow sockets kept available under the costume strip.
pub(crate) const SHADOW_SLOTS: [EquipPosition; 6] = [
    EquipPosition::SHADOW_WEAPON,
    EquipPosition::SHADOW_ARMOR,
    EquipPosition::SHADOW_SHIELD,
    EquipPosition::SHADOW_SHOES,
    EquipPosition::SHADOW_LEFT_ACCESSORY,
    EquipPosition::SHADOW_RIGHT_ACCESSORY,
];

/// Standalone inventory grid. Gear does not embed it.
pub(crate) const INVENTORY_ROWS: usize = 4;
pub(crate) const INVENTORY_COLUMNS: usize = 10;

/// Longest short label. Slots use this as a tooltip, not as shrinking text.
pub(crate) const EQUIP_LABEL_MAX_CHARS: usize = 9;

/// Body slots flanking the portrait.
///
/// Left column, top to bottom: head top, head lower, right hand, garment,
/// right accessory. Right column: head middle, armor, left hand, shoes, left
/// accessory. Ammo is centered under the portrait.
pub(crate) const PAPERDOLL_ROWS: [PaperdollRow; 5] = [
    PaperdollRow {
        left: EquipPosition::HEAD_TOP,
        right: EquipPosition::HEAD_MIDDLE,
    },
    PaperdollRow {
        left: EquipPosition::HEAD_LOWER,
        right: EquipPosition::ARMOR,
    },
    PaperdollRow {
        left: EquipPosition::RIGHT_HAND,
        right: EquipPosition::LEFT_HAND,
    },
    PaperdollRow {
        left: EquipPosition::GARMENT,
        right: EquipPosition::SHOES,
    },
    PaperdollRow {
        left: EquipPosition::RIGTH_ACCESSORY,
        right: EquipPosition::LEFT_ACCESSORY,
    },
];

/// A rectangle in the paperdoll, in pixels relative to the block origin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GearFrame {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

impl GearFrame {
    pub(crate) fn is_square(self) -> bool {
        (self.width - self.height).abs() < f32::EPSILON && (self.width - GEAR_SLOT_SIDE).abs() < f32::EPSILON
    }
}

/// Placed paperdoll. Side slots are squares. The portrait fills the center
/// column.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PaperdollGeometry {
    pub rows: [GearFrame; 5],
    pub left_slots: [GearFrame; 5],
    pub right_slots: [GearFrame; 5],
    pub portrait: GearFrame,
    pub ammo: GearFrame,
}

/// Frames for the body paperdoll. `origin` is the top-left of the block.
pub(crate) fn paperdoll_geometry(origin_left: f32, origin_top: f32) -> PaperdollGeometry {
    let row_stride = gear_slot_row_height() + GEAR_SLOT_GAP;
    let right_left = origin_left + GEAR_SLOT_SIDE + GEAR_SLOT_GAP + GEAR_PORTRAIT_WIDTH + GEAR_SLOT_GAP;
    let mut left_slots = [GearFrame {
        left: 0.0,
        top: 0.0,
        width: 0.0,
        height: 0.0,
    }; 5];
    let mut right_slots = left_slots;
    let mut rows = left_slots;
    for index in 0..5 {
        let top = origin_top + index as f32 * row_stride;
        left_slots[index] = GearFrame {
            left: origin_left,
            top,
            width: GEAR_SLOT_SIDE,
            height: GEAR_SLOT_SIDE,
        };
        right_slots[index] = GearFrame {
            left: right_left,
            top,
            width: GEAR_SLOT_SIDE,
            height: GEAR_SLOT_SIDE,
        };
        rows[index] = GearFrame {
            left: origin_left,
            top,
            width: right_left + GEAR_SLOT_SIDE - origin_left,
            height: gear_slot_row_height(),
        };
    }
    let portrait = GearFrame {
        left: origin_left + GEAR_SLOT_SIDE + GEAR_SLOT_GAP,
        top: origin_top,
        width: GEAR_PORTRAIT_WIDTH,
        height: 5.0 * gear_slot_row_height() + 4.0 * GEAR_SLOT_GAP,
    };
    let ammo = GearFrame {
        left: portrait.left + (portrait.width - GEAR_SLOT_SIDE) / 2.0,
        top: portrait.top + portrait.height - gear_slot_row_height(),
        width: GEAR_SLOT_SIDE,
        height: GEAR_SLOT_SIDE,
    };
    PaperdollGeometry {
        rows,
        left_slots,
        right_slots,
        portrait,
        ammo,
    }
}

/// Height of the paperdoll block. Ammo sits inside the portrait frame.
pub(crate) fn paperdoll_block_height() -> f32 {
    paperdoll_geometry(0.0, 0.0).portrait.height
}

/// How tall a wrapping strip of `count` squares is, including captions.
pub(crate) fn strip_block_height(count: usize) -> f32 {
    let rows = count.div_ceil(STRIP_SLOTS_PER_ROW).max(1);
    rows as f32 * gear_slot_row_height() + (rows - 1) as f32 * GEAR_SLOT_GAP
}

/// Equal squares. More than [`STRIP_SLOTS_PER_ROW`] wraps onto the next row.
pub(crate) fn square_strip_frames(count: usize, origin_left: f32, origin_top: f32) -> Vec<GearFrame> {
    (0..count)
        .map(|index| {
            let column = index % STRIP_SLOTS_PER_ROW;
            let row = index / STRIP_SLOTS_PER_ROW;
            GearFrame {
                left: origin_left + column as f32 * (GEAR_SLOT_SIDE + GEAR_SLOT_GAP),
                top: origin_top + row as f32 * (gear_slot_row_height() + GEAR_SLOT_GAP),
                width: GEAR_SLOT_SIDE,
                height: GEAR_SLOT_SIDE,
            }
        })
        .collect()
}

/// roBrowser `WinStats` renewal ASPD. Hercules `SP_ASPD` is amotion.
/// `Math.floor(200 - val / 10)` equals `(2000 - amotion) / 10` for integer
/// amotion.
pub(crate) fn display_aspd(amotion: u32) -> u32 {
    let delay = amotion.min(2000);
    (2000 - delay) / 10
}

/// roBrowser `WinStats` prints HIT, FLEE, CRIT, ATK, MATK, DEF, and MDEF as the
/// packet numbers. CRIT is not divided.
pub(crate) fn display_packet_stat(value: u32) -> u32 {
    value
}

/// Short socket name. Match order is historical.
pub(crate) fn equip_slot_label(equip_position: EquipPosition) -> &'static str {
    // More specific costume/shadow flags first so a bad/legacy multi-bit value
    // cannot fall through to a body-slot arm via `contains`.
    match equip_position {
        _ if equip_position.contains(EquipPosition::COSTUME_HEAD_TOP) => "C. Top",
        _ if equip_position.contains(EquipPosition::COSTUME_HEAD_MIDDLE) => "C. Mid",
        _ if equip_position.contains(EquipPosition::COSTUME_HEAD_LOWER) => "C. Low",
        _ if equip_position.contains(EquipPosition::COSTUME_GARMENT) => "C. Garm",
        _ if equip_position.contains(EquipPosition::SHADOW_LEFT_RIGHT_ACCESSORY) => "S. Acc",
        _ if equip_position.contains(EquipPosition::SHADOW_RIGHT_ACCESSORY) => "S.R",
        _ if equip_position.contains(EquipPosition::SHADOW_LEFT_ACCESSORY) => "S.L",
        _ if equip_position.contains(EquipPosition::SHADOW_WEAPON) => "S.Wep",
        _ if equip_position.contains(EquipPosition::SHADOW_SHIELD) => "S.Shd",
        _ if equip_position.contains(EquipPosition::SHADOW_SHOES) => "S.Sho",
        _ if equip_position.contains(EquipPosition::SHADOW_ARMOR) => "S.Amm",
        _ if equip_position.contains(EquipPosition::HEAD_LOWER) => "H. Low",
        _ if equip_position.contains(EquipPosition::HEAD_MIDDLE) => "H. Mid",
        _ if equip_position.contains(EquipPosition::HEAD_TOP) => "H. Top",
        _ if equip_position.contains(EquipPosition::RIGHT_HAND) => "R. Hand",
        _ if equip_position.contains(EquipPosition::LEFT_HAND) => "L. Hand",
        _ if equip_position.contains(EquipPosition::ARMOR) => "Armor",
        _ if equip_position.contains(EquipPosition::SHOES) => "Shoes",
        _ if equip_position.contains(EquipPosition::GARMENT) => "Garment",
        _ if equip_position.contains(EquipPosition::LEFT_ACCESSORY) => "L. Acc",
        _ if equip_position.contains(EquipPosition::RIGTH_ACCESSORY) => "R. Acc",
        _ if equip_position.contains(EquipPosition::AMMO) => "Ammo",
        _ if equip_position.contains(EquipPosition::LEFT_RIGHT_ACCESSORY) => "Acc",
        _ if equip_position.contains(EquipPosition::LEFT_RIGHT_HAND) => "2H",
        _ => panic!("no display name for equip position"),
    }
}

/// Full socket name shown as a tooltip. Same match order as
/// [`equip_slot_label`].
pub(crate) fn equip_slot_tooltip(equip_position: EquipPosition) -> &'static str {
    match equip_position {
        _ if equip_position.contains(EquipPosition::COSTUME_HEAD_TOP) => "Costume head top",
        _ if equip_position.contains(EquipPosition::COSTUME_HEAD_MIDDLE) => "Costume head middle",
        _ if equip_position.contains(EquipPosition::COSTUME_HEAD_LOWER) => "Costume head lower",
        _ if equip_position.contains(EquipPosition::COSTUME_GARMENT) => "Costume garment",
        _ if equip_position.contains(EquipPosition::SHADOW_LEFT_RIGHT_ACCESSORY) => "Shadow accessory",
        _ if equip_position.contains(EquipPosition::SHADOW_RIGHT_ACCESSORY) => "Shadow right accessory",
        _ if equip_position.contains(EquipPosition::SHADOW_LEFT_ACCESSORY) => "Shadow left accessory",
        _ if equip_position.contains(EquipPosition::SHADOW_WEAPON) => "Shadow weapon",
        _ if equip_position.contains(EquipPosition::SHADOW_SHIELD) => "Shadow shield",
        _ if equip_position.contains(EquipPosition::SHADOW_SHOES) => "Shadow shoes",
        _ if equip_position.contains(EquipPosition::SHADOW_ARMOR) => "Shadow ammo",
        _ if equip_position.contains(EquipPosition::HEAD_LOWER) => "Head lower",
        _ if equip_position.contains(EquipPosition::HEAD_MIDDLE) => "Head middle",
        _ if equip_position.contains(EquipPosition::HEAD_TOP) => "Head top",
        _ if equip_position.contains(EquipPosition::RIGHT_HAND) => "Right hand",
        _ if equip_position.contains(EquipPosition::LEFT_HAND) => "Left hand",
        _ if equip_position.contains(EquipPosition::ARMOR) => "Armor",
        _ if equip_position.contains(EquipPosition::SHOES) => "Shoes",
        _ if equip_position.contains(EquipPosition::GARMENT) => "Garment",
        _ if equip_position.contains(EquipPosition::LEFT_ACCESSORY) => "Left accessory",
        _ if equip_position.contains(EquipPosition::RIGTH_ACCESSORY) => "Right accessory",
        _ if equip_position.contains(EquipPosition::AMMO) => "Ammo",
        _ if equip_position.contains(EquipPosition::LEFT_RIGHT_ACCESSORY) => "Accessory",
        _ if equip_position.contains(EquipPosition::LEFT_RIGHT_HAND) => "Right hand",
        _ => panic!("no tooltip for equip position"),
    }
}

#[allow(dead_code)]
pub(crate) fn shown_equip_positions() -> impl Iterator<Item = EquipPosition> {
    PAPERDOLL_ROWS
        .into_iter()
        .flat_map(|row| [row.left, row.right])
        .chain(std::iter::once(EquipPosition::AMMO))
        .chain(COSTUME_SLOTS)
        .chain(SHADOW_SLOTS)
}

struct EquipmentPath<P> {
    equip_position: EquipPosition,
    path: P,
}

impl<P: Copy> Clone for EquipmentPath<P> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<P: Copy> Copy for EquipmentPath<P> {}

impl<P> Selector<ClientState, InventoryItem<ResourceMetadata>, false> for EquipmentPath<P>
where
    P: Path<ClientState, Vec<InventoryItem<ResourceMetadata>>>,
{
    fn select<'a>(&'a self, state: &'a ClientState) -> Option<&'a InventoryItem<ResourceMetadata>> {
        self.follow(state)
    }
}

impl<P> Path<ClientState, InventoryItem<ResourceMetadata>, false> for EquipmentPath<P>
where
    P: Path<ClientState, Vec<InventoryItem<ResourceMetadata>>>,
{
    fn follow<'a>(&self, state: &'a ClientState) -> Option<&'a InventoryItem<ResourceMetadata>> {
        self.path.follow_safe(state).iter().find(|item| {
            if let InventoryItemDetails::Equippable { equipped_position, .. } = &item.details {
                return equipped_position.contains(self.equip_position);
            }
            false
        })
    }

    fn follow_mut<'a>(&self, state: &'a mut ClientState) -> Option<&'a mut InventoryItem<ResourceMetadata>> {
        self.path.follow_mut_safe(state).iter_mut().find(|item| {
            if let InventoryItemDetails::Equippable { equipped_position, .. } = item.details {
                return equipped_position.contains(self.equip_position);
            }
            false
        })
    }
}

struct BonusText<A> {
    bonus_path: A,
    last_value: Cell<Option<i32>>,
    text: UnsafeCell<String>,
}

impl<A> BonusText<A> {
    fn new(bonus_path: A) -> Self {
        Self {
            bonus_path,
            last_value: Cell::default(),
            text: UnsafeCell::default(),
        }
    }
}

impl<A> Selector<ClientState, String> for BonusText<A>
where
    A: Path<ClientState, i32>,
{
    fn select<'a>(&'a self, state: &'a ClientState) -> Option<&'a String> {
        let bonus = *self.bonus_path.follow_safe(state);
        if self.last_value.get() != Some(bonus) {
            unsafe {
                *self.text.get() = format!("^000001{bonus:+}^000000");
            }
            self.last_value.set(Some(bonus));
        }
        unsafe { Some(self.text.as_ref_unchecked()) }
    }
}

struct PairText<A, B> {
    base_path: A,
    bonus_path: B,
    last_value: Cell<Option<(u32, u32)>>,
    text: UnsafeCell<String>,
}

impl<A, B> PairText<A, B> {
    fn new(base_path: A, bonus_path: B) -> Self {
        Self {
            base_path,
            bonus_path,
            last_value: Cell::default(),
            text: UnsafeCell::default(),
        }
    }
}

impl<A, B> Selector<ClientState, String> for PairText<A, B>
where
    A: Path<ClientState, u32>,
    B: Path<ClientState, u32>,
{
    fn select<'a>(&'a self, state: &'a ClientState) -> Option<&'a String> {
        let base = display_packet_stat(*self.base_path.follow_safe(state));
        let bonus = display_packet_stat(*self.bonus_path.follow_safe(state));
        if self.last_value.get() != Some((base, bonus)) {
            unsafe {
                *self.text.get() = format!("{base} ^000001+ {bonus}^000000");
            }
            self.last_value.set(Some((base, bonus)));
        }
        unsafe { Some(self.text.as_ref_unchecked()) }
    }
}

struct CostText<A> {
    cost_path: A,
    last_value: Cell<Option<u8>>,
    text: UnsafeCell<String>,
}

impl<A> CostText<A> {
    fn new(cost_path: A) -> Self {
        Self {
            cost_path,
            last_value: Cell::default(),
            text: UnsafeCell::default(),
        }
    }
}

impl<A> Selector<ClientState, String> for CostText<A>
where
    A: Path<ClientState, u8>,
{
    fn select<'a>(&'a self, state: &'a ClientState) -> Option<&'a String> {
        let cost = *self.cost_path.follow_safe(state);
        if self.last_value.get() != Some(cost) {
            unsafe {
                *self.text.get() = match cost {
                    0 => "^000001max^000000".to_string(),
                    cost => format!("+ ({cost})"),
                };
            }
            self.last_value.set(Some(cost));
        }
        unsafe { Some(self.text.as_ref_unchecked()) }
    }
}

struct PacketStatLabel<A> {
    path: A,
    last: Cell<u32>,
    text: UnsafeCell<String>,
}

impl<A> PacketStatLabel<A> {
    fn new(path: A) -> Self {
        Self {
            path,
            last: Cell::new(u32::MAX),
            text: UnsafeCell::default(),
        }
    }
}

impl<A> Selector<ClientState, String> for PacketStatLabel<A>
where
    A: Path<ClientState, u32>,
{
    fn select<'a>(&'a self, state: &'a ClientState) -> Option<&'a String> {
        let raw = self.path.follow(state).copied().unwrap_or(0);
        if self.last.get() != raw {
            unsafe {
                *self.text.get() = display_packet_stat(raw).to_string();
            }
            self.last.set(raw);
        }
        unsafe { Some(self.text.as_ref_unchecked()) }
    }
}

struct AspdLabel<A> {
    path: A,
    last: Cell<u32>,
    text: UnsafeCell<String>,
}

impl<A> AspdLabel<A> {
    fn new(path: A) -> Self {
        Self {
            path,
            last: Cell::new(u32::MAX),
            text: UnsafeCell::default(),
        }
    }
}

impl<A> Selector<ClientState, String> for AspdLabel<A>
where
    A: Path<ClientState, u32>,
{
    fn select<'a>(&'a self, state: &'a ClientState) -> Option<&'a String> {
        let amotion = self.path.follow(state).copied().unwrap_or(0);
        if self.last.get() != amotion {
            unsafe {
                *self.text.get() = display_aspd(amotion).to_string();
            }
            self.last.set(amotion);
        }
        unsafe { Some(self.text.as_ref_unchecked()) }
    }
}

struct SelectedSetName<A> {
    path: A,
    text: UnsafeCell<String>,
}

impl<A> SelectedSetName<A> {
    fn new(path: A) -> Self {
        Self {
            path,
            text: UnsafeCell::default(),
        }
    }
}

impl<A> Selector<ClientState, String> for SelectedSetName<A>
where
    A: Path<ClientState, EquipSwitch>,
{
    fn select<'a>(&'a self, state: &'a ClientState) -> Option<&'a String> {
        let equip = self.path.follow(state)?;
        let label = equip
            .loadout(equip.selected_index())
            .map(|set| set.name().to_string())
            .unwrap_or_else(|| "Sets".to_string());
        unsafe {
            *self.text.get() = label;
            Some(self.text.as_ref_unchecked())
        }
    }
}

struct GearSetName;

struct BeginRenameClick;

impl ClickHandler<ClientState> for BeginRenameClick {
    fn handle_click(&self, _state: &State<ClientState>, queue: &mut EventQueue<ClientState>) {
        queue.queue(InputEvent::BeginRenameLoadout);
    }
}

/// Enter applies the draft. Escape drops it. Both leave the text box.
struct GearSetNameHandler<P> {
    path: P,
}

impl<P> GearSetNameHandler<P> {
    fn new(path: P) -> Self {
        Self { path }
    }
}

impl<P> InputHandler<ClientState> for GearSetNameHandler<P>
where
    P: Path<ClientState, String>,
{
    fn handle_character(&self, state: &State<ClientState>, queue: &mut EventQueue<ClientState>, character: char) {
        if character == '\t' || character == '\r' {
            queue.queue(InputEvent::RenameEquipLoadout);
            queue.queue(Event::Unfocus);
        } else if character == '\x1b' {
            queue.queue(InputEvent::CancelRenameLoadout);
            queue.queue(Event::Unfocus);
        } else if character == '\x08' {
            state.update_value_with(self.path, move |current_text| {
                current_text.pop();
            });
        } else if !character.is_control() {
            state.update_value_with(self.path, move |current_text| {
                if current_text.len().saturating_add(character.len_utf8()) <= 24 {
                    current_text.push(character);
                }
            });
        }
    }
}

/// One row: set dropdown, +, −. The menu lists saved names. Double-click the
/// name to edit it inline.
struct SetBar<A> {
    switch_path: A,
    elements: Vec<ElementBox<ClientState>>,
    signature: usize,
    rename_click: BeginRenameClick,
    dropdown: Cell<Area>,
}

impl<A> SetBar<A>
where
    A: Copy + Path<ClientState, EquipSwitch> + 'static,
{
    fn new(switch_path: A) -> Self {
        Self {
            switch_path,
            elements: Vec::new(),
            signature: usize::MAX,
            rename_click: BeginRenameClick,
            dropdown: Cell::new(Area {
                left: 0.0,
                top: 0.0,
                width: 0.0,
                height: 0.0,
            }),
        }
    }
}

impl<A> Element<ClientState> for SetBar<A>
where
    A: Copy + Path<ClientState, EquipSwitch> + 'static,
{
    type LayoutInfo = ();

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        mut store: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        use korangar_interface::prelude::*;

        with_single_resolver(resolvers, |resolver| {
            let equip = state.get(&self.switch_path);
            let count = equip.loadout_count();
            let open = equip.sets_menu_open();
            let renaming = equip.renaming_set();
            let signature = count * 4 + usize::from(open) * 2 + usize::from(renaming);
            if signature != self.signature {
                self.signature = signature;
                self.elements.clear();
                let switch_path = self.switch_path;
                if renaming {
                    self.elements.push(ErasedElement::new(text_box! {
                        ghost_text: "Set name",
                        state: switch_path.draft_name(),
                        input_handler: GearSetNameHandler::new(switch_path.draft_name()),
                        focus_id: GearSetName,
                    }));
                } else {
                    self.elements.push(ErasedElement::new(button! {
                        text: SelectedSetName::new(switch_path),
                        event: InputEvent::ToggleGearSets,
                    }));
                }
                self.elements.push(ErasedElement::new(button! {
                    text: LOADOUT_CONTROLS[0],
                    event: InputEvent::SaveEquipLoadout,
                }));
                self.elements.push(ErasedElement::new(button! {
                    text: LOADOUT_CONTROLS[1],
                    disabled: ComputedSelector::new_default(move |state: &ClientState| {
                        switch_path.follow(state).is_none_or(|equip| equip.loadout_count() == 0)
                    }),
                    event: InputEvent::DeleteEquipLoadout,
                }));
                if open {
                    for index in 0..count {
                        self.elements.push(ErasedElement::new(button! {
                            text: ShowString::new(switch_path.loadouts().index(index).name()),
                            event: move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                                state.update_value_with(switch_path.selected(), move |selected| *selected = index);
                                queue.queue(InputEvent::ApplyEquipLoadout { index, wear: true });
                            },
                        }));
                    }
                }
            }

            let menu_rows = if open { count } else { 0 };
            let height = CONTROL_HEIGHT + menu_rows as f32 * (CONTROL_HEIGHT + COLUMN_GAP);
            let origin = resolver.with_height(height);
            let (dropdown, plus, minus) = set_row_widths(origin.width);
            let frames = [
                GearFrame {
                    left: origin.left,
                    top: origin.top,
                    width: dropdown,
                    height: CONTROL_HEIGHT,
                },
                GearFrame {
                    left: origin.left + dropdown,
                    top: origin.top,
                    width: plus,
                    height: CONTROL_HEIGHT,
                },
                GearFrame {
                    left: origin.left + dropdown + plus,
                    top: origin.top,
                    width: minus,
                    height: CONTROL_HEIGHT,
                },
            ];
            self.dropdown.set(Area {
                left: frames[0].left,
                top: frames[0].top,
                width: frames[0].width,
                height: frames[0].height,
            });
            let text_layouter = resolver.get_text_layouter();
            for (index, frame) in frames.into_iter().enumerate() {
                if let Some(element) = self.elements.get_mut(index) {
                    layout_erased_in_frame(element, state, store.child_store(index as u64), text_layouter, frame);
                }
            }
            for (row, element) in self.elements.iter_mut().enumerate().skip(3) {
                let frame = GearFrame {
                    left: origin.left,
                    top: origin.top + CONTROL_HEIGHT + COLUMN_GAP + (row - 3) as f32 * (CONTROL_HEIGHT + COLUMN_GAP),
                    width: origin.width,
                    height: CONTROL_HEIGHT,
                };
                layout_erased_in_frame(element, state, store.child_store(row as u64), text_layouter, frame);
            }
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        store: ElementStore<'a>,
        _: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        for (index, element) in self.elements.iter().enumerate() {
            element.lay_out(state, store.child_store(index as u64), &(), layout);
        }
        if self.dropdown.get().check().run(layout) {
            layout.register_click_handler(MouseButton::DoubleLeft, &self.rename_click);
        }
    }
}

pub(crate) fn paint_portrait(state: &State<ClientState>, layout: &mut WindowLayout<ClientState>, area: Area) {
    layout.add_rectangle(
        area,
        CornerDiameter::uniform(6.0),
        state.get(&client_theme().tokens().raised()).with_alpha_u8(230),
        state.get(&client_theme().tokens().bg()).with_alpha_u8(120),
        ShadowPadding::diagonal(2.0, 4.0),
    );

    let entity_path = this_entity();
    let Some(Entity::Player(player)) = state.try_get(&entity_path) else {
        return;
    };
    let Some(animation) = player.get_common().animation_data.as_ref() else {
        return;
    };
    let Some(frame) = animation.animations.first().and_then(|animation| animation.frames.first()) else {
        return;
    };

    let inset = 4.0;
    let box_size = (area.width - inset * 2.0).min(area.height - inset * 2.0);
    if box_size <= 1.0 {
        return;
    }
    let box_left = area.left + (area.width - box_size) / 2.0;
    let box_top = area.top + (area.height - box_size) / 2.0;

    let mut union: Option<(i32, i32, i32, i32)> = None;
    for part in frame.frame_parts.iter().filter(|part| part_is_drawable(part, animation)) {
        let (x, y) = sprite_part_top_left(part.offset.x, part.offset.y, part.size.x, part.size.y);
        union = Some(match union {
            None => (x, y, x + part.size.x, y + part.size.y),
            Some((min_x, min_y, max_x, max_y)) => (
                min_x.min(x),
                min_y.min(y),
                max_x.max(x + part.size.x),
                max_y.max(y + part.size.y),
            ),
        });
    }
    let Some((min_x, min_y, max_x, max_y)) = union else {
        return;
    };

    for part in frame.frame_parts.iter().filter(|part| part_is_drawable(part, animation)) {
        let (x, y) = sprite_part_top_left(part.offset.x, part.offset.y, part.size.x, part.size.y);
        let (left, top, width, height) = fit_sprite_part(
            min_x,
            min_y,
            max_x - min_x,
            max_y - min_y,
            x,
            y,
            part.size.x,
            part.size.y,
            box_left,
            box_top,
            box_size,
        );
        let texture = animation.animation_pair[part.animation_index].sprites.textures[part.sprite_number].clone();
        layout.add_texture(Area { left, top, width, height }, texture, part.color, false);
    }
}

fn paint_slot_label(state: &State<ClientState>, layout: &mut WindowLayout<ClientState>, slot: GearFrame, label: &'static str) {
    layout.add_text(
        Area {
            left: slot.left,
            top: slot.top + slot.height - GEAR_SLOT_LABEL_HEIGHT,
            width: slot.width,
            height: GEAR_SLOT_LABEL_HEIGHT,
        },
        label,
        FontSize(9.0),
        *state.get(&client_theme().tokens().muted()),
        *state.get(&client_theme().tokens().gold()),
        HorizontalAlignment::Center { offset: 0.0, border: 0.0 },
        VerticalAlignment::Top { offset: 0.0 },
        OverflowBehavior::Shrink,
    );
}

fn layout_erased_in_frame(
    element: &mut ElementBox<ClientState>,
    state: &State<ClientState>,
    store: ElementStoreMut,
    text_layouter: &<ClientState as Application>::TextLayouter,
    frame: GearFrame,
) {
    let mut resolver = Resolver::new(
        PartialArea {
            left: frame.left,
            top: frame.top,
            width: frame.width,
            height: Some(frame.height),
        },
        0.0,
        text_layouter,
    );
    element.create_layout_info(state, store, &mut resolver);
}

struct Paperdoll<A> {
    slots: Vec<ElementBox<ClientState>>,
    _path: std::marker::PhantomData<A>,
}

impl<A> Paperdoll<A>
where
    A: Copy + Path<ClientState, Vec<InventoryItem<ResourceMetadata>>>,
{
    fn new(items_path: A) -> Self {
        let mut slots = Vec::with_capacity(11);
        for row in PAPERDOLL_ROWS {
            slots.push(equip_socket(items_path, row.left));
        }
        for row in PAPERDOLL_ROWS {
            slots.push(equip_socket(items_path, row.right));
        }
        slots.push(equip_socket(items_path, EquipPosition::AMMO));
        Self {
            slots,
            _path: std::marker::PhantomData,
        }
    }
}

impl<A> Element<ClientState> for Paperdoll<A>
where
    A: Path<ClientState, Vec<InventoryItem<ResourceMetadata>>>,
{
    type LayoutInfo = Area;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        mut store: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            let origin = resolver.with_height(paperdoll_block_height());
            let geometry = paperdoll_geometry(origin.left, origin.top);
            let text_layouter = resolver.get_text_layouter();
            for (index, frame) in geometry.left_slots.into_iter().enumerate() {
                layout_erased_in_frame(
                    &mut self.slots[index],
                    state,
                    store.child_store(index as u64),
                    text_layouter,
                    frame,
                );
            }
            for (index, frame) in geometry.right_slots.into_iter().enumerate() {
                let slot_index = index + 5;
                layout_erased_in_frame(
                    &mut self.slots[slot_index],
                    state,
                    store.child_store(slot_index as u64),
                    text_layouter,
                    frame,
                );
            }
            layout_erased_in_frame(&mut self.slots[10], state, store.child_store(10), text_layouter, geometry.ammo);
            Area {
                left: geometry.portrait.left,
                top: geometry.portrait.top,
                width: geometry.portrait.width,
                height: geometry.portrait.height,
            }
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        store: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        paint_portrait(state, layout, *layout_info);
        let geometry = paperdoll_geometry(layout_info.left - GEAR_SLOT_SIDE - GEAR_SLOT_GAP, layout_info.top);
        for (index, row) in PAPERDOLL_ROWS.into_iter().enumerate() {
            self.slots[index].lay_out(state, store.child_store(index as u64), &(), layout);
            paint_slot_label(state, layout, geometry.left_slots[index], equip_slot_label(row.left));
            let right_index = index + 5;
            self.slots[right_index].lay_out(state, store.child_store(right_index as u64), &(), layout);
            paint_slot_label(state, layout, geometry.right_slots[index], equip_slot_label(row.right));
        }
        self.slots[10].lay_out(state, store.child_store(10), &(), layout);
        paint_slot_label(state, layout, geometry.ammo, equip_slot_label(EquipPosition::AMMO));
    }
}

struct LabeledStrip {
    positions: &'static [EquipPosition],
    slots: Vec<ElementBox<ClientState>>,
}

impl LabeledStrip {
    fn new<A>(items_path: A, positions: &'static [EquipPosition]) -> Self
    where
        A: Copy + Path<ClientState, Vec<InventoryItem<ResourceMetadata>>>,
    {
        Self {
            positions,
            slots: positions
                .iter()
                .copied()
                .map(|position| equip_socket(items_path, position))
                .collect(),
        }
    }
}

impl Element<ClientState> for LabeledStrip {
    type LayoutInfo = Vec<GearFrame>;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        mut store: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            let origin = resolver.with_height(strip_block_height(self.positions.len()));
            let frames = square_strip_frames(self.positions.len(), origin.left, origin.top);
            let text_layouter = resolver.get_text_layouter();
            for (index, frame) in frames.iter().copied().enumerate() {
                layout_erased_in_frame(
                    &mut self.slots[index],
                    state,
                    store.child_store(index as u64),
                    text_layouter,
                    frame,
                );
            }
            frames
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        store: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        for (index, position) in self.positions.iter().copied().enumerate() {
            self.slots[index].lay_out(state, store.child_store(index as u64), &(), layout);
            paint_slot_label(state, layout, layout_info[index], equip_slot_label(position));
        }
    }
}

/// Left-aligned row. Each child gets a fixed width, so stat text does not
/// spread across the right half of Gear.
struct PackedSplit<C> {
    widths: &'static [f32],
    gap: f32,
    children: C,
}

struct PackedResolvers<'a, App: Application> {
    area: PartialArea,
    widths: &'static [f32],
    gap: f32,
    used_height: f32,
    text_layouter: &'a App::TextLayouter,
}

impl<'a, App> Resolvers<'a, App> for PackedResolvers<'a, App>
where
    App: Application,
{
    fn for_index(&self, index: usize) -> Resolver<'a, App> {
        let mut left = self.area.left;
        for width in self.widths.iter().take(index) {
            left += width + self.gap;
        }
        let width = self.widths.get(index).copied().unwrap_or(0.0);
        Resolver::new(
            PartialArea {
                left,
                top: self.area.top,
                width,
                height: self.area.height,
            },
            self.gap,
            self.text_layouter,
        )
    }

    fn give_back(&mut self, resolver: Resolver<'a, App>) {
        self.used_height = self.used_height.max(resolver.get_used_height());
    }
}

impl<C> Element<ClientState> for PackedSplit<C>
where
    C: Element<ClientState>,
{
    type LayoutInfo = C::LayoutInfo;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        store: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            let area = resolver.push_available_area();
            let mut packed = PackedResolvers {
                area,
                widths: self.widths,
                gap: self.gap,
                used_height: 0.0,
                text_layouter: resolver.get_text_layouter(),
            };
            let layout_info = self.children.create_layout_info(state, store, &mut packed);
            let used_height = packed.used_height;
            resolver.commit_used_height(used_height);
            layout_info
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        store: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        self.children.lay_out(state, store, layout_info, layout);
    }
}

fn equip_socket<A>(items_path: A, equip_position: EquipPosition) -> ElementBox<ClientState>
where
    A: Copy + Path<ClientState, Vec<InventoryItem<ResourceMetadata>>> + 'static,
{
    use crate::interface::components::item_box::ItemBox;

    let equipment_path = EquipmentPath {
        equip_position,
        path: items_path,
    };
    ErasedElement::new(ItemBox::component_new(
        equipment_path,
        ItemSource::Equipment { position: equip_position },
        GEAR_SLOT_SIDE,
        GEAR_ICON_SIDE,
        equip_slot_tooltip(equip_position),
    ))
}

pub(crate) fn part_is_drawable(part: &AnimationFramePart, animation: &AnimationData) -> bool {
    part.size.x > 0
        && part.size.y > 0
        && part.animation_index < animation.animation_pair.len()
        && part.sprite_number < animation.animation_pair[part.animation_index].sprites.textures.len()
}

struct ShowString<A> {
    path: A,
    text: UnsafeCell<String>,
}

impl<A> ShowString<A> {
    fn new(path: A) -> Self {
        Self {
            path,
            text: UnsafeCell::default(),
        }
    }
}

impl<A> Selector<ClientState, String> for ShowString<A>
where
    A: Path<ClientState, String, false>,
{
    fn select<'a>(&'a self, state: &'a ClientState) -> Option<&'a String> {
        let value = self.path.follow(state)?;
        unsafe {
            if &*self.text.get() != value {
                *self.text.get() = value.clone();
            }
            Some(self.text.as_ref_unchecked())
        }
    }
}

pub struct EquipmentWindow<A> {
    items_path: A,
}

impl<A> EquipmentWindow<A> {
    pub fn new(items_path: A) -> Self {
        Self { items_path }
    }
}

impl<A> CustomWindow<ClientState> for EquipmentWindow<A>
where
    A: Copy + Path<ClientState, Vec<InventoryItem<ResourceMetadata>>>,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Equipment)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        fn disabled_cutoff<P, C>(stat_points_path: P, cost_path: C) -> impl Selector<ClientState, bool>
        where
            P: Path<ClientState, u32>,
            C: Path<ClientState, u8>,
        {
            ComputedSelector::new_default(move |state: &ClientState| {
                let stat_points = stat_points_path.follow_safe(state);
                let cost = cost_path.follow_safe(state);
                *cost == 0 || *stat_points < *cost as u32
            })
        }

        let _ = layout_contract_holds();
        let items = self.items_path;
        let player = this_player().manually_asserted();
        let switch_set = crate::state::client_state().equip_switch();

        macro_rules! base_stat {
            ($label:literal, $value:ident, $bonus:ident, $cost:ident, $kind:ident) => {
                PackedSplit {
                    widths: &BASE_STAT_PART_WIDTHS,
                    gap: STAT_PART_GAP,
                    children: (
                        text! {
                            text: $label,
                            overflow_behavior: OverflowBehavior::Shrink,
                        },
                        text! {
                            text: PartialEqDisplaySelector::new(player.$value()),
                            overflow_behavior: OverflowBehavior::Shrink,
                        },
                        text! {
                            text: BonusText::new(player.$bonus()),
                            overflow_behavior: OverflowBehavior::Shrink,
                        },
                        button! {
                            text: CostText::new(player.$cost()),
                            disabled: disabled_cutoff(player.stat_points(), player.$cost()),
                            event: InputEvent::StatUp { stat_type: StatUpType::$kind { amount: 1 } },
                        },
                    ),
                }
            };
        }

        macro_rules! combat_pair {
            ($label:literal, $base:ident, $bonus:ident) => {
                PackedSplit {
                    widths: &COMBAT_STAT_PART_WIDTHS,
                    gap: STAT_PART_GAP,
                    children: (
                        text! {
                            text: $label,
                            overflow_behavior: OverflowBehavior::Shrink,
                        },
                        text! {
                            text: PairText::new(player.$base(), player.$bonus()),
                            overflow_behavior: OverflowBehavior::Shrink,
                        },
                    ),
                }
            };
        }

        macro_rules! combat_single {
            ($label:literal, $value:ident) => {
                PackedSplit {
                    widths: &COMBAT_STAT_PART_WIDTHS,
                    gap: STAT_PART_GAP,
                    children: (
                        text! {
                            text: $label,
                            overflow_behavior: OverflowBehavior::Shrink,
                        },
                        text! {
                            text: PacketStatLabel::new(player.$value()),
                            overflow_behavior: OverflowBehavior::Shrink,
                        },
                    ),
                }
            };
        }

        window! {
            title: "Gear",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            minimum_width: GEAR_WINDOW_WIDTH,
            maximum_width: 960.0,
            minimum_height: GEAR_WINDOW_HEIGHT,
            maximum_height: 900.0,
            elements: (
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        fragment! {
                            gaps: COLUMN_GAP,
                            children: (
                                text! {
                                    text: "Equipment",
                                    overflow_behavior: OverflowBehavior::Shrink,
                                },
                                Paperdoll::new(items),
                                text! {
                                    text: "Costume",
                                    overflow_behavior: OverflowBehavior::Shrink,
                                },
                                LabeledStrip::new(items, &COSTUME_SLOTS),
                                text! {
                                    text: "Shadow",
                                    overflow_behavior: OverflowBehavior::Shrink,
                                },
                                LabeledStrip::new(items, &SHADOW_SLOTS),
                                SetBar::new(switch_set),
                            ),
                        },
                        fragment! {
                            gaps: theme().window().gaps(),
                            children: (
                                text! {
                                    text: "Base",
                                    overflow_behavior: OverflowBehavior::Shrink,
                                },
                                text! {
                                    text: PartialEqDisplaySelector::new(player.stat_points()),
                                    overflow_behavior: OverflowBehavior::Shrink,
                                },
                                base_stat!("STR", strength, bonus_strength, strength_stat_points_cost, Strength),
                                base_stat!("AGI", agility, bonus_agility, agility_stat_points_cost, Agility),
                                base_stat!("VIT", vitality, bonus_vitality, vitality_stat_points_cost, Vitality),
                                base_stat!("INT", intelligence, bonus_intelligence, intelligence_stat_points_cost, Intelligence),
                                base_stat!("DEX", dexterity, bonus_dexterity, dexterity_stat_points_cost, Dexterity),
                                base_stat!("LUK", luck, bonus_luck, luck_stat_points_cost, Luck),
                                text! {
                                    text: "Combat",
                                    overflow_behavior: OverflowBehavior::Shrink,
                                },
                                combat_pair!("ATK", attack_base, attack_bonus),
                                combat_pair!("MATK", magic_attack_base, magic_attack_bonus),
                                combat_pair!("DEF", defense_base, defense_bonus),
                                combat_pair!("MDEF", magic_defense_base, magic_defense_bonus),
                                combat_single!("HIT", hit),
                                combat_pair!("FLEE", flee_base, flee_bonus),
                                combat_single!("CRIT", critical),
                                PackedSplit {
                                    widths: &COMBAT_STAT_PART_WIDTHS,
                                    gap: STAT_PART_GAP,
                                    children: (
                                        text! {
                                            text: "ASPD",
                                            overflow_behavior: OverflowBehavior::Shrink,
                                        },
                                        text! {
                                            text: AspdLabel::new(player.attack_speed()),
                                            overflow_behavior: OverflowBehavior::Shrink,
                                        },
                                    ),
                                },
                            ),
                        },
                    ),
                },
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use ragnarok_packets::EquipPosition;

    use super::{
        BASE_STAT_PART_WIDTHS, COMBAT_STAT_LABELS, COMBAT_STAT_PART_WIDTHS, COSTUME_SLOTS, EQUIP_LABEL_MAX_CHARS, GEAR_COLUMN_COUNT,
        GEAR_EMBEDS_INVENTORY, GEAR_PORTRAIT_WIDTH, GEAR_SLOT_GAP, GEAR_SLOT_SIDE, GEAR_WINDOW_HEIGHT, GEAR_WINDOW_WIDTH,
        INVENTORY_COLUMNS, INVENTORY_ROWS, LOADOUT_CONTROLS, PAPERDOLL_COLUMN_COUNT, PAPERDOLL_ROWS, PORTRAIT_SPANS_ALL_ROWS,
        PREVIEW_RENDERS_CHARACTER, SHADOW_SLOTS, STAT_PART_GAP, STRIP_SLOTS_PER_ROW, display_aspd, display_packet_stat,
        draw_player_in_world, equip_slot_label, equip_slot_tooltip, fit_sprite_part, gear_content_budget, gear_left_column_content_height,
        packed_width, paperdoll_geometry, set_row_widths, shown_equip_positions, slot_caption_fits, sprite_part_top_left,
        square_strip_frames, strip_block_height,
    };

    #[test]
    fn paperdoll_shows_body_slots_and_both_accessories() {
        let shown: Vec<_> = shown_equip_positions().collect();
        for position in [
            EquipPosition::HEAD_TOP,
            EquipPosition::HEAD_MIDDLE,
            EquipPosition::HEAD_LOWER,
            EquipPosition::LEFT_HAND,
            EquipPosition::ARMOR,
            EquipPosition::RIGHT_HAND,
            EquipPosition::LEFT_ACCESSORY,
            EquipPosition::GARMENT,
            EquipPosition::RIGTH_ACCESSORY,
            EquipPosition::SHOES,
            EquipPosition::AMMO,
        ] {
            assert!(shown.contains(&position), "missing body slot {position:?}");
        }
    }

    #[test]
    fn paperdoll_includes_costume_and_shadow_slots() {
        let shown: Vec<_> = shown_equip_positions().collect();
        for position in [
            EquipPosition::COSTUME_HEAD_TOP,
            EquipPosition::COSTUME_HEAD_MIDDLE,
            EquipPosition::COSTUME_HEAD_LOWER,
            EquipPosition::COSTUME_GARMENT,
            EquipPosition::SHADOW_WEAPON,
            EquipPosition::SHADOW_ARMOR,
            EquipPosition::SHADOW_SHIELD,
            EquipPosition::SHADOW_SHOES,
            EquipPosition::SHADOW_LEFT_ACCESSORY,
            EquipPosition::SHADOW_RIGHT_ACCESSORY,
        ] {
            assert!(shown.contains(&position), "missing costume/shadow slot {position:?}");
        }
        assert!(!shown.contains(&EquipPosition::SHADOW_LEFT_RIGHT_ACCESSORY));
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn paperdoll_rows_flank_a_full_height_portrait() {
        assert!(PORTRAIT_SPANS_ALL_ROWS);
        assert_eq!(PAPERDOLL_COLUMN_COUNT, 3);
        assert_eq!(PAPERDOLL_ROWS.len(), 5);
        assert_eq!(PAPERDOLL_ROWS[0].left, EquipPosition::HEAD_TOP);
        assert_eq!(PAPERDOLL_ROWS[0].right, EquipPosition::HEAD_MIDDLE);
        assert_eq!(PAPERDOLL_ROWS[1].left, EquipPosition::HEAD_LOWER);
        assert_eq!(PAPERDOLL_ROWS[1].right, EquipPosition::ARMOR);
        assert_eq!(PAPERDOLL_ROWS[2].left, EquipPosition::RIGHT_HAND);
        assert_eq!(PAPERDOLL_ROWS[2].right, EquipPosition::LEFT_HAND);
        assert_eq!(PAPERDOLL_ROWS[3].left, EquipPosition::GARMENT);
        assert_eq!(PAPERDOLL_ROWS[3].right, EquipPosition::SHOES);
        assert_eq!(PAPERDOLL_ROWS[4].left, EquipPosition::RIGTH_ACCESSORY);
        assert_eq!(PAPERDOLL_ROWS[4].right, EquipPosition::LEFT_ACCESSORY);

        let geometry = paperdoll_geometry(0.0, 0.0);
        assert!(geometry.portrait.height > geometry.portrait.width);
        assert!(geometry.portrait.height > GEAR_PORTRAIT_WIDTH);
        assert!((geometry.portrait.top - geometry.left_slots[0].top).abs() < f32::EPSILON);
        let last = geometry.left_slots[4];
        assert!(geometry.portrait.top + geometry.portrait.height + 0.1 >= last.top + last.height);
        for slot in geometry.left_slots.into_iter().chain(geometry.right_slots) {
            assert!(slot.is_square(), "body slot is not a fixed square");
        }
        assert!(geometry.ammo.is_square());
        let portrait_center = geometry.portrait.left + geometry.portrait.width / 2.0;
        let ammo_center = geometry.ammo.left + geometry.ammo.width / 2.0;
        assert!((portrait_center - ammo_center).abs() < 0.01);
        assert!(geometry.ammo.top > geometry.portrait.top);
        assert!(geometry.ammo.top + geometry.ammo.height <= geometry.portrait.top + geometry.portrait.height + 0.1);
        assert!((geometry.ammo.width - GEAR_SLOT_SIDE).abs() < f32::EPSILON);

        let shadow = square_strip_frames(SHADOW_SLOTS.len(), 0.0, 0.0);
        assert!(SHADOW_SLOTS.len() > STRIP_SLOTS_PER_ROW);
        assert!(shadow[STRIP_SLOTS_PER_ROW].top >= shadow[0].top + shadow[0].height);
        for frame in square_strip_frames(COSTUME_SLOTS.len(), 0.0, 0.0).into_iter().chain(shadow) {
            assert!(frame.is_square());
        }
        let costume = square_strip_frames(COSTUME_SLOTS.len(), 8.0, 4.0);
        for pair in costume.windows(2) {
            let left = pair[0];
            let right = pair[1];
            if (left.top - right.top).abs() < f32::EPSILON {
                let left_caption_right = left.left + left.width;
                assert!(left_caption_right <= right.left + 0.1, "captions share a band");
            }
        }
        assert!(strip_block_height(SHADOW_SLOTS.len()) > strip_block_height(COSTUME_SLOTS.len()));
        assert_eq!(COSTUME_SLOTS.len(), 4);
        assert_eq!(SHADOW_SLOTS.len(), 6);
        assert_eq!(COMBAT_STAT_LABELS, [
            "ATK", "MATK", "DEF", "MDEF", "HIT", "FLEE", "CRIT", "ASPD"
        ]);
        let base = packed_width(&BASE_STAT_PART_WIDTHS, STAT_PART_GAP);
        let combat = packed_width(&COMBAT_STAT_PART_WIDTHS, STAT_PART_GAP);
        assert!(base < GEAR_WINDOW_WIDTH / 2.0, "base stats spread too wide: {base}");
        assert!(combat < base);
        let paperdoll_width = GEAR_SLOT_SIDE * 2.0 + GEAR_PORTRAIT_WIDTH + GEAR_SLOT_GAP * 2.0;
        assert!(
            paperdoll_width <= GEAR_WINDOW_WIDTH / 2.0 + 8.0,
            "left half is wider than the stats half"
        );
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn inventory_grid_dimensions_match_standalone_window() {
        assert!(!GEAR_EMBEDS_INVENTORY);
        assert_eq!(GEAR_COLUMN_COUNT, 2);
        assert_eq!(INVENTORY_ROWS, 4);
        assert_eq!(INVENTORY_COLUMNS, 10);
    }

    #[test]
    fn every_shown_slot_has_a_stable_label() {
        for position in shown_equip_positions() {
            let label = equip_slot_label(position);
            let tooltip = equip_slot_tooltip(position);
            assert!(!label.is_empty());
            assert!(label.chars().count() <= EQUIP_LABEL_MAX_CHARS, "{label} is too long");
            assert!(!tooltip.is_empty());
        }
        assert_eq!(equip_slot_label(EquipPosition::COSTUME_HEAD_TOP), "C. Top");
        assert_eq!(equip_slot_tooltip(EquipPosition::COSTUME_HEAD_TOP), "Costume head top");
        assert_eq!(equip_slot_label(EquipPosition::COSTUME_HEAD_LOWER), "C. Low");
        assert_eq!(equip_slot_label(EquipPosition::SHADOW_WEAPON), "S.Wep");
        assert_eq!(equip_slot_tooltip(EquipPosition::SHADOW_WEAPON), "Shadow weapon");
        assert_eq!(equip_slot_label(EquipPosition::SHADOW_ARMOR), "S.Amm");
        assert_eq!(equip_slot_label(EquipPosition::SHADOW_LEFT_ACCESSORY), "S.L");
        assert_eq!(equip_slot_label(EquipPosition::SHADOW_RIGHT_ACCESSORY), "S.R");
        for position in shown_equip_positions() {
            assert!(
                slot_caption_fits(equip_slot_label(position)),
                "{} overflows a {}px socket",
                equip_slot_label(position),
                GEAR_SLOT_SIDE
            );
        }
        assert_eq!(equip_slot_tooltip(EquipPosition::SHADOW_ARMOR), "Shadow ammo");
        // LEFT_RIGHT_HAND includes RIGHT_HAND; historical body-slot preference.
        assert_eq!(equip_slot_label(EquipPosition::LEFT_RIGHT_HAND), "R. Hand");
        assert_eq!(equip_slot_tooltip(EquipPosition::LEFT_RIGHT_HAND), "Right hand");
        assert_eq!(EquipPosition::COSTUME_HEAD_LOWER.bits(), 4096);
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn loadout_controls_fit_under_the_paperdoll() {
        assert_eq!(LOADOUT_CONTROLS, ["+", "-"]);
        let (dropdown, plus, minus) = set_row_widths(300.0);
        assert!((plus + minus - dropdown / 2.0).abs() < 0.01);
        assert_eq!(display_aspd(550), 145);
        assert_eq!(display_aspd(2000), 0);
        assert_eq!(display_aspd(551), 144);
        assert_eq!(display_packet_stat(3), 3);
        assert_eq!(display_packet_stat(225), 225);
        assert!(PREVIEW_RENDERS_CHARACTER);
        assert!(GEAR_PORTRAIT_WIDTH > GEAR_SLOT_SIDE);
        let content = gear_left_column_content_height();
        let budget = gear_content_budget(GEAR_WINDOW_HEIGHT);
        assert!(
            content <= budget,
            "loadout controls need {content}px but the window body is {budget}px"
        );
    }

    #[test]
    fn player_stays_in_the_world_while_gear_is_open() {
        assert!(draw_player_in_world(true));
        assert!(draw_player_in_world(false));
    }

    #[test]
    fn portrait_part_stays_inside_the_frame() {
        let (x, y) = sprite_part_top_left(10, 20, 5, 7);
        assert_eq!((x, y), (8, 17));
        let (left, top, width, height) = fit_sprite_part(0, 0, 100, 200, 0, 0, 100, 200, 10.0, 20.0, 80.0);
        assert!((left - 30.0).abs() < 0.01);
        assert!((top - 20.0).abs() < 0.01);
        assert!((width - 40.0).abs() < 0.01);
        assert!((height - 80.0).abs() < 0.01);
        assert!(left >= 10.0 && top >= 20.0);
        assert!(left + width <= 90.0 + 0.01);
        assert!(top + height <= 100.0 + 0.01);
    }
}
