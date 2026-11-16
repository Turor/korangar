//! Rodex compose window: receiver, title, body, zeny and item attachments,
//! plus the amount prompt for attaching part of a stack.
//!
//! Items are attached the roBrowser way: drag a stack from the inventory
//! onto the item area (a stack above 1 asks how many). Clicking an attached
//! item takes it back off the mail.

use korangar_interface::MouseMode;
use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::element::Element;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::event::{ClickHandler, DropHandler, EventQueue};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::tooltip::TooltipExt;
use korangar_interface::layout::{MouseButton, Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{HorizontalAlignment, TextThemePathExt, VerticalAlignment};
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{Path, State};

use crate::graphics::Color;
use crate::input::{InputEvent, MouseInputMode};
use crate::interface::components::item_box::{paint_item_slot, slot_border_and_fill};
use crate::interface::font_scale::from_theme_body;
use crate::interface::resource::ItemSource;
use crate::interface::windows::WindowClass;
use crate::loaders::{FontSize, OverflowBehavior};
use crate::renderer::LayoutExt;
use crate::state::mail_attach::{MAX_ATTACHMENTS, MailAttachment, MailAttachmentsPathExt};
use crate::state::mail_compose::MailComposeStatePathExt;
use crate::state::theme::{InterfaceThemePathExt, InterfaceThemeType, ThemeTokensPathExt};
use crate::state::{ClientState, ClientStatePathExt, client_state, client_theme};

pub struct MailComposeToBox;
pub struct MailComposeTitleBox;
pub struct MailComposeBodyBox;
pub struct MailComposeZenyBox;
pub struct MailAttachAmountBox;

const ATTACHMENT_ROW: f32 = 28.0;

/// Takes the attachment in one row back off the mail.
struct DetachHandler {
    slot: usize,
}

impl ClickHandler<ClientState> for DetachHandler {
    fn handle_click(&self, state: &State<ClientState>, queue: &mut EventQueue<ClientState>) {
        let path = client_state().mail_compose().attachments().items();
        let items = state.get(&path);
        if let Some(item) = items.get(self.slot) {
            queue.queue(InputEvent::MailDetachItem { index: item.index });
        }
    }
}

/// Attached stacks with their icons. A click takes one off.
struct MailAttachmentList<P> {
    items_path: P,
    handlers: [DetachHandler; MAX_ATTACHMENTS],
}

impl<P> MailAttachmentList<P> {
    fn new(items_path: P) -> Self {
        Self {
            items_path,
            handlers: std::array::from_fn(|slot| DetachHandler { slot }),
        }
    }
}

impl<P> Element<ClientState> for MailAttachmentList<P>
where
    P: Path<ClientState, Vec<MailAttachment>>,
{
    type LayoutInfo = Area;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            let count = state.get(&self.items_path).len();
            let height = if count == 0 { 20.0 } else { count as f32 * ATTACHMENT_ROW };
            resolver.with_height(height)
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let font_size = FontSize(from_theme_body(*state.get(&client_theme().tokens().font_size()), 12.0));
        let items = state.get(&self.items_path);
        if items.is_empty() {
            layout.add_text(
                *layout_info,
                "No items attached",
                font_size,
                *state.get(&client_theme().tokens().muted()),
                *state.get(&client_theme().tokens().gold()),
                HorizontalAlignment::Left { offset: 0.0, border: 0.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
            return;
        }

        for (slot, item) in items.iter().enumerate().take(MAX_ATTACHMENTS) {
            let row = Area {
                left: layout_info.left,
                top: layout_info.top + slot as f32 * ATTACHMENT_ROW,
                width: layout_info.width,
                height: ATTACHMENT_ROW,
            };
            let icon = Area {
                left: row.left,
                top: row.top + 2.0,
                width: 24.0,
                height: 24.0,
            };
            let hovered = row.check().run(layout);
            let (border, fill) = slot_border_and_fill(state, hovered, false);
            paint_item_slot(state, layout, icon, border, fill, 4.0, true);
            if let Some(texture) = item.texture.as_ref() {
                layout.add_texture(icon, texture.clone(), Color::WHITE, false);
            }
            layout.add_text(
                Area {
                    left: row.left + 30.0,
                    top: row.top,
                    width: (row.width - 30.0).max(1.0),
                    height: row.height,
                },
                &item.name,
                font_size,
                *state.get(&client_theme().text().color()),
                *state.get(&client_theme().tokens().gold()),
                HorizontalAlignment::Left { offset: 0.0, border: 0.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
            layout.add_text(
                row,
                &item.count_text,
                font_size,
                *state.get(&client_theme().text().color()),
                *state.get(&client_theme().tokens().gold()),
                HorizontalAlignment::Right { offset: 4.0, border: 0.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
            if hovered {
                struct DetachTip;
                layout.add_tooltip("Click to take it off the mail", DetachTip.tooltip_id());
                layout.register_click_handler(MouseButton::Left, &self.handlers[slot]);
            }
        }
    }
}

/// Drop target for inventory stacks.
struct MailDropZone;

impl Element<ClientState> for MailDropZone {
    type LayoutInfo = Area;

    fn create_layout_info(
        &mut self,
        _: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| resolver.with_height(36.0))
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let moving = matches!(layout.get_mouse_mode(), MouseMode::Custom {
            mode: MouseInputMode::MoveItem {
                source: ItemSource::Inventory,
                ..
            },
        });
        let hovered = if moving {
            let hovered = layout_info.check().any_mouse_mode().run(layout);
            if hovered {
                layout.set_hovered();
            }
            hovered
        } else {
            layout_info.check().run(layout)
        };
        let (border, fill) = slot_border_and_fill(state, hovered, moving);
        paint_item_slot(state, layout, *layout_info, border, fill, 6.0, true);
        layout.add_text(
            *layout_info,
            "Drop an inventory item to attach it",
            FontSize(from_theme_body(*state.get(&client_theme().tokens().font_size()), 12.0)),
            *state.get(&client_theme().text().color()),
            *state.get(&client_theme().tokens().gold()),
            HorizontalAlignment::Center { offset: 0.0, border: 0.0 },
            VerticalAlignment::Center { offset: 0.0 },
            OverflowBehavior::Shrink,
        );
        if hovered {
            layout.register_drop_handler(&MailInventoryDrop);
        }
    }
}

struct MailInventoryDrop;

impl DropHandler<ClientState> for MailInventoryDrop {
    fn handle_drop(&self, _: &State<ClientState>, queue: &mut EventQueue<ClientState>, mouse_mode: &MouseMode<ClientState>) {
        // roBrowser only accepts stacks dragged from the inventory.
        let MouseMode::Custom {
            mode: MouseInputMode::MoveItem {
                source: ItemSource::Inventory,
                item,
            },
        } = mouse_mode
        else {
            return;
        };
        queue.queue(InputEvent::MailAttachItem {
            index: item.index.0,
            amount: None,
        });
    }
}

pub struct MailComposeWindow;

impl CustomWindow<ClientState> for MailComposeWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::MailCompose)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let compose = client_state().mail_compose();
        let ignore = |_state: &State<ClientState>, _queue: &mut EventQueue<ClientState>| {};

        window! {
            title: "Write mail",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            minimum_width: 320.0,
            elements: (
                text! {
                    text: compose.status(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! {
                    text: "To",
                },
                text_box! {
                    ghost_text: "character name",
                    state: compose.to(),
                    input_handler: DefaultHandler::<_, _, 23>::new(compose.to(), ignore),
                    focus_id: MailComposeToBox,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! {
                    text: "Title",
                },
                text_box! {
                    ghost_text: "No subject",
                    state: compose.title(),
                    input_handler: DefaultHandler::<_, _, 39>::new(compose.title(), ignore),
                    focus_id: MailComposeTitleBox,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! {
                    text: "Message",
                },
                text_box! {
                    ghost_text: "message",
                    state: compose.body(),
                    input_handler: DefaultHandler::<_, _, 499>::new(compose.body(), ignore),
                    focus_id: MailComposeBodyBox,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! {
                    text: compose.attachments().summary(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                MailAttachmentList::new(compose.attachments().items()),
                MailDropZone,
                text! {
                    text: "Zeny",
                },
                text_box! {
                    ghost_text: "0",
                    state: compose.zeny_text(),
                    input_handler: DefaultHandler::<_, _, 10>::new(compose.zeny_text(), ignore),
                    focus_id: MailComposeZenyBox,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! {
                    text: compose.attachments().fee_text(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        button! {
                            text: "Cancel",
                            event: InputEvent::CancelMailCompose,
                        },
                        button! {
                            text: "Send",
                            event: InputEvent::SendMail,
                        },
                    ),
                },
            ),
        }
    }
}

/// roBrowser's InputBox for a stack above 1: how many to attach.
pub struct MailAttachAmountWindow;

impl CustomWindow<ClientState> for MailAttachAmountWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::MailAttachAmount)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let attachments = client_state().mail_compose().attachments();
        let confirm = |_state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
            queue.queue(InputEvent::ConfirmMailAttachAmount);
        };

        window! {
            title: "Attach item",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            elements: (
                text! {
                    text: attachments.amount_prompt(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text_box! {
                    ghost_text: "amount",
                    state: attachments.amount_text(),
                    input_handler: DefaultHandler::<_, _, 5>::new(attachments.amount_text(), confirm),
                    focus_id: MailAttachAmountBox,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        button! {
                            text: "Cancel",
                            event: InputEvent::CancelMailAttachAmount,
                        },
                        button! {
                            text: "OK",
                            event: InputEvent::ConfirmMailAttachAmount,
                        },
                    ),
                },
            ),
        }
    }
}
