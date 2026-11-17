//! Hotbar, headless (Matthew t274u).
//!
//! [`offline_hotbar_scenario`] opens the hotbar at its first-open size,
//! drags its top-right corner in (narrower, taller) with the scripted input
//! path and the same [`resize_window_area`] the interface uses, and sizes the
//! slot grid for each region with [`hotbar_metrics`]: rows never wrap
//! (Matthew t280u), extra height becomes padding, and laying the window out
//! again does not change its height. It saves and reloads the size through
//! `window_cache.ron` and checks a reset forgets it. It then plays the cooldown
//! and cast timeline the client builds from `0x0B1A`, `0x043D` and
//! `EFST_POSTDELAY` through [`SkillCooldowns`] and [`CastBars`].
//!
//! Two more sizes: two rows in the smallest window the limits allow, and the
//! first-open bar at the largest global font scale. Both must keep the grid
//! inside the window width.

use std::mem::ManuallyDrop;
use std::path::Path;

use korangar_interface::application::WindowCache as _;
use korangar_interface::layout::ResizeMode;
use korangar_interface::layout::area::Area;
use korangar_interface::window::{ResizeLimits, resize_handle_areas, resize_handle_at, resize_window_area};
use ragnarok_packets::{ClientTick, SkillId};

use super::resizable_windows::{RESIZE_SCREEN, scripted_drag};
use crate::graphics::{ScreenPosition, ScreenSize};
use crate::interface::components::hotbar_layout::{
    HOTBAR_DEFAULT_HEIGHT, HOTBAR_GRAB_HEIGHT, HOTBAR_HINT_FONT_PX, HOTBAR_WINDOW_BORDER, HOTBAR_WINDOW_MAXIMUM_HEIGHT,
    HOTBAR_WINDOW_MAXIMUM_WIDTH, HOTBAR_WINDOW_MINIMUM_HEIGHT, HOTBAR_WINDOW_MINIMUM_WIDTH, HotbarMetrics, hint_strip_height,
    hotbar_metrics, laid_out_window_height,
};
use crate::interface::font_scale::{FONT_SCALE_MAX, font_scale_factor, from_theme_body};
use crate::interface::windows::{WindowCache, WindowClass};
use crate::state::skill_cooldowns::{SkillCooldowns, SlotDelay};
use crate::state::theme::InterfaceTheme;
use crate::world::effects::{CastBars, CastPhase};

/// Corner drag: 220px narrower and 120px taller.
pub const HOTBAR_NARROW_DRAG: (f32, f32) = (-220.0, -120.0);

/// One hotbar window as drawn and its grid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HotbarSnapshot {
    pub width: f32,
    pub height: f32,
    /// Width the grid gets inside the window borders.
    pub region_width: f32,
    /// Window height after the grid grew it (never below `height`).
    pub laid_out_height: f32,
    /// Height after 100 more layout passes, each fed the last result.
    pub settled_height: f32,
    pub metrics: HotbarMetrics,
}

/// One moment of the cooldown timeline.
#[derive(Clone, Debug, PartialEq)]
pub struct TimelinePoint {
    pub at_ms: u32,
    /// Overlay on the used skill (`Cooldown` with seconds, or `AfterCast`).
    pub used_skill: Option<SlotDelay>,
    /// Overlay on another skill (only the after-cast dim).
    pub other_skill: Option<SlotDelay>,
    /// Phase of the bar under the player, if one shows.
    pub bar: Option<CastPhase>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OfflineHotbarReport {
    pub grabbed: Option<ResizeMode>,
    pub before: HotbarSnapshot,
    pub narrowed: HotbarSnapshot,
    pub reloaded: HotbarSnapshot,
    pub reset_forgets: bool,
    /// Two rows in the minimum window.
    pub minimum_two_rows: HotbarSnapshot,
    /// First-open bar at the largest font scale.
    pub font_scaled: HotbarSnapshot,
    /// Same bar at the default font scale, for comparison.
    pub font_default: HotbarSnapshot,
    pub timeline: Vec<TimelinePoint>,
}

/// Hotbar window limits (`hotbar.rs`).
pub fn hotbar_limits() -> ResizeLimits {
    ResizeLimits::new(
        HOTBAR_WINDOW_MINIMUM_WIDTH,
        HOTBAR_WINDOW_MAXIMUM_WIDTH,
        HOTBAR_WINDOW_MINIMUM_HEIGHT,
        HOTBAR_WINDOW_MAXIMUM_HEIGHT,
    )
}

/// Grid for a hotbar window of this size with one row shown: the grab strip
/// above, the window border on the sides and below.
pub fn hotbar_window_grid(width: f32, height: f32) -> HotbarMetrics {
    hotbar_window_snapshot(1, width, height, 1.0).metrics
}

/// `rows` rows drawn in a window of this size with the global font scale at
/// `font_factor`, as `hotbar_grid.rs` lays it out.
pub fn hotbar_window_snapshot(rows: usize, width: f32, height: f32, font_factor: f32) -> HotbarSnapshot {
    let theme = InterfaceTheme::turoran_dark();
    let chrome = HOTBAR_GRAB_HEIGHT + HOTBAR_WINDOW_BORDER;
    let region_width = width - HOTBAR_WINDOW_BORDER * 2.0;
    let hint_height = hint_strip_height(from_theme_body(theme.tokens.font_size, HOTBAR_HINT_FONT_PX) * font_factor);
    let metrics = hotbar_metrics(rows, region_width, Some(height - chrome), hint_height);
    let laid_out_height = laid_out_window_height(height, rows, width, hint_height).max(height);
    let settled_height = (0..100).fold(laid_out_height, |window, _| {
        laid_out_window_height(window, rows, width, hint_height)
    });
    HotbarSnapshot {
        width,
        height,
        region_width,
        laid_out_height: laid_out_height.min(HOTBAR_WINDOW_MAXIMUM_HEIGHT),
        settled_height,
        metrics,
    }
}

fn snapshot(area: Area) -> HotbarSnapshot {
    hotbar_window_snapshot(1, area.width, area.height, 1.0)
}

fn save(cache: &mut WindowCache, area: Area) {
    let mut anchor = cache
        .get_window_state(WindowClass::Hotbar, RESIZE_SCREEN)
        .map(|(anchor, _)| anchor)
        .unwrap_or_default();
    let size = ScreenSize {
        width: area.width,
        height: area.height,
    };
    anchor.update(
        RESIZE_SCREEN,
        ScreenPosition {
            left: area.left,
            top: area.top,
        },
        size,
    );
    cache.update_anchor(WindowClass::Hotbar, anchor);
    cache.update_size(WindowClass::Hotbar, size);
}

fn restore(cache: &WindowCache) -> Result<Area, String> {
    let (anchor, size) = cache
        .get_window_state(WindowClass::Hotbar, RESIZE_SCREEN)
        .ok_or("no Hotbar state")?;
    let position = anchor.to_position(RESIZE_SCREEN);
    Ok(Area {
        left: position.left,
        top: position.top,
        width: size.width,
        height: size.height,
    })
}

/// Cooldown and cast timeline for one cast of `skill` (cast 1 s, after-cast
/// 0.5 s, cooldown 3 s), sampled at fixed times.
pub fn cooldown_timeline() -> Vec<TimelinePoint> {
    const PLAYER: u32 = 150_000;
    const SKILL: SkillId = SkillId(19);
    const OTHER: SkillId = SkillId(14);
    let mut cooldowns = SkillCooldowns::default();
    let mut bars = CastBars::default();
    let mut timeline = Vec::new();
    let mut sample = |at_ms: u32, cooldowns: &SkillCooldowns, bars: &CastBars| {
        timeline.push(TimelinePoint {
            at_ms,
            used_skill: cooldowns.slot_delay(SKILL, ClientTick(at_ms)),
            other_skill: cooldowns.slot_delay(OTHER, ClientTick(at_ms)),
            bar: bars.iter_visible(at_ms).find(|bar| bar.entity_id == PLAYER).map(|bar| bar.phase),
        });
    };
    // 0x0B1A: cast time 1000 ms.
    bars.begin(PLAYER, SKILL.0, 1_000, 0);
    sample(500, &cooldowns, &bars);
    // Cast ends: 0x043D (3000 ms) and EFST_POSTDELAY (500 ms) arrive.
    cooldowns.set(SKILL, 3_000, ClientTick(1_000));
    cooldowns.start_after_cast(500, 500, ClientTick(1_000));
    bars.begin_after_cast(PLAYER, 500, 1_000);
    sample(1_200, &cooldowns, &bars);
    sample(2_000, &cooldowns, &bars);
    bars.expire(4_100);
    cooldowns.prune(ClientTick(4_100));
    sample(4_100, &cooldowns, &bars);
    timeline
}

/// Run the scenario. `directory` holds the saved cache file.
pub fn offline_hotbar_scenario(directory: &Path) -> Result<OfflineHotbarReport, String> {
    let mut cache = ManuallyDrop::new(WindowCache::default());
    let limits = hotbar_limits();
    let start = Area {
        left: (RESIZE_SCREEN.width - 420.0) / 2.0,
        top: RESIZE_SCREEN.height - HOTBAR_DEFAULT_HEIGHT - 8.0,
        width: limits.clamp_width(420.0),
        height: limits.clamp_height(HOTBAR_DEFAULT_HEIGHT),
    };
    let before = snapshot(start);

    let border = InterfaceTheme::turoran_dark().window.border;
    let (_, corner) = resize_handle_areas(start, border)
        .into_iter()
        .find(|(mode, _)| *mode == ResizeMode::TOP_RIGHT)
        .ok_or("no top-right handle")?;
    let press = (corner.left + corner.width / 2.0, corner.top + corner.height / 2.0);
    let mode = resize_handle_at(start, border, press.0, press.1, limits.allows_width(), true).ok_or("the corner is not a resize handle")?;
    let deltas = scripted_drag(press, (press.0 + HOTBAR_NARROW_DRAG.0, press.1 + HOTBAR_NARROW_DRAG.1));
    let area = deltas.iter().fold(start, |area, delta| {
        resize_window_area(
            area,
            mode,
            delta.width,
            delta.height,
            1.0,
            limits,
            RESIZE_SCREEN.width,
            RESIZE_SCREEN.height,
        )
    });
    let narrowed = snapshot(area);

    save(&mut cache, area);
    let path = directory.join("window_cache.ron");
    cache.save_to(&path).map_err(|error| format!("save {}: {error}", path.display()))?;
    let reloaded_cache = ManuallyDrop::new(WindowCache::load_from(&path).ok_or("saved window cache did not load")?);
    let reloaded = snapshot(restore(&reloaded_cache)?);

    let mut reset_cache = ManuallyDrop::new(WindowCache::load_from(&path).ok_or("saved window cache did not load")?);
    reset_cache.forget_all();
    let reset_forgets = !reset_cache.has_saved_state(WindowClass::Hotbar);

    Ok(OfflineHotbarReport {
        grabbed: Some(mode),
        before,
        narrowed,
        reloaded,
        reset_forgets,
        minimum_two_rows: hotbar_window_snapshot(2, HOTBAR_WINDOW_MINIMUM_WIDTH, HOTBAR_WINDOW_MINIMUM_HEIGHT, 1.0),
        font_scaled: hotbar_window_snapshot(1, 420.0, HOTBAR_DEFAULT_HEIGHT, font_scale_factor(FONT_SCALE_MAX)),
        font_default: hotbar_window_snapshot(1, 420.0, HOTBAR_DEFAULT_HEIGHT, 1.0),
        timeline: cooldown_timeline(),
    })
}

/// Pointer flow on a two-slot hotbar (Matthew t278u): right-click each slot,
/// drag the skill onto the item's slot (swap), cancel a drag with Escape,
/// then drag each one off the bar (world, then a window that ignores it).
#[derive(Clone, Debug, PartialEq)]
pub struct HotbarPointerReport {
    /// Title of the window each right-click opens, in order: skill, item,
    /// empty slot.
    pub right_click_titles: Vec<Option<String>>,
    /// Right-click opened the skill description and the item details.
    pub right_click_kinds: Vec<&'static str>,
    /// Slot kinds after the right-clicks (they must not clear).
    pub after_right_click: [u8; 2],
    /// Slot kinds after dropping the skill on the item's slot.
    pub after_swap: [u8; 2],
    /// Hotkey updates the swap sends: (slot, is_skill).
    pub swap_updates: Vec<(u16, bool)>,
    /// Escape cancelled the drag, and releasing afterwards cleared nothing.
    pub escape_cancels: bool,
    pub cleared_after_escape: bool,
    /// Slots cleared by dropping on the world and on a window that ignores
    /// the drag.
    pub cleared_off_bar: Vec<u16>,
    /// Slot kinds at the end.
    pub after_drag_off: [u8; 2],
}

/// Run [`HotbarPointerReport`] offline.
pub fn offline_hotbar_pointer() -> HotbarPointerReport {
    use korangar_interface::MouseMode;
    use korangar_networking::{InventoryItem, InventoryItemDetails};
    use ragnarok_packets::{EquipPosition, HotbarSlot, HotbarTab, InventoryIndex, ItemId, RegularItemFlags, SkillLevel};

    use crate::input::{InputEvent, MouseInputMode};
    use crate::interface::components::hotbar_actions::{
        DragRelease, escape_cancels_drag, hotbar_drag_source, hotbar_info_event, hotbar_item, slot_to_clear_on_release,
    };
    use crate::interface::components::item_tooltip::ItemTooltipText;
    use crate::interface::resource::{ItemSource, SkillSource};
    use crate::state::hotbar::Hotbar;
    use crate::state::skills::{LearnableSkill, SkillAcquisition};
    use crate::world::{ResourceMetadata, SpriteAnimationState};

    const TAB: HotbarTab = HotbarTab(0);
    const SKILL: HotbarSlot = HotbarSlot(0);
    const ITEM: HotbarSlot = HotbarSlot(1);
    let endure = LearnableSkill {
        skill_id: SkillId(8),
        maximum_level: SkillLevel(1),
        file_name: "sm_endure".to_owned(),
        skill_name: "Endure".to_owned(),
        description: "Keep moving while hit.".to_owned(),
        spell_point_costs: vec![10],
        can_select_level: false,
        acquisition: SkillAcquisition::Job,
        required_skills: Default::default(),
        required_for_skills: Default::default(),
        sprite: None,
        actions: None,
        animation_state: SpriteAnimationState::new(ClientTick(0)),
    };
    let potion = InventoryItem {
        metadata: ResourceMetadata {
            texture: None,
            name: "Red Potion".to_owned(),
            description: String::new(),
        },
        index: InventoryIndex(2),
        item_id: ItemId(501),
        item_type: 0,
        slot: [0; 4],
        hire_expiration_date: 0,
        details: InventoryItemDetails::Regular {
            amount: 7,
            equipped_position: EquipPosition::empty(),
            flags: RegularItemFlags::IDENTIFIED,
        },
    };
    let inventory = [potion.clone()];
    let held = |item_id: ItemId| inventory.iter().find(|item| item.item_id == item_id).cloned();
    let text = |item: &InventoryItem<ResourceMetadata>| ItemTooltipText {
        body: format!("{} (inventory text)", item.metadata.name),
        compare: String::new(),
    };

    let mut hotbar = Hotbar::default();
    hotbar.set_skill_local(TAB, SKILL, endure);
    hotbar.set_item_local(TAB, ITEM, 501, 7);
    let kinds = |hotbar: &Hotbar| [SKILL, ITEM].map(|slot| hotbar.cell(TAB, slot).map(|cell| cell.kind).unwrap_or(u8::MAX));

    let mut right_click_titles = Vec::new();
    let mut right_click_kinds = Vec::new();
    for slot in [SKILL, ITEM, HotbarSlot(5)] {
        let cell = hotbar.cell(TAB, slot).filter(|cell| cell.kind != crate::state::hotbar::CELL_EMPTY);
        match hotbar_info_event(cell, held, text) {
            Some(InputEvent::ToggleSkillDescription { title, .. }) => {
                right_click_titles.push(Some(title));
                right_click_kinds.push("skill description");
            }
            Some(InputEvent::OpenItemDetails { title, .. }) => {
                right_click_titles.push(Some(title));
                right_click_kinds.push("item details");
            }
            Some(_) => right_click_kinds.push("other"),
            None => right_click_titles.push(None),
        }
    }
    let after_right_click = kinds(&hotbar);

    // Drag the skill onto the item's slot: the slot's drop handler queues
    // MoveSkill Hotbar -> Hotbar, which swaps.
    let swap_updates = hotbar
        .swap_local(TAB, SKILL, TAB, ITEM)
        .map(|updates| updates.iter().map(|(_, slot, data)| (slot.0, data.is_skill())).collect())
        .unwrap_or_default();
    let after_swap = kinds(&hotbar);

    // Pick up the item (now in SKILL) and press Escape.
    let item_cell = hotbar.cell(TAB, SKILL).cloned();
    let item = item_cell
        .as_ref()
        .and_then(|cell| hotbar_item(cell, held(ItemId(cell.item_id)).as_ref()));
    let mut mouse_mode: MouseMode<crate::state::ClientState> = MouseMode::Custom {
        mode: MouseInputMode::MoveItem {
            item: item.unwrap_or(potion),
            source: ItemSource::Hotbar { tab: TAB, slot: SKILL },
        },
    };
    let escape_cancels = escape_cancels_drag(&mouse_mode);
    if escape_cancels {
        mouse_mode = MouseMode::Default;
    }
    let off_bar = DragRelease {
        over_interface: false,
        over_hotbar: false,
        accepted: false,
    };
    let cleared_after_escape = slot_to_clear_on_release(hotbar_drag_source(&mouse_mode), off_bar).is_some();

    // Drag the item off onto the world, then the skill onto a window that
    // ignores it (for example chat).
    let mut cleared_off_bar = Vec::new();
    let skill = hotbar.cell(TAB, ITEM).and_then(|cell| cell.skill.clone());
    let drags = [
        (
            MouseMode::Custom {
                mode: MouseInputMode::MoveItem {
                    item: hotbar_item(hotbar.cell(TAB, SKILL).unwrap(), None).unwrap(),
                    source: ItemSource::Hotbar { tab: TAB, slot: SKILL },
                },
            },
            off_bar,
        ),
        (
            MouseMode::Custom {
                mode: MouseInputMode::MoveSkill {
                    skill: skill.expect("the swapped skill"),
                    source: SkillSource::Hotbar { tab: TAB, slot: ITEM },
                },
            },
            DragRelease {
                over_interface: true,
                over_hotbar: false,
                accepted: false,
            },
        ),
    ];
    for (mode, release) in drags {
        if let Some((tab, slot)) = slot_to_clear_on_release(hotbar_drag_source(&mode), release) {
            hotbar.clear_local(tab, slot);
            cleared_off_bar.push(slot.0);
        }
    }

    HotbarPointerReport {
        right_click_titles,
        right_click_kinds,
        after_right_click,
        after_swap,
        swap_updates,
        escape_cancels,
        cleared_after_escape,
        cleared_off_bar,
        after_drag_off: kinds(&hotbar),
    }
}
