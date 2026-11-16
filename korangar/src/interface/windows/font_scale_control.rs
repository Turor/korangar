//! Settings → Interface font-scale slider and percent text box.
//!
//! There is no slider widget. The track records its geometry while laying out
//! (that runs before the click on the same frame). A click switches the mouse
//! to [`MouseInputMode::AdjustFontScale`]; later frames keep writing the
//! percent from the pointer until the button is released.

use std::cell::Cell;

use korangar_interface::MouseMode;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{BaseLayoutInfo, Element};
use korangar_interface::event::{ClickHandler, Event, EventQueue, InputHandler};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{MouseButton, Resolvers, WindowLayout, with_single_resolver};
use rust_state::{Path, State};

use crate::graphics::{CornerDiameter, ShadowPadding};
use crate::input::MouseInputMode;
use crate::interface::font_scale::{self, FontScaleSetting};
use crate::state::theme::{InterfaceThemePathExt, ThemeTokensPathExt};
use crate::state::{ClientState, client_theme};

const SLIDER_HEIGHT: f32 = 22.0;
const TRACK_HEIGHT: f32 = 6.0;
const THUMB_WIDTH: f32 = 10.0;
const THUMB_HEIGHT: f32 = 16.0;

#[derive(Clone, Copy)]
struct TrackSample {
    left: f32,
    width: f32,
    mouse_x: f32,
}

thread_local! {
    static TRACK: Cell<TrackSample> = const { Cell::new(TrackSample { left: 0.0, width: 1.0, mouse_x: 0.0 }) };
}

fn slider_t(left: f32, width: f32, mouse_x: f32) -> f32 {
    if width <= f32::EPSILON {
        return 0.0;
    }
    ((mouse_x - left) / width).clamp(0.0, 1.0)
}

fn write_slider(setting: &mut FontScaleSetting, t: f32) {
    let (percent, draft) = font_scale::apply_slider_font_scale(t);
    setting.percent = percent;
    setting.draft = draft;
}

pub struct FontScaleSlider<P> {
    setting: P,
}

impl<P> FontScaleSlider<P> {
    pub fn new(setting: P) -> Self {
        Self { setting }
    }
}

impl<P> ClickHandler<ClientState> for FontScaleSlider<P>
where
    P: Path<ClientState, FontScaleSetting>,
{
    fn handle_click(&self, state: &State<ClientState>, queue: &mut EventQueue<ClientState>) {
        let sample = TRACK.with(|cell| cell.get());
        let t = slider_t(sample.left, sample.width, sample.mouse_x);
        state.update_value_with(self.setting, move |setting| write_slider(setting, t));
        queue.queue(Event::SetMouseMode {
            mouse_mode: MouseMode::Custom {
                mode: MouseInputMode::AdjustFontScale,
            },
        });
    }
}

impl<P> Element<ClientState> for FontScaleSlider<P>
where
    P: Path<ClientState, FontScaleSetting>,
{
    type LayoutInfo = BaseLayoutInfo;

    fn create_layout_info(
        &mut self,
        _: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            let area = resolver.with_height(SLIDER_HEIGHT);
            BaseLayoutInfo { area }
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let area = layout_info.area;
        let mouse_x = layout.get_mouse_position().left;
        TRACK.with(|cell| {
            cell.set(TrackSample {
                left: area.left,
                width: area.width.max(1.0),
                mouse_x,
            });
        });

        let dragging = matches!(layout.get_mouse_mode(), MouseMode::Custom {
            mode: MouseInputMode::AdjustFontScale,
        });
        if dragging {
            let t = slider_t(area.left, area.width, mouse_x);
            state.update_value_with(self.setting, move |setting| write_slider(setting, t));
        }

        let hovered = if dragging {
            let hovered = area.check().any_mouse_mode().run(layout);
            if hovered {
                layout.set_hovered();
            }
            hovered
        } else {
            area.check().run(layout)
        };
        if hovered {
            layout.register_click_handler(MouseButton::Left, self);
        }

        let raised = *state.get(&client_theme().tokens().raised());
        let accent = *state.get(&client_theme().tokens().accent());
        let border = *state.get(&client_theme().tokens().border());
        let percent = state.get(&self.setting).percent;
        let t = font_scale::percent_to_slider_t(percent);

        let track = Area {
            left: area.left,
            top: area.top + (area.height - TRACK_HEIGHT) / 2.0,
            width: area.width,
            height: TRACK_HEIGHT,
        };
        layout.add_rectangle(
            track,
            CornerDiameter::uniform(TRACK_HEIGHT / 2.0),
            border,
            border,
            ShadowPadding::uniform(0.0),
        );
        let inset = 1.0;
        layout.add_rectangle(
            Area {
                left: track.left + inset,
                top: track.top + inset,
                width: (track.width - inset * 2.0).max(1.0),
                height: (track.height - inset * 2.0).max(1.0),
            },
            CornerDiameter::uniform(TRACK_HEIGHT / 2.0),
            raised,
            raised,
            ShadowPadding::uniform(0.0),
        );

        let travel = (area.width - THUMB_WIDTH).max(0.0);
        let thumb = Area {
            left: area.left + t * travel,
            top: area.top + (area.height - THUMB_HEIGHT) / 2.0,
            width: THUMB_WIDTH,
            height: THUMB_HEIGHT,
        };
        layout.add_rectangle(thumb, CornerDiameter::uniform(3.0), accent, border, ShadowPadding::uniform(0.0));
    }
}

enum DraftEdit {
    Character(char),
    Backspace,
    Commit,
    Cancel,
}

fn edit_font_scale(setting: &mut FontScaleSetting, edit: DraftEdit) {
    match edit {
        DraftEdit::Character(character) => {
            if !character.is_ascii_digit() {
                return;
            }
            let canonical = font_scale::clamp_font_scale_percent(setting.percent).to_string();
            if setting.draft == canonical {
                setting.draft.clear();
            }
            if setting.draft.len() >= 3 {
                return;
            }
            setting.draft.push(character);
            let (percent, text) = font_scale::apply_typed_font_scale(setting.percent, &setting.draft);
            setting.percent = percent;
            setting.draft = text;
        }
        DraftEdit::Backspace => {
            setting.draft.pop();
            let (percent, text) = font_scale::apply_typed_font_scale(setting.percent, &setting.draft);
            setting.percent = percent;
            setting.draft = text;
        }
        DraftEdit::Commit => {
            let (percent, text) = font_scale::commit_font_scale_draft(setting.percent, &setting.draft);
            setting.percent = percent;
            setting.draft = text;
        }
        DraftEdit::Cancel => {
            let percent = font_scale::clamp_font_scale_percent(setting.percent);
            setting.percent = percent;
            setting.draft = percent.to_string();
        }
    }
}

pub struct FontScaleInput<P> {
    setting: P,
}

impl<P> FontScaleInput<P> {
    pub fn new(setting: P) -> Self {
        Self { setting }
    }
}

impl<P> InputHandler<ClientState> for FontScaleInput<P>
where
    P: Path<ClientState, FontScaleSetting>,
{
    fn handle_character(&self, state: &State<ClientState>, queue: &mut EventQueue<ClientState>, character: char) {
        if character == '\t' || character == '\r' || character == '\n' {
            state.update_value_with(self.setting, |setting| edit_font_scale(setting, DraftEdit::Commit));
            queue.queue(Event::Unfocus);
            return;
        }
        if character == '\x1b' {
            state.update_value_with(self.setting, |setting| edit_font_scale(setting, DraftEdit::Cancel));
            queue.queue(Event::Unfocus);
            return;
        }
        if character == '\x08' {
            state.update_value_with(self.setting, |setting| edit_font_scale(setting, DraftEdit::Backspace));
            return;
        }
        if character.is_ascii_digit() {
            state.update_value_with(self.setting, move |setting| {
                edit_font_scale(setting, DraftEdit::Character(character));
            });
        }
    }
}
