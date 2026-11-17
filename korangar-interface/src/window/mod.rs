mod anchor;
pub mod resize;
pub mod store;

use std::marker::PhantomData;

pub use anchor::{Anchor, AnchorPoint};
pub use interface_macros::StateWindow;
pub use resize::{FrameTarget, ResizeLimits, frame_hit, frame_targets, resize_handle_areas, resize_handle_at, resize_window_area};
use rust_state::{Path, RustState, Selector, State};
use store::WindowStore;

use crate::MouseMode;
use crate::application::{Application, CornerDiameter, Position, ShadowPadding, Size};
use crate::element::Element;
use crate::element::id::ElementIdGenerator;
use crate::element::store::ElementStore;
use crate::event::{ClickHandler, Event};
use crate::layout::alignment::{HorizontalAlignment, VerticalAlignment};
use crate::layout::area::Area;
use crate::layout::{MouseButton, ResizeMode, Resolver, WindowLayout};
use crate::prelude::EventQueue;
use crate::theme::{ThemePathGetter, theme};

mod private {
    /// Sealed trait to avoid outside implementations of
    /// [`Window`](super::Window).
    pub trait Sealed {}
}

/// This trait is only used to abstract over [`WindowInternal`] and it's
/// generics and is not supposed to be implemented outised this crate.
pub trait Window<App: Application>: private::Sealed {
    /// Get the window class of the window (if any).
    fn get_class(&self) -> Option<App::WindowClass>;

    /// Get the window theme.
    fn get_theme_type(&self) -> App::ThemeType;

    /// Returns if the window is closable or not.
    fn is_closable(&self, state: &State<App>) -> bool;

    /// Create the layout info for the window.
    #[allow(private_interfaces)]
    fn create_layout_info(
        &mut self,
        state: &State<App>,
        store: &mut WindowStore,
        data: &mut WindowData<App>,
        generator: &mut ElementIdGenerator,
        text_layouter: &App::TextLayouter,
        window_size: App::Size,
    ) -> DisplayInformation;

    /// Lay out the window.
    #[allow(private_interfaces)]
    fn lay_out<'a>(&'a self, state: &'a State<App>, store: &'a WindowStore, data: &'a WindowData<App>, layout: &mut WindowLayout<'a, App>);
}

/// An application specific custom window.
///
/// Can be passed to [`open_window`](crate::Interface::open_window).
pub trait CustomWindow<App>
where
    App: Application,
{
    /// Return the window class of the window (if any).
    fn window_class() -> Option<App::WindowClass> {
        None
    }

    /// Convert to a real window.
    fn to_window<'a>(self) -> impl Window<App> + 'a;
}

/// A window for inspecting a part of the application state.
///
/// Can be passed to [`open_state_window`](crate::Interface::open_state_window).
///
/// Implementing this trait is generally discouraged, see [`CustomWindow`] for
/// custom windows.
pub trait StateWindow<App>
where
    App: Application,
{
    /// Return the window class of the window (if any).
    fn window_class() -> Option<App::WindowClass> {
        None
    }

    /// Create a new immutable inspector window of the state.
    fn to_window<'a>(self_path: impl Path<App, Self>) -> impl Window<App> + 'a;

    /// Create a new mutable inspector window of the state.
    fn to_window_mut<'a>(self_path: impl Path<App, Self>) -> impl Window<App> + 'a;
}

#[derive(RustState)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct WindowTheme<App>
where
    App: Application,
{
    pub title_color: App::Color,
    pub hovered_title_color: App::Color,
    pub background_color: App::Color,
    pub highlight_color: App::Color,
    pub shadow_color: App::Color,
    pub shadow_padding: App::ShadowPadding,
    pub gaps: f32,
    pub border: f32,
    pub corner_diameter: App::CornerDiameter,
    pub close_button_size: App::Size,
    pub close_button_corner_diameter: App::CornerDiameter,
    pub minimum_width: f32,
    pub maximum_width: f32,
    pub minimum_height: f32,
    pub maximum_height: f32,
    pub title_height: f32,
    pub title_gap: f32,
    pub font_size: App::FontSize,
    pub horizontal_alignment: HorizontalAlignment,
    pub vertical_alignment: VerticalAlignment,
    pub overflow_behavior: App::OverflowBehavior,
    pub anchor_color: App::Color,
    pub closest_anchor_color: App::Color,
    /// Outline drawn just inside the window edge. Themes set this from
    /// `border`.
    pub frame_color: App::Color,
    /// Thickness of [`Self::frame_color`] in interface pixels. Zero skips the
    /// outline.
    pub frame_width: f32,
}

pub struct WindowData<App>
where
    App: Application,
{
    pub id: u64,
    pub anchor: Anchor<App>,
    pub size: App::Size,
}

pub(crate) struct DisplayInformation {
    pub real_area: Area,
    pub display_height: f32,
    /// Size limits resolved during the last layout, used by resize drags.
    pub limits: ResizeLimits,
    /// The window lets the user change its height.
    pub vertical_resize: bool,
}

pub struct WindowLayoutInfoSet<T> {
    area: Area,
    title_area: Area,
    children: T,
}

struct ResizeClickHandler {
    window_id: u64,
    resize_mode: ResizeMode,
}

impl ResizeClickHandler {
    fn new(resize_mode: ResizeMode) -> Self {
        Self { window_id: 0, resize_mode }
    }

    fn update(&mut self, window_id: u64) {
        self.window_id = window_id;
    }
}

impl<App: Application> ClickHandler<App> for ResizeClickHandler {
    fn handle_click(&self, _: &State<App>, queue: &mut EventQueue<App>) {
        let Self { window_id, resize_mode } = *self;

        queue.queue(Event::SetMouseMode {
            mouse_mode: MouseMode::ResizingWindow { resize_mode, window_id },
        });
    }
}

#[derive(Default)]
struct MoveClickHandler {
    window_id: u64,
}

impl MoveClickHandler {
    fn update(&mut self, window_id: u64) {
        self.window_id = window_id;
    }
}

impl<App: Application> ClickHandler<App> for MoveClickHandler {
    fn handle_click(&self, _: &State<App>, queue: &mut EventQueue<App>) {
        queue.queue(Event::SetMouseMode {
            mouse_mode: MouseMode::MovingWindow { window_id: self.window_id },
        });
    }
}

#[derive(Default)]
struct CloseClickHandler {
    window_id: u64,
}

impl CloseClickHandler {
    fn update(&mut self, window_id: u64) {
        self.window_id = window_id;
    }
}

impl<App: Application> ClickHandler<App> for CloseClickHandler {
    fn handle_click(&self, _: &State<App>, queue: &mut EventQueue<App>) {
        queue.queue(Event::CloseWindow { window_id: self.window_id });
    }
}

pub struct WindowInternal<App, Title, A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, Elements>
where
    App: Application,
    Elements: Element<App>,
{
    title_marker: PhantomData<Title>,
    title: A,
    title_color: B,
    hovered_title_color: C,
    background_color: D,
    highlight_color: E,
    shadow_color: F,
    shadow_padding: G,
    title_height: H,
    title_gap: I,
    font_size: J,
    gaps: K,
    border: L,
    corner_diameter: M,
    closable: N,
    resizable: O,
    close_button_size: P,
    close_button_corner_diameter: Q,
    minimum_width: R,
    maximum_width: S,
    minimum_height: T,
    maximum_height: U,
    theme: App::ThemeType,
    class: Option<App::WindowClass>,
    elements: Elements,
    // HACK: This is a bit ugly since all of these store the window_id. Ideally, we would be able
    // to inject some data into the click action instead.
    close_click_action: CloseClickHandler,
    move_click_action: MoveClickHandler,
    /// One handler per corner and edge, in [`resize_handle_areas`] order.
    resize_click_actions: [ResizeClickHandler; 8],
    layout_info: Option<WindowLayoutInfoSet<<Elements as Element<App>>::LayoutInfo>>,
}

impl<App, Title, A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, Elements>
    WindowInternal<App, Title, A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, Elements>
where
    App: Application,
    Elements: Element<App>,
{
    #[allow(clippy::too_many_arguments)]
    pub fn component_new(
        title: A,
        title_color: B,
        hovered_title_color: C,
        background_color: D,
        highlight_color: E,
        shadow_color: F,
        shadow_padding: G,
        title_height: H,
        title_gap: I,
        font_size: J,
        gaps: K,
        border: L,
        corner_diameter: M,
        closable: N,
        resizable: O,
        close_button_size: P,
        close_button_corner_diameter: Q,
        minimum_width: R,
        maximum_width: S,
        minimum_height: T,
        maximum_height: U,
        theme: App::ThemeType,
        class: Option<App::WindowClass>,
        elements: Elements,
    ) -> Self {
        Self {
            title_marker: PhantomData,
            title,
            title_color,
            hovered_title_color,
            background_color,
            highlight_color,
            shadow_color,
            shadow_padding,
            title_height,
            title_gap,
            font_size,
            gaps,
            border,
            corner_diameter,
            closable,
            resizable,
            close_button_size,
            close_button_corner_diameter,
            minimum_width,
            maximum_width,
            minimum_height,
            maximum_height,
            theme,
            class,
            elements,
            close_click_action: CloseClickHandler::default(),
            move_click_action: MoveClickHandler::default(),
            resize_click_actions: [
                ResizeMode::TOP_LEFT,
                ResizeMode::TOP_RIGHT,
                ResizeMode::BOTTOM_LEFT,
                ResizeMode::BOTTOM_RIGHT,
                ResizeMode::LEFT,
                ResizeMode::RIGHT,
                ResizeMode::TOP,
                ResizeMode::BOTTOM,
            ]
            .map(ResizeClickHandler::new),
            layout_info: None,
        }
    }
}

impl<App, Title, A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, Elements> private::Sealed
    for WindowInternal<App, Title, A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, Elements>
where
    App: Application,
    Elements: Element<App>,
{
}

impl<App, Title, A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, Elements> Window<App>
    for WindowInternal<App, Title, A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, Elements>
where
    App: Application,
    Title: AsRef<str>,
    A: Selector<App, Title>,
    B: Selector<App, App::Color>,
    C: Selector<App, App::Color>,
    D: Selector<App, App::Color>,
    E: Selector<App, App::Color>,
    F: Selector<App, App::Color>,
    G: Selector<App, App::ShadowPadding>,
    H: Selector<App, f32>,
    I: Selector<App, f32>,
    J: Selector<App, App::FontSize>,
    K: Selector<App, f32>,
    L: Selector<App, f32>,
    M: Selector<App, App::CornerDiameter>,
    N: Selector<App, bool>,
    O: Selector<App, bool>,
    P: Selector<App, App::Size>,
    Q: Selector<App, App::CornerDiameter>,
    R: Selector<App, f32>,
    S: Selector<App, f32>,
    T: Selector<App, f32>,
    U: Selector<App, f32>,
    Elements: Element<App>,
    <Elements as Element<App>>::LayoutInfo: 'static,
{
    fn get_class(&self) -> Option<App::WindowClass> {
        self.class
    }

    fn get_theme_type(&self) -> <App as Application>::ThemeType {
        self.theme
    }

    fn is_closable(&self, state: &State<App>) -> bool {
        *state.get(&self.closable)
    }

    #[allow(private_interfaces)]
    fn create_layout_info(
        &mut self,
        state: &State<App>,
        store: &mut WindowStore,
        data: &mut WindowData<App>,
        generator: &mut ElementIdGenerator,
        text_layouter: &App::TextLayouter,
        window_size: App::Size,
    ) -> DisplayInformation {
        self.close_click_action.update(data.id);
        self.move_click_action.update(data.id);
        self.resize_click_actions.iter_mut().for_each(|action| action.update(data.id));

        App::set_current_theme_type(self.theme);

        let title_height = *state.get(&self.title_height);
        let title_gap = *state.get(&self.title_gap);

        let limits = ResizeLimits::new(
            *state.get(&self.minimum_width),
            *state.get(&self.maximum_width),
            *state.get(&self.minimum_height),
            *state.get(&self.maximum_height),
        );
        let minimum_height = limits.minimum_height;
        let maximum_height = limits.maximum_height;

        let adjusted_size = App::Size::new(limits.clamp_width(data.size.width()), limits.clamp_height(data.size.height()));

        if data.anchor.is_initializing() {
            data.anchor.initialize(window_size, adjusted_size);
        }

        // Keep the whole window inside the viewport. The real height is only
        // known after layout, so a window whose content runs past the bottom
        // edge is laid out once more, moved up by the overflow.
        let anchor_position = data.anchor.to_position(window_size);
        let mut window_height = title_height;
        let mut laid_out = None;
        for _ in 0..2 {
            let (left, top) = clamp_window_origin(
                anchor_position.left(),
                anchor_position.top(),
                adjusted_size.width(),
                window_height,
                window_size.width(),
                window_size.height(),
            );

            let available_area = Area {
                left,
                top,
                width: adjusted_size.width(),
                height: adjusted_size.height(),
            };

            let element_store = store.get_or_create_from_window_id(data.id, generator);
            let mut resolver = Resolver::new(available_area, 0.0, text_layouter);

            let title_area = resolver.with_height(title_height);

            let (area, children) =
                resolver.with_derived_borderless(*state.get(&self.gaps), *state.get(&self.border), title_gap, |resolver| {
                    self.elements.create_layout_info(state, element_store, resolver)
                });

            let area = Area {
                left: title_area.left,
                top: title_area.top,
                width: title_area.width,
                height: (title_area.height + area.height).min(maximum_height).max(minimum_height),
            };

            let overflows = area.top > 0.5 && area.top + area.height > window_size.height() + 0.5;
            let settled = !overflows || area.height <= window_height + 0.5;
            window_height = area.height;
            laid_out = Some((title_area, area, children));
            if settled {
                break;
            }
        }
        let (title_area, area, children) = laid_out.expect("window layout ran");

        self.layout_info = Some(WindowLayoutInfoSet {
            area,
            title_area,
            children,
        });

        let display_height = area.height;

        DisplayInformation {
            real_area: area,
            display_height,
            limits,
            vertical_resize: *state.get(&self.resizable),
        }
    }

    #[cfg_attr(feature = "debug", korangar_debug::profile)]
    #[allow(private_interfaces)]
    fn lay_out<'a>(&'a self, state: &'a State<App>, store: &'a WindowStore, data: &'a WindowData<App>, layout: &mut WindowLayout<'a, App>) {
        let store = store.get_from_window_id(data.id);
        let layout_info = self.layout_info.as_ref().expect("no layout present");

        App::set_current_theme_type(self.theme);

        if layout_info.area.check().dont_mark().run(layout) {
            layout.set_hovered();
        }

        // Frame targets in priority order (close button, resize handles, title
        // bar), checked before the contents. The first hovered one takes the
        // click; see `resize::frame_targets`.
        let border = *state.get(&self.border);
        let horizontal_resize_available = *state.get(&self.minimum_width) < *state.get(&self.maximum_width);
        let vertical_resize_available = *state.get(&self.resizable);
        let close_button_area = state.get(&self.closable).then(|| {
            let close_button_size = *state.get(&self.close_button_size);
            resize::close_button_area(
                layout_info.title_area,
                close_button_size.width(),
                close_button_size.height(),
                border,
            )
        });
        let targets = resize::frame_targets(
            layout_info.area,
            layout_info.title_area,
            close_button_area,
            border,
            horizontal_resize_available,
            vertical_resize_available,
        );
        let hit = targets
            .iter()
            .find(|(_, target_area)| target_area.check().run(layout))
            .map(|(target, target_area)| (*target, *target_area));

        let resizing_mode = match layout.get_mouse_mode() {
            MouseMode::ResizingWindow { resize_mode, window_id } if *window_id == data.id => Some(*resize_mode),
            _ => None,
        };
        let mut highlighted_handle = None;
        let mut is_title_hovered = false;
        let mut is_close_hovered = false;
        match hit {
            Some((resize::FrameTarget::Close, _)) => {
                layout.register_click_handler(MouseButton::Left, &self.close_click_action);
                is_close_hovered = true;
            }
            Some((resize::FrameTarget::Resize { index, .. }, handle_area)) => {
                layout.register_click_handler(MouseButton::Left, &self.resize_click_actions[index]);
                layout.set_hovered();
                layout.set_resize_hovered();
                highlighted_handle = Some(handle_area);
            }
            Some((resize::FrameTarget::Move, _)) => {
                layout.register_click_handler(MouseButton::Left, &self.move_click_action);
                is_title_hovered = true;
            }
            None => {}
        }
        if let Some(resizing_mode) = resizing_mode {
            layout.set_resize_hovered();
            if highlighted_handle.is_none() {
                highlighted_handle = resize_handle_areas(layout_info.area, border)
                    .into_iter()
                    .find(|(mode, _)| *mode == resizing_mode)
                    .map(|(_, handle_area)| handle_area);
            }
        }

        let close_button = close_button_area.map(|close_button_area| {
            let close_button_color = match is_close_hovered {
                true => *state.get(&self.hovered_title_color),
                false => *state.get(&self.title_color),
            };
            (close_button_area, close_button_color)
        });

        layout.with_clip(layout_info.area, |layout| {
            layout.with_layer(|layout| {
                self.elements
                    .lay_out(state, ElementStore::new(store, data.id), &layout_info.children, layout);
            });
        });

        let title_color = match is_title_hovered {
            true => *state.get(&self.hovered_title_color),
            false => *state.get(&self.title_color),
        };

        layout.add_text(
            layout_info.title_area,
            state.get(&self.title).as_ref(),
            *state.get(&self.font_size),
            title_color,
            *state.get(&self.highlight_color),
            // TODO: Make this configurable in the window.
            *state.get(&theme().window().horizontal_alignment()),
            *state.get(&theme().window().vertical_alignment()),
            *state.get(&theme().window().overflow_behavior()),
        );

        let frame_width = *state.get(&theme().window().frame_width());
        if frame_width > 0.0 {
            layout.add_rectangle(
                layout_info.area,
                *state.get(&self.corner_diameter),
                *state.get(&theme().window().frame_color()),
                *state.get(&self.shadow_color),
                *state.get(&self.shadow_padding),
            );
            layout.add_rectangle(
                inset_area(layout_info.area, frame_width),
                *state.get(&self.corner_diameter),
                *state.get(&self.background_color),
                *state.get(&self.shadow_color),
                App::ShadowPadding::none(),
            );
        } else {
            layout.add_rectangle(
                layout_info.area,
                *state.get(&self.corner_diameter),
                *state.get(&self.background_color),
                *state.get(&self.shadow_color),
                *state.get(&self.shadow_padding),
            );
        }

        if let Some(handle_area) = highlighted_handle {
            layout.add_rectangle(
                handle_area,
                App::CornerDiameter::new(4.0, 4.0, 4.0, 4.0),
                *state.get(&theme().window().closest_anchor_color()),
                // TODO: Properly theme
                *state.get(&theme().window().closest_anchor_color()),
                App::ShadowPadding::none(),
            );
        }

        if let Some((close_button_area, close_button_color)) = close_button {
            layout.add_rectangle(
                close_button_area,
                *state.get(&self.close_button_corner_diameter),
                close_button_color,
                // TODO: Properly theme
                close_button_color,
                App::ShadowPadding::none(),
            );

            // TODO: Use own values
            layout.add_text(
                close_button_area,
                "X",
                *state.get(&self.font_size),
                *state.get(&self.background_color),
                *state.get(&self.highlight_color),
                HorizontalAlignment::Center { offset: 0.0, border: 0.0 },
                VerticalAlignment::Center { offset: 0.0 },
                // TODO: This shouldn't matter at all.
                *state.get(&theme().window().overflow_behavior()),
            );
        }
    }
}

/// Inner fill after a frame of `inset` interface pixels is drawn on `area`.
pub fn inset_area(area: Area, inset: f32) -> Area {
    let inset = if inset.is_finite() { inset.max(0.0) } else { 0.0 };
    Area {
        left: area.left + inset,
        top: area.top + inset,
        width: (area.width - inset * 2.0).max(0.0),
        height: (area.height - inset * 2.0).max(0.0),
    }
}

/// Top-left corner for a window of `width` by `height` so it stays inside a
/// `screen_width` by `screen_height` viewport. A window larger than the
/// viewport is pinned to the top-left edge.
pub(crate) fn clamp_window_origin(left: f32, top: f32, width: f32, height: f32, screen_width: f32, screen_height: f32) -> (f32, f32) {
    let max_left = (screen_width - width).max(0.0);
    let max_top = (screen_height - height).max(0.0);
    (left.clamp(0.0, max_left), top.clamp(0.0, max_top))
}

#[cfg(test)]
mod tests {
    use super::{Area, clamp_window_origin, inset_area};

    #[test]
    fn frame_inset_leaves_a_border_inside_the_window() {
        let outer = Area {
            left: 10.0,
            top: 20.0,
            width: 100.0,
            height: 80.0,
        };
        let inner = inset_area(outer, 2.0);
        assert_eq!(inner.left, 12.0);
        assert_eq!(inner.top, 22.0);
        assert_eq!(inner.width, 96.0);
        assert_eq!(inner.height, 76.0);
        assert_eq!(inset_area(outer, 0.0).width, outer.width);
    }

    #[test]
    fn window_inside_the_viewport_does_not_move() {
        assert_eq!(clamp_window_origin(100.0, 200.0, 300.0, 150.0, 1280.0, 720.0), (100.0, 200.0));
    }

    #[test]
    fn window_hanging_off_the_bottom_moves_up() {
        // A 140px hotbar placed for a 96px frame on a 700px window.
        let (_, top) = clamp_window_origin(430.0, 596.0, 420.0, 140.0, 1279.0, 700.0);
        assert_eq!(top, 560.0);
    }

    #[test]
    fn window_hanging_off_the_sides_moves_in() {
        assert_eq!(clamp_window_origin(-200.0, 10.0, 400.0, 100.0, 1024.0, 560.0).0, 0.0);
        assert_eq!(clamp_window_origin(900.0, 10.0, 400.0, 100.0, 1024.0, 560.0).0, 624.0);
    }

    #[test]
    fn window_larger_than_the_viewport_is_pinned_top_left() {
        assert_eq!(clamp_window_origin(50.0, 50.0, 1400.0, 900.0, 1024.0, 560.0), (0.0, 0.0));
    }
}
