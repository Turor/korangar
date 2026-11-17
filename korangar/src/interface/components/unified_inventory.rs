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
use korangar_interface::layout::tooltip::TooltipExt;
use korangar_interface::layout::{MouseButton, Resolver, Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{ButtonThemePathExt, HorizontalAlignment, VerticalAlignment, WindowThemePathExt};
use korangar_interface::theme::{ThemePathGetter, theme};
use rust_state::{Path, PathExt, Selector, State};

use crate::graphics::{Color, CornerDiameter, ShadowPadding};
use crate::input::{InputEvent, MouseInputMode};
use crate::interface::components::inventory_controls::WEIGHT_BAR_HEIGHT;
use crate::interface::components::item_grid::{GridMetrics, grid_metrics, item_count_text};
use crate::interface::font_scale::from_theme_body;
use crate::interface::resource::ItemSource;
use crate::loaders::{FontSize, OverflowBehavior};
use crate::state::inventory::Inventory;
use crate::state::inventory_view::{InventorySort, InventoryTab, SORT_BUTTONS, favorite_drop};
use crate::state::theme::{InterfaceThemePathExt, ThemeTokensPathExt};
use crate::state::unified_inventory::{PaneTab, StorageKind, split_columns};
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

/// Heights the inventory window stacks above its item grids, read from the
/// theme. Mirrors `windows::inventory`: the Inventory / Cart / Storage tabs,
/// the category tabs, search, sort, the optional storage tool row, weight and
/// Zeny, the weight bar, then the count line over each grid.
#[derive(Clone, Copy, Debug)]
pub struct InventoryLayoutMetrics {
    pub title: f32,
    pub title_gap: f32,
    pub border: f32,
    pub gaps: f32,
    pub tab: f32,
    pub text_box: f32,
    pub text: f32,
}

impl InventoryLayoutMetrics {
    pub fn from_theme(theme: &crate::state::theme::InterfaceTheme) -> Self {
        Self {
            title: theme.window.title_height,
            title_gap: theme.window.title_gap,
            border: theme.window.border,
            gaps: theme.window.gaps,
            tab: theme.button.height.max(16.0),
            text_box: theme.text_box.height,
            text: theme.text.height,
        }
    }

    /// Window height down to the top of the item grids.
    pub fn grid_top(&self, storage_tools: bool) -> f32 {
        let tabs = self.tab + TAB_UNDERLINE;
        // Pane tabs, category tabs, search, sort, weight | Zeny, bar, count.
        let mut rows = vec![tabs, tabs, self.text_box, self.tab, self.text, WEIGHT_BAR_HEIGHT, self.text];
        if storage_tools {
            rows.push(storage_tool_row_height(self.tab));
        }
        // Every element is followed by a gap, the last one by the gap
        // before the scroll view.
        self.title + self.title_gap + rows.iter().sum::<f32>() + self.gaps * rows.len() as f32
    }

    /// Item rows fully visible in a window of this size.
    #[cfg(test)]
    pub fn visible_rows(&self, width: f32, height: f32, split: bool, storage_tools: bool) -> usize {
        let viewport = height - self.grid_top(storage_tools) - self.border;
        let slot = self.slot(width, split);
        (((viewport + self.gaps) / (slot + self.gaps)).floor().max(0.0)) as usize
    }

    /// Window height that shows `rows` item rows.
    pub fn height_for_rows(&self, rows: usize, width: f32, split: bool, storage_tools: bool) -> f32 {
        let slot = self.slot(width, split);
        let grid = rows as f32 * (slot + self.gaps) - self.gaps;
        (self.grid_top(storage_tools) + grid + self.border).ceil()
    }

    fn slot(&self, width: f32, split: bool) -> f32 {
        let content = (width - self.border * 2.0).max(0.0);
        match split {
            true => split_grid_metrics(split_areas(0.0, content, self.gaps).1, self.gaps).slot,
            false => grid_metrics(content, self.gaps).slot,
        }
    }
}

/// The storage tool row: saved-search chips and the Save / Deposit / Close
/// buttons on one line.
pub fn storage_tool_row_height(tab: f32) -> f32 {
    tab.max(STORAGE_CHIP_HEIGHT)
}

/// Height of a saved-search chip.
pub const STORAGE_CHIP_HEIGHT: f32 = 22.0;

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
    /// Inventory, Cart, Storage or Guild Storage.
    Pane(PaneTab),
}

impl TabKind {
    fn label(self, width: f32) -> &'static str {
        let short = width < SHORT_LABEL_WIDTH;
        match self {
            Self::Category(tab) if short => short_tab_label(tab),
            Self::Category(tab) => tab.label(),
            Self::Pane(PaneTab::Inventory) if short => "Bag",
            Self::Pane(PaneTab::Inventory) => "Inventory",
            Self::Pane(PaneTab::Cart) => "Cart",
            Self::Pane(PaneTab::Storage(StorageKind::Guild)) if short => "Guild",
            Self::Pane(PaneTab::Storage(kind)) => kind.label(),
        }
    }
}

/// Tooltip on Guild Storage outside a guild.
pub const NO_GUILD_TOOLTIP: &str = "Join a guild to use guild storage";

/// The tab can be clicked: Guild Storage needs a guild.
pub fn pane_tab_enabled(tab: PaneTab, in_guild: bool) -> bool {
    in_guild || tab != PaneTab::Storage(StorageKind::Guild)
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
            TabKind::Pane(tab) => state.get(&client_state().unified_inventory()).is_tab_selected(tab),
        }
    }
}

impl<P> ClickHandler<ClientState> for TabHandler<P>
where
    P: Path<ClientState, Inventory> + Copy + 'static,
{
    fn handle_click(&self, state: &State<ClientState>, queue: &mut EventQueue<ClientState>) {
        match self.kind {
            TabKind::Category(tab) => state.update_value_with(self.inventory_path, move |inventory: &mut Inventory| {
                inventory.view_mut().set_tab(tab)
            }),
            // Storage tabs talk to the server, so the client handles them.
            TabKind::Pane(tab) => queue.queue(InputEvent::ClickPaneTab { tab }),
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
    /// Handlers shown this frame, with their cells.
    shown: Vec<usize>,
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

    /// Inventory, Cart (with a cart equipped), Storage and Guild Storage.
    pub fn panes(inventory_path: P) -> Self {
        Self {
            tabs: [
                PaneTab::Inventory,
                PaneTab::Cart,
                PaneTab::Storage(StorageKind::Personal),
                PaneTab::Storage(StorageKind::Guild),
            ]
            .into_iter()
            .map(|tab| TabHandler {
                inventory_path,
                kind: TabKind::Pane(tab),
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
        let pane_tabs = state.get(&client_state().unified_inventory()).pane_tabs();
        let shown: Vec<usize> = self
            .tabs
            .iter()
            .enumerate()
            .filter(|(_, handler)| match handler.kind {
                TabKind::Category(_) => true,
                TabKind::Pane(tab) => pane_tabs.contains(&tab),
            })
            .map(|(index, _)| index)
            .collect();
        with_single_resolver(resolvers, |resolver| {
            let row = resolver.with_height(tab_height + TAB_UNDERLINE);
            let cells = row_cells(Area { height: tab_height, ..row }, shown.len(), 2.0);
            TabRowLayoutInfo {
                row,
                tab_height,
                shown,
                cells,
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
        let pending_storage = state.get(&client_state().unified_inventory()).pending_storage();
        let in_guild = state.get(&client_state().guild()).joined;
        let corners = CornerDiameter {
            top_left: 6.0,
            top_right: 6.0,
            bottom_left: 0.0,
            bottom_right: 0.0,
        };

        for (handler, cell) in layout_info.shown.iter().map(|index| &self.tabs[*index]).zip(&layout_info.cells) {
            let enabled = match handler.kind {
                TabKind::Pane(tab) => pane_tab_enabled(tab, in_guild),
                TabKind::Category(_) => true,
            };
            // Asked for, waiting on the server: drawn half-selected.
            let pending = matches!(handler.kind, TabKind::Pane(PaneTab::Storage(kind)) if pending_storage == Some(kind));
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
            let outline = if selected || pending { accent_mid } else { accent_dim };
            let area = tab_outline(*cell, layout_info.tab_height, selected);
            add_outlined(layout, area, outline, fill, corners);
            let text_color = match (enabled, selected || pending, hovered) {
                (false, ..) => muted,
                (true, true, _) | (true, false, true) => text,
                (true, false, false) => muted,
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
                    (false, _) if !enabled => {
                        struct NoGuildTooltip;
                        layout.add_tooltip(NO_GUILD_TOOLTIP, NoGuildTooltip.tooltip_id());
                    }
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
    title: fn(&ClientState) -> &'static str,
    inventory_path: P,
    fallback_capacity: Option<u32>,
    last: Cell<Option<(&'static str, usize, Option<u32>)>>,
    text: UnsafeCell<String>,
}

impl<P> PaneLabel<P> {
    /// `title` picks the name each frame ("Storage" or "Guild Storage").
    pub fn new(title: fn(&ClientState) -> &'static str, inventory_path: P, fallback_capacity: Option<u32>) -> Self {
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
        let key = (
            (self.title)(state),
            inventory.item_count(),
            inventory.capacity().or(self.fallback_capacity),
        );
        if self.last.get() != Some(key) {
            unsafe {
                *self.text.get() = pane_label_text(key.0, key.1, key.2);
            }
            self.last.set(Some(key));
        }
        unsafe { Some(self.text.as_ref_unchecked()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guild_storage_needs_a_guild() {
        let guild = PaneTab::Storage(StorageKind::Guild);
        assert!(!pane_tab_enabled(guild, false));
        assert!(pane_tab_enabled(guild, true));
        for tab in [PaneTab::Inventory, PaneTab::Cart, PaneTab::Storage(StorageKind::Personal)] {
            assert!(pane_tab_enabled(tab, false));
        }
    }

    /// Rows the default window shows, per theme and screen (review B1).
    #[test]
    fn the_default_window_shows_item_rows_in_every_view() {
        use crate::interface::windows::placement::{first_open_hud, inventory_window_frame};
        use crate::state::theme::InterfaceTheme;
        use crate::state::unified_inventory::{FitStep, SPLIT_MINIMUM_ROWS, SecondPane, SplitFit};

        for (name, theme) in [
            ("turoran_dark", InterfaceTheme::turoran_dark()),
            ("verdant_rune", InterfaceTheme::verdant_rune()),
            ("turoran_dark x1.25", InterfaceTheme::turoran_dark().with_font_scale(1.25)),
        ] {
            let metrics = InventoryLayoutMetrics::from_theme(&theme);
            for (screen_width, screen_height) in [(1280.0_f32, 720.0_f32), (1920.0, 1080.0), (1024.0, 768.0)] {
                let frame = inventory_window_frame(
                    first_open_hud(screen_width, screen_height).inventory,
                    screen_width,
                    screen_height,
                );
                let (width, height) = (
                    frame.width.max(crate::interface::windows::resizable::GRID_WINDOW_MINIMUM_WIDTH),
                    frame.height.max(240.0),
                );
                let single = metrics.visible_rows(width, height, false, false);
                assert!(
                    single >= 5,
                    "{name} {screen_width}x{screen_height}: single {single} at {width}x{height}"
                );

                // The split grows the window (screen permitting) to four rows a half.
                let room = (screen_height - 16.0).min(720.0);
                for (label, pane, storage) in [
                    ("cart split", SecondPane::Cart, false),
                    ("storage split", SecondPane::Storage, true),
                    ("cart pane with storage open", SecondPane::Cart, true),
                ] {
                    let needed = metrics.height_for_rows(SPLIT_MINIMUM_ROWS, width, true, storage);
                    let mut fit = SplitFit::default();
                    let grown = match fit.step(Some(pane), height, needed, room) {
                        FitStep::Resize(grown) => grown,
                        FitStep::Nothing => height,
                    };
                    let rows = metrics.visible_rows(width, grown, true, storage);
                    assert!(
                        rows >= SPLIT_MINIMUM_ROWS,
                        "{name} {screen_width}x{screen_height} {label}: {rows} rows at {width}x{grown}"
                    );
                }
            }
        }
    }

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
        assert_eq!(TabKind::Pane(PaneTab::Cart).label(40.0), "Cart");
        assert_eq!(TabKind::Pane(PaneTab::Inventory).label(80.0), "Inventory");
        assert_eq!(TabKind::Pane(PaneTab::Storage(StorageKind::Personal)).label(80.0), "Storage");
        assert_eq!(TabKind::Pane(PaneTab::Storage(StorageKind::Guild)).label(80.0), "Guild Storage");
        assert_eq!(TabKind::Pane(PaneTab::Storage(StorageKind::Guild)).label(40.0), "Guild");
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
