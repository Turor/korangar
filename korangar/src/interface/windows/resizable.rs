//! Resizable windows (Matthew t272u).
//!
//! Every window can be resized from all four edges and corners (see
//! `korangar_interface::window::resize`), sizes and positions are saved in
//! `client/window_cache.ron`, and Settings > Interface > Reset window layout
//! forgets them. Item grids reflow with the width they get, see
//! [`crate::interface::components::item_grid::grid_metrics`].
//!
//! Sizes are interface units, so a saved layout follows the interface
//! scaling setting instead of being pinned to screen pixels.

/// Narrowest Inventory, Storage and Cart window. The tab row and search box
/// still fit, and the grid drops to smaller squares.
pub const GRID_WINDOW_MINIMUM_WIDTH: f32 = 240.0;
/// Widest Inventory, Storage and Cart window. The grid caps its columns and
/// slot size well before this.
pub const GRID_WINDOW_MAXIMUM_WIDTH: f32 = 1_200.0;
