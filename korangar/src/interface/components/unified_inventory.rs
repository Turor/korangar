//! Controls of the unified Inventory window (Matthew t275u): accent tabs,
//! the A-Z / Recent / Zeny sort buttons, and the split panes for the cart
//! and storage.
//!
//! Tabs sit on a full-width `accent` underline. An idle tab has an
//! `accent_dim` outline; the selected tab stands taller, has an `accent_mid`
//! outline and runs down onto the underline. All colors are theme tokens.

use std::cell::{Cell, UnsafeCell};

use korangar_interface::MouseMode;
use korangar_interface::element::Element;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::event::{ClickHandler, DropHandler, EventQueue};
use korangar_interface::layout::area::{Area, PartialArea};
use korangar_interface::layout::{MouseButton, Resolver, Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{ButtonThemePathExt, HorizontalAlignment, VerticalAlignment, WindowThemePathExt};
use korangar_interface::theme::{ThemePathGetter, theme};
use rust_state::{Path, PathExt, Selector, State};

use crate::graphics::{Color, CornerDiameter, ShadowPadding};
use crate::input::{InputEvent, MouseInputMode};
use crate::interface::components::item_grid::{GridMetrics, grid_metrics, item_count_text};
use crate::interface::font_scale::from_theme_body;
use crate::interface::resource::ItemSource;
use crate::loaders::{FontSize, OverflowBehavior};
use crate::state::inventory::Inventory;
use crate::state::inventory_view::{InventorySort, InventoryTab, SORT_BUTTONS, favorite_drop};
use crate::state::theme::{InterfaceThemePathExt, ThemeTokensPathExt};
use crate::state::unified_inventory::{UnifiedInventory, split_columns};
use crate::state::{ClientState, ClientStatePathExt, client_state, client_theme};

/// Thickness of the accent underline under a tab row.
pub const TAB_UNDERLINE: f32 = 2.0;
/// Idle tabs start this much lower than the selected one.
pub const TAB_STEP: f32 = 2.0;
/// Width of the accent bar down the middle of the split.
pub const SPLIT_DIVIDER_WIDTH: f32 = 2.0;
/// Below this width a tab uses its short label.
const SHORT_LABEL_WIDTH: f32 = 54.0;

/// `count` equal cells across `area` with `gap` between them.
pub fn row_cells(area: Area, count: usize, gap: f32) -> Vec<Area> {
    if count == 0 {
        return Vec::new();
    }
    let gap = if gap.is_finite() { gap.max(0.0) } else { 0.0 };
    let width = ((area.width - gap * (count - 1) as f32) / count as f32).max(0.0);
    (0..count)
        .map(|index| Area {
            left: area.left + index as f32 * (width + gap),
            top: area.top,
            width,
            height: area.height,
        })
        .collect()
}

/// Outline rectangle for one tab in a row whose tabs are `tab_height` high.
/// The selected tab fills the whole height down to the underline; idle tabs
/// start [`TAB_STEP`] lower.
pub fn tab_outline(cell: Area, tab_height: f32, selected: bool) -> Area {
    let step = if selected { 0.0 } else { TAB_STEP.min(tab_height / 2.0) };
    Area {
        left: cell.left,
        top: cell.top + step,
        width: cell.width,
        height: (tab_height - step).max(1.0),
    }
}

/// The underline under a tab row.
pub fn tab_underline(row: Area, tab_height: f32) -> Area {
    Area {
        left: row.left,
        top: row.top + tab_height,
        width: row.width,
        height: TAB_UNDERLINE,
    }
}

/// Left pane, divider and right pane of the split: two equal panes with the
/// window gap on both sides of the divider.
pub fn split_areas(left: f32, width: f32, gaps: f32) -> (f32, f32, f32, f32) {
    let gaps = if gaps.is_finite() { gaps.max(0.0) } else { 0.0 };
    let pane = ((width - gaps * 2.0 - SPLIT_DIVIDER_WIDTH) / 2.0).max(0.0).floor();
    let divider = left + pane + gaps;
    let right = divider + SPLIT_DIVIDER_WIDTH + gaps;
    (left, pane, divider, right)
}

/// Grid of one pane of the split. The slot size is the single view's at this
/// window width; the pane draws [`split_columns`] of its columns.
pub fn split_grid_metrics(pane_width: f32, gaps: f32) -> GridMetrics {
    let gaps_used = if gaps.is_finite() { gaps.max(0.0) } else { 0.0 };
    let full = grid_metrics(pane_width * 2.0 + gaps_used * 2.0 + SPLIT_DIVIDER_WIDTH, gaps);
    GridMetrics {
        columns: split_columns(full.columns),
        slot: full.slot,
    }
}

fn flat() -> ShadowPadding {
    ShadowPadding::diagonal(0.0, 0.0)
}

fn none() -> Color {
    Color::rgba_u8(0, 0, 0, 0)
}

fn add_outlined<'a>(layout: &mut WindowLayout<'a, ClientState>, area: Area, outline: Color, fill: Color, corners: CornerDiameter) {
    layout.add_rectangle(area, corners, outline, none(), flat());
    layout.add_rectangle(
        Area {
            left: area.left + 1.0,
            top: area.top + 1.0,
            width: (area.width - 2.0).max(1.0),
            height: (area.height - 2.0).max(1.0),
        },
        corners,
        fill,
        none(),
        flat(),
    );
}

fn tab_height(state: &State<ClientState>) -> f32 {
    // The button height follows the font scale.
    (*state.get(&theme().button().height())).max(16.0)
}

fn body_font(state: &State<ClientState>) -> FontSize {
    FontSize(from_theme_body(*state.get(&client_theme().tokens().font_size()), 13.0))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TabKind {
    Category(InventoryTab),
    /// Inventory (`false`) or Cart (`true`).
    Pane(bool),
}

impl TabKind {
    fn label(self, width: f32) -> &'static str {
        let short = width < SHORT_LABEL_WIDTH;
        match self {
            Self::Category(tab) if short => short_tab_label(tab),
            Self::Category(tab) => tab.label(),
            Self::Pane(false) => "Inventory",
            Self::Pane(true) => "Cart",
        }
    }
}

/// Label for a narrow tab.
pub fn short_tab_label(tab: InventoryTab) -> &'static str {
    match tab {
        InventoryTab::All => "All",
        InventoryTab::Usable => "Use",
        InventoryTab::Equip => "Equip",
        InventoryTab::Ammo => "Ammo",
        InventoryTab::Etc => "Etc",
        InventoryTab::Cards => "Card",
        InventoryTab::Favorites => "Fav",
    }
}

struct TabHandler<P> {
    inventory_path: P,
    kind: TabKind,
}

impl<P> TabHandler<P>
where
    P: Path<ClientState, Inventory> + Copy + 'static,
{
    fn is_selected(&self, state: &State<ClientState>) -> bool {
        match self.kind {
            TabKind::Category(tab) => state.get(&self.inventory_path).view().tab() == tab,
            TabKind::Pane(cart) => state.get(&client_state().unified_inventory()).cart_view() == cart,
        }
    }
}

impl<P> ClickHandler<ClientState> for TabHandler<P>
where
    P: Path<ClientState, Inventory> + Copy + 'static,
{
    fn handle_click(&self, state: &State<ClientState>, _: &mut EventQueue<ClientState>) {
        match self.kind {
            TabKind::Category(tab) => state.update_value_with(self.inventory_path, move |inventory: &mut Inventory| {
                inventory.view_mut().set_tab(tab)
            }),
            TabKind::Pane(cart) => state.update_value_with(client_state().unified_inventory(), move |panes: &mut UnifiedInventory| {
                panes.show_cart(cart)
            }),
        }
    }
}

impl<P> DropHandler<ClientState> for TabHandler<P> {
    fn handle_drop(&self, _: &State<ClientState>, queue: &mut EventQueue<ClientState>, mouse_mode: &MouseMode<ClientState>) {
        let TabKind::Category(tab) = self.kind else {
            return;
        };
        if let MouseMode::Custom {
            mode: MouseInputMode::MoveItem {
                source: ItemSource::Inventory,
                item,
            },
        } = mouse_mode
            && let Some(favorite) = favorite_drop(tab, item)
        {
            queue.queue(InputEvent::SetItemFavorite {
                index: item.index,
                favorite,
            });
        }
    }
}

pub struct TabRowLayoutInfo {
    row: Area,
    tab_height: f32,
    cells: Vec<Area>,
}

/// A row of accent tabs: the category tabs (All ... Favorites) or the
/// Inventory / Cart pane tabs. Dropping a bag item on Favorites favorites
/// it, on another category tab unfavorites it.
pub struct AccentTabs<P> {
    tabs: Vec<TabHandler<P>>,
}

impl<P: Copy> AccentTabs<P> {
    pub fn categories(inventory_path: P) -> Self {
        Self {
            tabs: InventoryTab::ALL
                .into_iter()
                .map(|tab| TabHandler {
                    inventory_path,
                    kind: TabKind::Category(tab),
                })
                .collect(),
        }
    }

    pub fn panes(inventory_path: P) -> Self {
        Self {
            tabs: [false, true]
                .into_iter()
                .map(|cart| TabHandler {
                    inventory_path,
                    kind: TabKind::Pane(cart),
                })
                .collect(),
        }
    }
}

impl<P> Element<ClientState> for AccentTabs<P>
where
    P: Path<ClientState, Inventory> + Copy + 'static,
{
    type LayoutInfo = TabRowLayoutInfo;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        let tab_height = tab_height(state);
        with_single_resolver(resolvers, |resolver| {
            let row = resolver.with_height(tab_height + TAB_UNDERLINE);
            let cells = row_cells(Area { height: tab_height, ..row }, self.tabs.len(), 2.0);
            TabRowLayoutInfo { row, tab_height, cells }
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let is_move_mode = matches!(layout.get_mouse_mode(), MouseMode::Custom {
            mode: MouseInputMode::MoveItem { .. },
        });
        let tokens = client_theme().tokens();
        let accent = *state.get(&tokens.accent());
        let accent_dim = *state.get(&tokens.accent_dim());
        let accent_mid = *state.get(&tokens.accent_mid());
        let fill = *state.get(&tokens.raised());
        let text = *state.get(&tokens.text());
        let muted = *state.get(&tokens.muted());
        let font = body_font(state);
        let corners = CornerDiameter {
            top_left: 6.0,
            top_right: 6.0,
            bottom_left: 0.0,
            bottom_right: 0.0,
        };

        for (handler, cell) in self.tabs.iter().zip(&layout_info.cells) {
            let hovered = if is_move_mode {
                let hovered = cell.check().any_mouse_mode().run(layout);
                if hovered {
                    layout.set_hovered();
                }
                hovered
            } else {
                cell.check().run(layout)
            };
            let selected = handler.is_selected(state);
            let outline = if selected { accent_mid } else { accent_dim };
            let area = tab_outline(*cell, layout_info.tab_height, selected);
            add_outlined(layout, area, outline, fill, corners);
            let text_color = match (selected, hovered) {
                (true, _) | (false, true) => text,
                (false, false) => muted,
            };
            layout.add_text(
                area,
                handler.kind.label(cell.width),
                font,
                text_color,
                accent,
                HorizontalAlignment::Center { offset: 0.0, border: 2.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
            if hovered {
                match (is_move_mode, handler.kind) {
                    (true, TabKind::Category(_)) => layout.register_drop_handler(handler),
                    (true, TabKind::Pane(_)) => {}
                    (false, _) => layout.register_click_handler(MouseButton::Left, handler),
                }
            }
        }

        layout.add_rectangle(
            tab_underline(layout_info.row, layout_info.tab_height),
            CornerDiameter::uniform(0.0),
            accent,
            none(),
            flat(),
        );
    }
}

struct SortClick<P> {
    inventory_path: P,
    sort: InventorySort,
}

impl<P> ClickHandler<ClientState> for SortClick<P>
where
    P: Path<ClientState, Inventory> + Copy + 'static,
{
    fn handle_click(&self, state: &State<ClientState>, _: &mut EventQueue<ClientState>) {
        let sort = self.sort;
        state.update_value_with(self.inventory_path, move |inventory: &mut Inventory| {
            inventory.view_mut().click_sort(sort)
        });
    }
}

/// Bars of a small arrow, top row first: `(left, top, width, height)`
/// offsets inside a 9 x 6 box. Up arrows are narrow at the top.
pub fn arrow_bars(up: bool) -> [(f32, f32, f32, f32); 3] {
    let rows = [(3.0, 3.0), (1.5, 6.0), (0.0, 9.0)];
    let mut bars = [(0.0, 0.0, 0.0, 0.0); 3];
    for (row, (inset, width)) in rows.into_iter().enumerate() {
        let slot = if up { row } else { 2 - row };
        bars[slot] = (inset, slot as f32 * 2.0, width, 2.0);
    }
    bars
}

pub struct SortRowLayoutInfo {
    cells: Vec<Area>,
}

/// A-Z, Recent and Zeny. The active one has the `accent_mid` outline and an
/// arrow for its direction; a second click reverses it, a third goes back
/// to the server order.
pub struct SortButtons<P> {
    buttons: Vec<SortClick<P>>,
}

impl<P: Copy> SortButtons<P> {
    pub fn new(inventory_path: P) -> Self {
        Self {
            buttons: SORT_BUTTONS.into_iter().map(|sort| SortClick { inventory_path, sort }).collect(),
        }
    }
}

impl<P> Element<ClientState> for SortButtons<P>
where
    P: Path<ClientState, Inventory> + Copy + 'static,
{
    type LayoutInfo = SortRowLayoutInfo;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        let height = tab_height(state);
        let gaps = *state.get(&theme().window().gaps());
        with_single_resolver(resolvers, |resolver| {
            let row = resolver.with_height(height);
            SortRowLayoutInfo {
                cells: row_cells(row, self.buttons.len(), gaps),
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
        let tokens = client_theme().tokens();
        let accent = *state.get(&tokens.accent());
        let accent_dim = *state.get(&tokens.accent_dim());
        let accent_mid = *state.get(&tokens.accent_mid());
        let fill = *state.get(&tokens.raised());
        let text = *state.get(&tokens.text());
        let muted = *state.get(&tokens.muted());
        let font = body_font(state);
        let Some(first) = self.buttons.first() else {
            return;
        };
        let view = state.get(&first.inventory_path).view();
        let (current, reversed) = (view.sort(), view.reversed());

        for (button, cell) in self.buttons.iter().zip(&layout_info.cells) {
            let hovered = cell.check().run(layout);
            let active = current == button.sort;
            let outline = if active || hovered { accent_mid } else { accent_dim };
            add_outlined(layout, *cell, outline, fill, CornerDiameter::uniform(4.0));
            let arrow_room = if active { 14.0 } else { 0.0 };
            layout.add_text(
                Area {
                    width: (cell.width - arrow_room).max(1.0),
                    ..*cell
                },
                button.sort.button_label(),
                font,
                if active || hovered { text } else { muted },
                accent,
                HorizontalAlignment::Center { offset: 0.0, border: 2.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
            if active {
                let left = cell.left + cell.width - 14.0;
                let top = cell.top + (cell.height - 6.0) / 2.0;
                for (x, y, width, height) in arrow_bars(button.sort.arrow_up(reversed)) {
                    layout.add_rectangle(
                        Area {
                            left: left + x,
                            top: top + y,
                            width,
                            height,
                        },
                        CornerDiameter::uniform(0.0),
                        accent,
                        none(),
                        flat(),
                    );
                }
            }
            if hovered {
                layout.register_click_handler(MouseButton::Left, button);
            }
        }
    }
}

pub struct DualPaneLayoutInfo<L, R> {
    divider: Area,
    left: L,
    right: R,
}

/// Two panes side by side with an `accent` bar down the split. Each pane is
/// a column of elements (a header line and a scrolling grid).
pub struct DualPane<L, R> {
    left: L,
    right: R,
}

impl<L, R> DualPane<L, R> {
    pub fn new(left: L, right: R) -> Self {
        Self { left, right }
    }
}

impl<L, R> Element<ClientState> for DualPane<L, R>
where
    L: Element<ClientState>,
    R: Element<ClientState>,
{
    type LayoutInfo = DualPaneLayoutInfo<L::LayoutInfo, R::LayoutInfo>;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        mut store: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        let gaps = *state.get(&theme().window().gaps());
        with_single_resolver(resolvers, |resolver| {
            let available = resolver.push_available_area();
            let (left, pane, divider, right) = split_areas(available.left, available.width, gaps);
            let pane_area = |left: f32| PartialArea {
                left,
                top: available.top,
                width: pane,
                height: available.height,
            };
            let text_layouter = resolver.get_text_layouter();
            let mut left_resolver = Resolver::new(pane_area(left), gaps, text_layouter);
            let left_info = self.left.create_layout_info(state, store.child_store(0), &mut left_resolver);
            let mut right_resolver = Resolver::new(pane_area(right), gaps, text_layouter);
            let right_info = self.right.create_layout_info(state, store.child_store(1), &mut right_resolver);
            let used = left_resolver.get_used_height().max(right_resolver.get_used_height());
            resolver.commit_used_height(used);
            DualPaneLayoutInfo {
                divider: Area {
                    left: divider,
                    top: available.top,
                    width: SPLIT_DIVIDER_WIDTH,
                    height: used,
                },
                left: left_info,
                right: right_info,
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
        self.left.lay_out(state, store.child_store(0), &layout_info.left, layout);
        self.right.lay_out(state, store.child_store(1), &layout_info.right, layout);
        let accent = *state.get(&client_theme().tokens().accent());
        layout.add_rectangle(layout_info.divider, CornerDiameter::uniform(1.0), accent, none(), flat());
    }
}

/// "Storage · Items 37 / 600" over one pane.
pub struct PaneLabel<P> {
    title: &'static str,
    inventory_path: P,
    fallback_capacity: Option<u32>,
    last: Cell<Option<(usize, Option<u32>)>>,
    text: UnsafeCell<String>,
}

impl<P> PaneLabel<P> {
    pub fn new(title: &'static str, inventory_path: P, fallback_capacity: Option<u32>) -> Self {
        Self {
            title,
            inventory_path,
            fallback_capacity,
            last: Cell::default(),
            text: UnsafeCell::default(),
        }
    }
}

/// Header text of a pane.
pub fn pane_label_text(title: &str, used: usize, capacity: Option<u32>) -> String {
    format!("{title} · {}", item_count_text(used, capacity))
}

impl<P> Selector<ClientState, String> for PaneLabel<P>
where
    P: Path<ClientState, Inventory>,
{
    fn select<'a>(&'a self, state: &'a ClientState) -> Option<&'a String> {
        let inventory = self.inventory_path.follow_safe(state);
        let key = (inventory.item_count(), inventory.capacity().or(self.fallback_capacity));
        if self.last.get() != Some(key) {
            unsafe {
                *self.text.get() = pane_label_text(self.title, key.0, key.1);
            }
            self.last.set(Some(key));
        }
        unsafe { Some(self.text.as_ref_unchecked()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(width: f32) -> Area {
        Area {
            left: 10.0,
            top: 0.0,
            width,
            height: 24.0,
        }
    }

    #[test]
    fn tab_cells_fill_the_row() {
        let cells = row_cells(area(300.0), 7, 2.0);
        assert_eq!(cells.len(), 7);
        let last = cells[6];
        assert!((last.left + last.width - 310.0).abs() < 0.01);
        assert!(
            cells
                .windows(2)
                .all(|pair| (pair[1].left - pair[0].left - pair[0].width - 2.0).abs() < 0.01)
        );
        assert!(row_cells(area(300.0), 0, 2.0).is_empty());
    }

    #[test]
    fn selected_tab_stands_taller_and_meets_the_underline() {
        let cell = area(40.0);
        let selected = tab_outline(cell, 24.0, true);
        let idle = tab_outline(cell, 24.0, false);
        let underline = tab_underline(cell, 24.0);
        assert_eq!(selected.top, 0.0);
        assert_eq!(idle.top, TAB_STEP);
        // Both end where the underline starts.
        assert_eq!(selected.top + selected.height, underline.top);
        assert_eq!(idle.top + idle.height, underline.top);
        assert_eq!(underline.height, TAB_UNDERLINE);
    }

    #[test]
    fn narrow_tabs_use_short_labels() {
        assert_eq!(TabKind::Category(InventoryTab::Favorites).label(80.0), "Favorites");
        assert_eq!(TabKind::Category(InventoryTab::Favorites).label(40.0), "Fav");
        assert_eq!(TabKind::Category(InventoryTab::Usable).label(40.0), "Use");
        assert_eq!(TabKind::Pane(true).label(40.0), "Cart");
    }

    #[test]
    fn split_panes_are_equal_around_the_divider() {
        let (left, pane, divider, right) = split_areas(8.0, 384.0, 6.0);
        assert_eq!(left, 8.0);
        assert_eq!(pane, 185.0);
        assert_eq!(divider, 8.0 + 185.0 + 6.0);
        assert_eq!(right, divider + SPLIT_DIVIDER_WIDTH + 6.0);
        assert!(right + pane <= 8.0 + 384.0 + 0.01);
        assert_eq!(split_areas(0.0, 4.0, 6.0).1, 0.0);
    }

    #[test]
    fn split_grids_take_half_the_columns_less_one_at_the_same_slot_size() {
        let gaps = 6.0;
        for width in [240.0_f32, 384.0, 400.0, 620.0, 900.0, 1_184.0] {
            let single = grid_metrics(width, gaps);
            let (_, pane, ..) = split_areas(0.0, width, gaps);
            let split = split_grid_metrics(pane, gaps);
            assert_eq!(split.columns, split_columns(single.columns), "{width}");
            assert!((split.slot - single.slot).abs() <= 1.0, "{width}: {split:?} vs {single:?}");
            // Room is left in each pane beside the grid.
            assert!(split.row_width(gaps) < pane, "{width}: {split:?} in {pane}");
        }
        // The default 400-wide window: 10 columns, so 4 per pane.
        let (_, pane, ..) = split_areas(0.0, 384.0, gaps);
        assert_eq!(split_grid_metrics(pane, gaps).columns, 4);
    }

    #[test]
    fn arrows_point_the_right_way() {
        let up = arrow_bars(true);
        let down = arrow_bars(false);
        assert!(up[0].2 < up[2].2, "up arrows are narrow at the top");
        assert!(down[0].2 > down[2].2, "down arrows are wide at the top");
        assert!(up.iter().all(|bar| bar.1 + bar.3 <= 6.0));
    }

    #[test]
    fn pane_label() {
        assert_eq!(pane_label_text("Storage", 37, Some(600)), "Storage · Items 37 / 600");
        assert_eq!(pane_label_text("Cart", 3, None), "Cart · Items 3");
    }
}
