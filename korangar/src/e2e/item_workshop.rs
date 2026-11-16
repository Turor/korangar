//! Offline item reform and laphine windows.
//!
//! Packets are the Hercules layouts. Clicks are injected input events, the
//! same path the picker rows queue.

use ragnarok_bytes::ByteReader;
use ragnarok_packets::{
    ItemReformOpenPacket, ItemReformResultPacket, LaphineSynthesisOpenPacket, LaphineSynthesisResultPacket, LaphineUpgradeOpenPacket,
    LaphineUpgradeResultPacket, Packet, PacketExt,
};

use crate::e2e::logic::LogicSession;
use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::state::item_workshop::{
    ItemReformState, LaphineSynthesisState, LaphineUpgradeState, WorkshopCatalog, WorkshopSnap, catalog_from_lua_source,
    reform_close_bytes, reform_request_bytes, synthesis_close_bytes, synthesis_request_bytes, upgrade_close_bytes, upgrade_request_bytes,
};

/// What the reform and laphine windows are showing, plus the bytes they sent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkshopView {
    pub windows: Vec<String>,
    pub reform_status: String,
    pub reform_catalyst: String,
    pub reform_candidates: Vec<String>,
    pub reform_materials: Vec<String>,
    pub reform_preview: String,
    pub reform_kept: String,
    pub reform_enabled: bool,
    pub synthesis_status: String,
    pub synthesis_box: String,
    pub synthesis_requirement: String,
    pub synthesis_candidates: Vec<String>,
    pub synthesis_chosen: Vec<String>,
    pub synthesis_enabled: bool,
    pub upgrade_status: String,
    pub upgrade_box: String,
    pub upgrade_candidates: Vec<String>,
    pub upgrade_preview: String,
    pub upgrade_enabled: bool,
    pub outbound: Vec<Vec<u8>>,
    pub chat: Vec<String>,
}

enum WorkshopEffect {
    None,
    Open(WindowClass),
    Close(WindowClass),
}

#[derive(Default)]
pub struct ItemWorkshopSession {
    reform: ItemReformState,
    synthesis: LaphineSynthesisState,
    upgrade: LaphineUpgradeState,
    catalog: WorkshopCatalog,
    snaps: Vec<WorkshopSnap>,
    outbound: Vec<Vec<u8>>,
    chat: Vec<String>,
}

impl ItemWorkshopSession {
    fn view(&self) -> WorkshopView {
        WorkshopView {
            windows: Vec::new(),
            reform_status: self.reform.status.clone(),
            reform_catalyst: self.reform.catalyst_name.clone(),
            reform_candidates: labels(&self.reform.candidates),
            reform_materials: labels(&self.reform.materials),
            reform_preview: self.reform.result_preview.clone(),
            reform_kept: self.reform.kept.clone(),
            reform_enabled: self.reform.submit_enabled,
            synthesis_status: self.synthesis.status.clone(),
            synthesis_box: self.synthesis.box_name.clone(),
            synthesis_requirement: self.synthesis.requirement.clone(),
            synthesis_candidates: labels(&self.synthesis.candidates),
            synthesis_chosen: labels(&self.synthesis.chosen),
            synthesis_enabled: self.synthesis.submit_enabled,
            upgrade_status: self.upgrade.status.clone(),
            upgrade_box: self.upgrade.box_name.clone(),
            upgrade_candidates: labels(&self.upgrade.candidates),
            upgrade_preview: self.upgrade.result_preview.clone(),
            upgrade_enabled: self.upgrade.submit_enabled,
            outbound: self.outbound.clone(),
            chat: self.chat.clone(),
        }
    }

    fn feed(&mut self, bytes: &[u8]) -> Result<WorkshopEffect, String> {
        if bytes.len() < 2 {
            return Err("workshop packet is too short".to_owned());
        }
        let header = u16::from_le_bytes([bytes[0], bytes[1]]);
        let mut reader = ByteReader::without_metadata(bytes);
        match header {
            0x0B8F => {
                let packet = read_packet::<ItemReformOpenPacket>(&mut reader)?;
                self.reform.open(packet.item_id.0, &self.snaps, &self.catalog);
                Ok(WorkshopEffect::Open(WindowClass::ItemReform))
            }
            0x0B92 => {
                let packet = read_packet::<ItemReformResultPacket>(&mut reader)?;
                let close = self.reform.note_result(packet.index.0, packet.result);
                self.chat.push(self.reform.status.clone());
                if close {
                    self.outbound.push(reform_close_bytes());
                    Ok(WorkshopEffect::Close(WindowClass::ItemReform))
                } else {
                    Ok(WorkshopEffect::None)
                }
            }
            0x0A4E => {
                let packet = read_packet::<LaphineSynthesisOpenPacket>(&mut reader)?;
                self.synthesis.open(packet.item_id.0, &self.snaps, &self.catalog);
                Ok(WorkshopEffect::Open(WindowClass::LaphineSynthesis))
            }
            0x0A50 => {
                let packet = read_packet::<LaphineSynthesisResultPacket>(&mut reader)?;
                let close = self.synthesis.note_result(packet.result);
                self.chat.push(self.synthesis.status.clone());
                if close {
                    self.outbound.push(synthesis_close_bytes());
                    Ok(WorkshopEffect::Close(WindowClass::LaphineSynthesis))
                } else {
                    Ok(WorkshopEffect::None)
                }
            }
            0x0AB4 => {
                let packet = read_packet::<LaphineUpgradeOpenPacket>(&mut reader)?;
                self.upgrade.open(packet.item_id.0, &self.snaps, &self.catalog);
                Ok(WorkshopEffect::Open(WindowClass::LaphineUpgrade))
            }
            0x0AB7 => {
                let packet = read_packet::<LaphineUpgradeResultPacket>(&mut reader)?;
                let close = self.upgrade.note_result(packet.result);
                self.chat.push(self.upgrade.status.clone());
                if close {
                    self.outbound.push(upgrade_close_bytes());
                    Ok(WorkshopEffect::Close(WindowClass::LaphineUpgrade))
                } else {
                    Ok(WorkshopEffect::None)
                }
            }
            _ => Err(format!("unknown workshop packet {header:#06X}")),
        }
    }

    fn on_input(&mut self, event: &InputEvent) -> Option<WindowClass> {
        match event {
            InputEvent::ItemReformSelect { index } => {
                self.reform.select(*index);
                None
            }
            InputEvent::ItemReformSubmit => {
                match self.reform.request() {
                    Ok((item_id, index)) => self.outbound.push(reform_request_bytes(item_id, index)),
                    Err(message) => self.reform.status = message.to_owned(),
                }
                None
            }
            InputEvent::ItemReformClose => {
                self.outbound.push(reform_close_bytes());
                Some(WindowClass::ItemReform)
            }
            InputEvent::LaphineSynthesisToggle { index } => {
                self.synthesis.toggle(*index);
                None
            }
            InputEvent::LaphineSynthesisClear => {
                self.synthesis.clear_rows();
                None
            }
            InputEvent::LaphineSynthesisAdd => {
                self.synthesis.status = "Choose an item from the list.".to_owned();
                None
            }
            InputEvent::LaphineSynthesisSubmit => {
                match self.synthesis.request() {
                    Ok((item_id, stacks)) => self.outbound.push(synthesis_request_bytes(item_id, &stacks)),
                    Err(message) => self.synthesis.status = message.to_owned(),
                }
                None
            }
            InputEvent::LaphineSynthesisClose => {
                self.outbound.push(synthesis_close_bytes());
                Some(WindowClass::LaphineSynthesis)
            }
            InputEvent::LaphineUpgradeSelect { index } => {
                self.upgrade.select(*index);
                None
            }
            InputEvent::LaphineUpgradeSubmit => {
                match self.upgrade.request() {
                    Ok((item_id, index)) => self.outbound.push(upgrade_request_bytes(item_id, index)),
                    Err(message) => self.upgrade.status = message.to_owned(),
                }
                None
            }
            InputEvent::LaphineUpgradeClose => {
                self.outbound.push(upgrade_close_bytes());
                Some(WindowClass::LaphineUpgrade)
            }
            _ => None,
        }
    }
}

fn labels(rows: &[crate::state::picker::PickerRow]) -> Vec<String> {
    rows.iter().map(|row| row.label.clone()).collect()
}

fn read_packet<P: Packet>(reader: &mut ByteReader<'_>) -> Result<P, String> {
    P::packet_from_bytes(reader).map_err(|_| "workshop packet did not decode".to_owned())
}

impl LogicSession {
    /// Install the client reform and laphine tables from a lua source.
    pub fn load_workshop_lua(&mut self, source: &str) -> Result<(), String> {
        self.workshop.catalog = catalog_from_lua_source(source)?;
        Ok(())
    }

    /// One equippable inventory row the workshop windows can offer.
    pub fn seed_workshop_equip(&mut self, index: u16, item_id: u32, name: &str, refine: u8, card: u32) {
        self.workshop.snaps.push(WorkshopSnap {
            index,
            item_id,
            name: name.to_owned(),
            amount: 1,
            refine,
            option_count: 0,
            slot_count: u8::from(card != 0),
            cards: if card == 0 { Vec::new() } else { vec![card] },
            card_names: if card == 0 { Vec::new() } else { vec![format!("Item {card}")] },
            equip: true,
        });
    }

    /// One stackable inventory row.
    pub fn seed_workshop_stack(&mut self, index: u16, item_id: u32, name: &str, amount: u16) {
        self.workshop.snaps.push(WorkshopSnap {
            index,
            item_id,
            name: name.to_owned(),
            amount,
            ..WorkshopSnap::default()
        });
    }

    /// Feed one server packet (`ZC_OPEN_*` or `ZC_*_ACK`).
    pub fn feed_workshop_packet(&mut self, bytes: &[u8]) -> Result<(), String> {
        match self.workshop.feed(bytes)? {
            WorkshopEffect::None => {}
            WorkshopEffect::Open(class) => self.open_window(class),
            WorkshopEffect::Close(class) => self.close_window(class),
        }
        Ok(())
    }

    pub fn workshop_view(&self) -> WorkshopView {
        let mut view = self.workshop.view();
        view.windows = self.window_names();
        view
    }

    pub(super) fn apply_workshop_event(&mut self, event: &InputEvent) {
        if let Some(class) = self.workshop.on_input(event) {
            self.close_window(class);
        }
    }
}
