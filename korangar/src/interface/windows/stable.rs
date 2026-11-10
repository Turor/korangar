//! Homunculus and cute-pet stable, modeled on roBrowser `UI/Components/Stable`.

use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{Element, ElementBox};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{HorizontalAlignment, VerticalAlignment};
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{ManuallyAssertExt, Path, PathExt, State, VecIndexExt};

use crate::graphics::{Color, CornerDiameter, ShadowPadding};
use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::loaders::{FontSize, OverflowBehavior};
use crate::renderer::LayoutExt;
use crate::state::inventory::InventoryPathExt;
pub use crate::state::stable::StableWindowState;
use crate::state::stable::{
    ItemFace, StableHomunculusRowPathExt, StableMaterialRow, StablePetRowPathExt, StableRequest, StableWindowStatePathExt, pet_face_ids,
};
use crate::state::theme::{InterfaceThemePathExt, InterfaceThemeType, ThemeTokensPathExt};
use crate::state::{ClientState, ClientStatePathExt, client_state, client_theme};

const ROW_HEIGHT: f32 = 46.0;
const LINE_HEIGHT: f32 = 18.0;
const ICON: f32 = 24.0;

pub struct StableWindow;

impl StableWindow {
    pub fn new() -> Self {
        Self
    }
}

pub fn item_faces(loader: &crate::loaders::AsyncLoader, ids: &[i32]) -> Vec<ItemFace> {
    ids.iter()
        .copied()
        .filter(|item_id| *item_id > 0)
        .map(|item_id| {
            let (name, texture) = loader.identified_item_face(item_id);
            ItemFace { item_id, name, texture }
        })
        .collect()
}

pub fn pet_face_list(loader: &crate::loaders::AsyncLoader, pets: &[ragnarok_packets::StablePetEntry]) -> Vec<ItemFace> {
    item_faces(loader, &pet_face_ids(pets))
}

fn queue_request(queue: &mut korangar_interface::event::EventQueue<ClientState>, request: StableRequest) {
    match request {
        StableRequest::Open => queue.queue(InputEvent::OpenStable),
        StableRequest::Homunculus(homunculus_id) => queue.queue(InputEvent::RequestStableHomunculus { homunculus_id }),
        StableRequest::Pet { action, id } => queue.queue(InputEvent::RequestStablePet { action, id }),
    }
}

fn paint_icon(layout: &mut WindowLayout<ClientState>, state: &State<ClientState>, area: Area, item_id: i32, selected: bool) {
    let border = if selected {
        *state.get(&client_theme().tokens().hover())
    } else {
        *state.get(&client_theme().tokens().border())
    };
    let fill = *state.get(&client_theme().tokens().raised());
    let corner = *state.get(&client_theme().tokens().corner_radius());
    layout.add_rectangle(
        area,
        CornerDiameter::uniform(corner),
        border,
        border,
        ShadowPadding::uniform(0.0),
    );
    let inset = 2.0;
    let inner = Area {
        left: area.left + inset,
        top: area.top + inset,
        width: (area.width - inset * 2.0).max(1.0),
        height: (area.height - inset * 2.0).max(1.0),
    };
    layout.add_rectangle(inner, CornerDiameter::uniform(corner), fill, fill, ShadowPadding::uniform(0.0));
    if item_id <= 0 {
        return;
    }
    let icons_path = client_state().stable_window().icons();
    let icons = state.get(&icons_path);
    let Some(texture) = icons
        .iter()
        .find(|icon| icon.item_id == item_id)
        .and_then(|icon| icon.texture.clone())
    else {
        return;
    };
    layout.add_texture(
        Area {
            left: area.left + inset,
            top: area.top + inset,
            width: (area.width - inset * 2.0).max(1.0),
            height: (area.height - inset * 2.0).max(1.0),
        },
        texture,
        Color::WHITE,
        false,
    );
}

fn lineage_mark(lineage: &str) -> &'static str {
    match lineage {
        "Lif" => "L",
        "Amistr" => "A",
        "Filir" => "F",
        "Vanilmirth" => "V",
        "Eleanor" => "E",
        _ => "H",
    }
}

fn paint_mark(layout: &mut WindowLayout<ClientState>, state: &State<ClientState>, area: Area, letter: &'static str, selected: bool) {
    paint_icon(layout, state, area, 0, selected);
    let color = *state.get(&client_theme().tokens().text());
    let font = FontSize(*state.get(&client_theme().tokens().font_size()));
    layout.add_text(
        area,
        letter,
        font,
        color,
        color,
        HorizontalAlignment::Center { offset: 0.0, border: 0.0 },
        VerticalAlignment::Center { offset: 0.0 },
        OverflowBehavior::Shrink,
    );
}

struct RowHeading<A> {
    row: A,
}

impl<A> RowHeading<A> {
    fn new(row: A) -> Self {
        Self { row }
    }
}

struct RowLayout {
    area: Area,
}

impl<A> Element<ClientState> for RowHeading<A>
where
    A: Path<ClientState, crate::state::stable::StableHomunculusRow>,
{
    type LayoutInfo = RowLayout;

    fn create_layout_info(
        &mut self,
        _: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| RowLayout {
            area: resolver.with_height(ROW_HEIGHT),
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let row = state.get(&self.row);
        let text = *state.get(&client_theme().tokens().text());
        let muted = *state.get(&client_theme().tokens().muted());
        let font = FontSize(*state.get(&client_theme().tokens().font_size()));
        let area = layout_info.area;
        let icon = Area {
            left: area.left,
            top: area.top + 4.0,
            width: ICON,
            height: ICON,
        };
        paint_mark(layout, state, icon, lineage_mark(&row.lineage), row.active);
        let text_area = Area {
            left: area.left + ICON + 8.0,
            top: area.top,
            width: (area.width - ICON - 8.0).max(1.0),
            height: area.height,
        };
        layout.add_text(
            Area { height: 20.0, ..text_area },
            &row.title,
            font,
            text,
            text,
            HorizontalAlignment::Left { offset: 0.0, border: 0.0 },
            VerticalAlignment::Center { offset: 0.0 },
            OverflowBehavior::Shrink,
        );
        layout.add_text(
            Area {
                top: text_area.top + 20.0,
                height: 18.0,
                ..text_area
            },
            &row.detail,
            font,
            muted,
            muted,
            HorizontalAlignment::Left { offset: 0.0, border: 0.0 },
            VerticalAlignment::Center { offset: 0.0 },
            OverflowBehavior::Shrink,
        );
    }
}

struct PetHeading<A> {
    row: A,
}

impl<A> PetHeading<A> {
    fn new(row: A) -> Self {
        Self { row }
    }
}

impl<A> Element<ClientState> for PetHeading<A>
where
    A: Path<ClientState, crate::state::stable::StablePetRow>,
{
    type LayoutInfo = RowLayout;

    fn create_layout_info(
        &mut self,
        _: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| RowLayout {
            area: resolver.with_height(ROW_HEIGHT),
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let row = state.get(&self.row);
        let text = *state.get(&client_theme().tokens().text());
        let muted = *state.get(&client_theme().tokens().muted());
        let font = FontSize(*state.get(&client_theme().tokens().font_size()));
        let area = layout_info.area;
        paint_icon(
            layout,
            state,
            Area {
                left: area.left,
                top: area.top + 4.0,
                width: ICON,
                height: ICON,
            },
            row.icon_item_id,
            row.selected || row.active,
        );
        let text_area = Area {
            left: area.left + ICON + 8.0,
            top: area.top,
            width: (area.width - ICON - 8.0).max(1.0),
            height: area.height,
        };
        layout.add_text(
            Area { height: 20.0, ..text_area },
            &row.title,
            font,
            text,
            text,
            HorizontalAlignment::Left { offset: 0.0, border: 0.0 },
            VerticalAlignment::Center { offset: 0.0 },
            OverflowBehavior::Shrink,
        );
        layout.add_text(
            Area {
                top: text_area.top + 20.0,
                height: 18.0,
                ..text_area
            },
            &row.detail,
            font,
            muted,
            muted,
            HorizontalAlignment::Left { offset: 0.0, border: 0.0 },
            VerticalAlignment::Center { offset: 0.0 },
            OverflowBehavior::Shrink,
        );
    }
}

struct DetailLine {
    text: String,
    tone: u8,
    item_id: i32,
}

struct DetailBlock<A> {
    state_path: A,
    lines: Vec<DetailLine>,
}

impl<A> DetailBlock<A> {
    fn new(state_path: A) -> Self {
        Self {
            state_path,
            lines: Vec::new(),
        }
    }
}

struct DetailLayout {
    area: Area,
}

impl<A> Element<ClientState> for DetailBlock<A>
where
    A: Path<ClientState, StableWindowState>,
{
    type LayoutInfo = DetailLayout;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        self.lines.clear();
        if *state.get(&self.state_path.detail_open()) {
            let push = |lines: &mut Vec<DetailLine>, label: &str, value: &str, tone: u8, item_id: i32| {
                lines.push(DetailLine {
                    text: format!("{label}  {value}"),
                    tone,
                    item_id,
                });
            };
            push(&mut self.lines, "Name", state.get(&self.state_path.detail_name()), 0, 0);
            push(&mut self.lines, "Species", state.get(&self.state_path.detail_species()), 0, 0);
            push(&mut self.lines, "Level", state.get(&self.state_path.detail_level()), 0, 0);
            push(&mut self.lines, "EXP", state.get(&self.state_path.detail_exp()), 0, 0);
            push(&mut self.lines, "ATK", state.get(&self.state_path.detail_attack()), 0, 0);
            push(&mut self.lines, "ASPD", state.get(&self.state_path.detail_aspd()), 0, 0);
            push(&mut self.lines, "Hunger", state.get(&self.state_path.detail_hunger()), 0, 0);
            push(&mut self.lines, "Intimacy", state.get(&self.state_path.detail_intimacy()), 0, 0);
            push(
                &mut self.lines,
                "Accessory",
                state.get(&self.state_path.detail_accessory()),
                0,
                *state.get(&self.state_path.accessory_item_id()),
            );
            let buff_tone = *state.get(&self.state_path.buff_tone());
            push(&mut self.lines, "Buff", state.get(&self.state_path.detail_buff()), buff_tone, 0);
            push(&mut self.lines, "Food", state.get(&self.state_path.detail_food()), 0, 0);
            push(&mut self.lines, "Status", state.get(&self.state_path.detail_status()), 0, 0);
            push(&mut self.lines, "Renamed", state.get(&self.state_path.detail_rename()), 0, 0);
            push(&mut self.lines, "Owner", state.get(&self.state_path.detail_owner()), 0, 0);
            for material in state.get(&self.state_path.materials()) {
                self.lines.push(material_line(material));
            }
        }
        let height = LINE_HEIGHT * self.lines.len().max(1) as f32;
        with_single_resolver(resolvers, |resolver| DetailLayout {
            area: resolver.with_height(height),
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let text = *state.get(&client_theme().tokens().text());
        let muted = *state.get(&client_theme().tokens().muted());
        let accent = *state.get(&client_theme().tokens().accent());
        let font = FontSize(*state.get(&client_theme().tokens().font_size()));
        for (index, line) in self.lines.iter().enumerate() {
            let top = layout_info.area.top + LINE_HEIGHT * index as f32;
            let short = line.tone == 0
                && line
                    .text
                    .rsplit_once(' ')
                    .and_then(|(_, qty)| qty.split_once('/'))
                    .and_then(|(owned, needed)| Some(owned.parse::<i32>().ok()? < needed.parse::<i32>().ok()?))
                    .unwrap_or(false);
            let color = match line.tone {
                1 => accent,
                2 => muted,
                _ if short => muted,
                _ => text,
            };
            let text_left = if line.item_id > 0 { ICON + 6.0 } else { 0.0 };
            if line.item_id > 0 {
                paint_icon(
                    layout,
                    state,
                    Area {
                        left: layout_info.area.left,
                        top,
                        width: ICON,
                        height: ICON.min(LINE_HEIGHT),
                    },
                    line.item_id,
                    false,
                );
            }
            layout.add_text(
                Area {
                    left: layout_info.area.left + text_left,
                    top,
                    width: (layout_info.area.width - text_left).max(1.0),
                    height: LINE_HEIGHT,
                },
                &line.text,
                font,
                color,
                color,
                HorizontalAlignment::Left { offset: 0.0, border: 0.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
        }
    }
}

fn material_line(material: &StableMaterialRow) -> DetailLine {
    if material.header {
        DetailLine {
            text: material.label.clone(),
            tone: 0,
            item_id: material.item_id,
        }
    } else {
        DetailLine {
            text: format!("{}  {}", material.label, material.qty),
            tone: 0,
            item_id: material.item_id,
        }
    }
}

struct Board {
    pets: bool,
    elements: Vec<ElementBox<ClientState>>,
}

impl Board {
    fn homunculi() -> Self {
        Self {
            pets: false,
            elements: Vec::new(),
        }
    }

    fn pets() -> Self {
        Self {
            pets: true,
            elements: Vec::new(),
        }
    }
}

impl Element<ClientState> for Board {
    type LayoutInfo = ();

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        mut store: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        use korangar_interface::prelude::*;

        self.elements.clear();
        let state_path = client_state().stable_window();
        if self.pets {
            self.elements.push(ErasedElement::new(text! {
                text: "Select a pet. Deposit an egg to share it, or store the one that is out.",
                overflow_behavior: OverflowBehavior::Shrink,
            }));
            if state.get(&state_path.pets()).is_empty() {
                self.elements.push(ErasedElement::new(text! {
                    text: "No pets in the account stable.",
                    overflow_behavior: OverflowBehavior::Shrink,
                }));
            }
            for index in 0..state.get(&state_path.pets()).len() {
                let row_path = state_path.pets().index(index).manually_asserted();
                let id_path = row_path.id();
                self.elements.push(ErasedElement::new(PetHeading::new(row_path)));
                self.elements.push(ErasedElement::new(button! {
                    text: "Details",
                    event: move |state: &State<ClientState>, _: &mut EventQueue<ClientState>| {
                        let pet_id = *state.get(&id_path);
                        state.update_value_with(client_state().stable_window(), move |window| window.select_pet(pet_id));
                    },
                }));
            }
            self.elements.push(ErasedElement::new(button! {
                text: "Store current pet",
                event: |_state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                    queue_request(queue, StableWindowState::default().store_pet_request());
                },
            }));
            self.elements.push(ErasedElement::new(DetailBlock::new(state_path)));
            self.elements.push(ErasedElement::new(button! {
                text: "Summon",
                disabled: ComputedSelector::new_default(|state: &ClientState| {
                    !*client_state().stable_window().can_summon().follow_safe(state)
                }),
                event: |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                    if let Some(request) = state.get(&client_state().stable_window()).summon_request() {
                        queue_request(queue, request);
                    }
                },
            }));
            self.elements.push(ErasedElement::new(button! {
                text: "Deposit egg",
                disabled: ComputedSelector::new_default(|state: &ClientState| {
                    !*client_state().stable_window().can_deposit().follow_safe(state)
                }),
                event: |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                    if let Some(request) = state.get(&client_state().stable_window()).deposit_request() {
                        queue_request(queue, request);
                    }
                },
            }));
            self.elements.push(ErasedElement::new(button! {
                text: "To egg",
                disabled: ComputedSelector::new_default(|state: &ClientState| {
                    !*client_state().stable_window().can_to_egg().follow_safe(state)
                }),
                event: |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                    if let Some(request) = state.get(&client_state().stable_window()).to_egg_request() {
                        queue_request(queue, request);
                    }
                },
            }));
            self.elements.push(ErasedElement::new(button! {
                text: "Feed",
                disabled: ComputedSelector::new_default(|state: &ClientState| {
                    !*client_state().stable_window().can_feed().follow_safe(state)
                }),
                event: |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                    if let Some(request) = state.get(&client_state().stable_window()).feed_request() {
                        queue_request(queue, request);
                    }
                },
            }));
            self.elements.push(ErasedElement::new(button! {
                text: "Evolve",
                disabled: ComputedSelector::new_default(|state: &ClientState| {
                    !*client_state().stable_window().can_evolve().follow_safe(state)
                }),
                event: |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                    if let Some(request) = state.get(&client_state().stable_window()).evolve_request() {
                        queue_request(queue, request);
                    }
                },
            }));
            self.elements.push(ErasedElement::new(button! {
                text: "Equip accessory",
                disabled: ComputedSelector::new_default(|state: &ClientState| {
                    let window = client_state().stable_window();
                    if !*window.can_equip().follow_safe(state) {
                        return true;
                    }
                    let accessory = *window.species_accessory_id().follow_safe(state);
                    !client_state().inventory().items().follow_safe(state).iter().any(|item| {
                        i32::try_from(item.item_id.0).ok() == Some(accessory)
                    })
                }),
                event: |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                    let window_path = client_state().stable_window();
                    let accessory = *state.get(&window_path.species_accessory_id());
                    let Some(stored_index) = state.get(&client_state().inventory().items()).iter().find_map(|item| {
                        (i32::try_from(item.item_id.0).ok() == Some(accessory)).then_some(i32::from(item.index.0))
                    }) else {
                        return;
                    };
                    if let Some(request) = state.get(&window_path).equip_request(stored_index) {
                        queue_request(queue, request);
                    }
                },
            }));
            self.elements.push(ErasedElement::new(button! {
                text: "Unequip accessory",
                disabled: ComputedSelector::new_default(|state: &ClientState| {
                    !*client_state().stable_window().can_unequip().follow_safe(state)
                }),
                event: |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                    if let Some(request) = state.get(&client_state().stable_window()).unequip_request() {
                        queue_request(queue, request);
                    }
                },
            }));
        } else {
            self.elements.push(ErasedElement::new(text! {
                text: "One of each lineage. Store the active homunculus, then swap another in.",
                overflow_behavior: OverflowBehavior::Shrink,
            }));
            if state.get(&state_path.homunculi()).is_empty() {
                self.elements.push(ErasedElement::new(text! {
                    text: "No homunculi stored yet.",
                    overflow_behavior: OverflowBehavior::Shrink,
                }));
            }
            for index in 0..state.get(&state_path.homunculi()).len() {
                let row_path = state_path.homunculi().index(index).manually_asserted();
                let id_path = row_path.id();
                let active_path = row_path.active();
                self.elements.push(ErasedElement::new(RowHeading::new(row_path)));
                self.elements.push(ErasedElement::new(button! {
                    text: "Swap in",
                    disabled: ComputedSelector::new_default(move |state: &ClientState| *active_path.follow_safe(state)),
                    event: move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                        let id = *state.get(&id_path);
                        if let Some(request) = state.get(&client_state().stable_window()).swap_homunculus_request(id) {
                            queue_request(queue, request);
                        }
                    },
                }));
            }
            self.elements.push(ErasedElement::new(button! {
                text: "Store current",
                event: |_state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                    queue_request(queue, StableWindowState::default().store_homunculus_request());
                },
            }));
        }

        with_single_resolver(resolvers, |resolver| {
            self.elements.iter_mut().enumerate().for_each(|(index, element)| {
                element.create_layout_info(state, store.child_store(index as u64), resolver);
            });
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        store: ElementStore<'a>,
        _: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        self.elements
            .iter()
            .enumerate()
            .for_each(|(index, element)| element.lay_out(state, store.child_store(index as u64), &(), layout));
    }
}

impl CustomWindow<ClientState> for StableWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Stable)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        window! {
            title: "Homunculus & Pet Stable",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            resizable: true,
            minimum_width: 380.0,
            maximum_width: 680.0,
            minimum_height: 320.0,
            maximum_height: 720.0,
            elements: (
                text! {
                    text: client_state().stable_window().status(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        button! {
                            text: "Homunculus",
                            disabled: ComputedSelector::new_default(|state: &ClientState| {
                                !*client_state().stable_window().pets_tab().follow_safe(state)
                            }),
                            event: |state: &State<ClientState>, _: &mut EventQueue<ClientState>| {
                                state.update_value(client_state().stable_window().pets_tab(), false);
                            },
                        },
                        button! {
                            text: "Cute Pets",
                            disabled: ComputedSelector::new_default(|state: &ClientState| {
                                *client_state().stable_window().pets_tab().follow_safe(state)
                            }),
                            event: |state: &State<ClientState>, _: &mut EventQueue<ClientState>| {
                                state.update_value(client_state().stable_window().pets_tab(), true);
                            },
                        },
                        button! {
                            text: "Refresh",
                            event: |_state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                                queue_request(queue, StableRequest::Open);
                            },
                        },
                    ),
                },
                scroll_view! {
                    children: either! {
                        selector: client_state().stable_window().pets_tab(),
                        on_true: Board::pets(),
                        on_false: Board::homunculi(),
                    },
                },
            ),
        }
    }
}
