//! Companion panels in logic mode.
//!
//! Inbound bytes are the real map-server packets. Clicks and rename text go
//! through [`InputEvent`] and [`LogicSession`]'s injected characters, then the
//! same state methods the client match arms call. Outbound bytes are the
//! packets those arms send.

use korangar_networking::format_pet;
use ragnarok_bytes::{ByteReader, ByteWriter};
use ragnarok_packets::{
    EntityId, HomunculusPropertyPacket, HomunculusSkillListPacket, PacketExt, PetEggListPacket, PetPropertyPacket, RenameHomunculusPacket,
    RenamePetPacket, SelectPetEggPacket, StartCapturePacket, TryCaptureMonsterPacket, TryCaptureResultPacket,
};

use super::logic::LogicSession;
use crate::e2e::LiveConnect;
use crate::input::InputEvent;
use crate::interface::windows::{
    CompanionPanelView, CompanionSkill, CompanionWindowState, EggChoice, HomunculusPanel, PetPanel, SkillView, WindowClass,
};

/// Packets a live map session actually delivered. Empty means the character
/// has no companion data.
#[derive(Clone, Debug, Default)]
pub struct CompanionNote {
    pub pet: bool,
    pub homunculus: bool,
    pub mercenary: bool,
    pub elemental: bool,
    pub capture: bool,
    pub eggs: bool,
}

impl CompanionNote {
    pub fn observe(&mut self, event: &korangar_networking::NetworkEvent) {
        use korangar_networking::{CompanionNotice, NetworkEvent};
        match event {
            NetworkEvent::PetInfo { .. } | NetworkEvent::PetState { .. } => self.pet = true,
            NetworkEvent::HomunculusInfo { .. } => self.homunculus = true,
            NetworkEvent::MercenaryInfo { .. } | NetworkEvent::MercenaryParameter { .. } => self.mercenary = true,
            NetworkEvent::ElementalInfo { .. } | NetworkEvent::ElementalParameter { .. } => self.elemental = true,
            NetworkEvent::CompanionNotice(notice) => match notice {
                CompanionNotice::StartCapture | CompanionNotice::CaptureResult { .. } => self.capture = true,
                CompanionNotice::PetEggs { .. } => self.eggs = true,
                CompanionNotice::HomunculusState { .. }
                | CompanionNotice::HomunculusSkills { .. }
                | CompanionNotice::HomunculusSkillUpdate { .. } => self.homunculus = true,
                CompanionNotice::MercenarySkills { .. } | CompanionNotice::MercenarySkillUpdate { .. } => self.mercenary = true,
            },
            _ => {}
        }
    }

    /// A property, egg list, or skill list arrived. Capture alone does not.
    pub fn saw_owned_companion(&self) -> bool {
        self.pet || self.homunculus || self.mercenary || self.elemental || self.eggs
    }
}

/// Offline companion window plus the packets it would send.
pub struct CompanionSession {
    logic: LogicSession,
    window: CompanionWindowState,
    outbound: Vec<Vec<u8>>,
}

impl CompanionSession {
    pub fn offline() -> Self {
        let mut session = Self::from_logic(LogicSession::offline());
        session.logic.show_window(WindowClass::Companions);
        session
    }

    pub fn start_live() -> LiveConnect<Self> {
        match LogicSession::start_live() {
            LiveConnect::Skip(reason) => LiveConnect::Skip(reason),
            LiveConnect::Invalid(error) => LiveConnect::Invalid(error),
            LiveConnect::Ready(logic) => LiveConnect::Ready(Self::from_logic(logic)),
        }
    }

    fn from_logic(logic: LogicSession) -> Self {
        Self {
            logic,
            window: CompanionWindowState::default(),
            outbound: Vec::new(),
        }
    }

    pub fn username(&self) -> &str {
        self.logic.username()
    }

    /// Log in and open the Pets panel. The panel stays empty until a packet
    /// says otherwise.
    pub fn enter_world(&mut self) -> Result<(), String> {
        self.logic.login_to_character_select()?;
        self.logic.enter_world()?;
        self.logic.show_window(WindowClass::Companions);
        Ok(())
    }

    pub fn window_open(&self) -> bool {
        self.logic.window_names().iter().any(|name| name == "Companions")
    }

    pub fn note(&self) -> &CompanionNote {
        self.logic.companion_note()
    }

    pub fn view(&self) -> CompanionPanelView {
        self.window.panel_view()
    }

    pub fn homunculus_skill(&self, index: usize) -> Option<SkillView> {
        self.window.panel_view().homunculus_skills.into_iter().nth(index)
    }

    pub fn outbound(&self) -> &[Vec<u8>] {
        &self.outbound
    }

    pub fn packet(&self, header: u16) -> Option<&[u8]> {
        self.outbound
            .iter()
            .rev()
            .find(|bytes| packet_header(bytes) == Some(header))
            .map(Vec::as_slice)
    }

    /// Parse one server packet and apply the client state change it causes.
    pub fn feed(&mut self, bytes: &[u8]) -> Result<(), String> {
        let header = packet_header(bytes).ok_or_else(|| "packet is shorter than a header".to_owned())?;
        match header {
            0x019E => {
                let _packet = read_packet::<StartCapturePacket>(bytes)?;
                self.window.arm_capture();
                self.logic.show_window(WindowClass::Companions);
            }
            0x01A0 => {
                let packet = read_packet::<TryCaptureResultPacket>(bytes)?;
                self.window.apply_capture_result(packet.result != 0);
            }
            0x01A2 => {
                let packet = read_packet::<PetPropertyPacket>(bytes)?;
                let summary = format_pet(
                    &packet.name,
                    packet.renamed != 0,
                    packet.level,
                    packet.hunger,
                    packet.intimacy,
                    packet.accessory_id,
                    packet.class_id,
                );
                self.window.show_pet(
                    PetPanel {
                        name: packet.name,
                        renamed: packet.renamed != 0,
                        level: packet.level,
                        hunger: packet.hunger,
                        intimacy: packet.intimacy,
                        accessory: packet.accessory_id,
                        class_id: packet.class_id,
                    },
                    summary,
                );
                self.logic.show_window(WindowClass::Companions);
            }
            0x01A6 => {
                let packet = read_packet::<PetEggListPacket>(bytes)?;
                let eggs = packet
                    .indices
                    .into_iter()
                    .map(|entry| EggChoice {
                        wire_index: entry.index,
                        label: format!("Egg {}", entry.index),
                        item_id: 0,
                        texture: None,
                    })
                    .collect();
                self.window.show_eggs(eggs);
                self.logic.show_window(WindowClass::PetEggs);
            }
            0x0BA4 => {
                let packet = read_packet::<HomunculusPropertyPacket>(bytes)?;
                let summary = format!("{} Lv {}", packet.name, packet.level);
                self.window.show_homunculus(
                    HomunculusPanel {
                        name: packet.name,
                        flags: packet.flags,
                        level: packet.level,
                        hunger: packet.hunger,
                        intimacy: packet.intimacy,
                        attack: packet.attack,
                        magic_attack: packet.magic_attack,
                        hit: packet.hit,
                        critical: packet.critical,
                        defense: packet.defense,
                        magic_defense: packet.magic_defense,
                        flee: packet.flee,
                        attack_motion: packet.attack_motion,
                        health_points: packet.health_points,
                        maximum_health_points: packet.maximum_health_points,
                        spell_points: packet.spell_points,
                        maximum_spell_points: packet.maximum_spell_points,
                        experience: packet.experience,
                        next_experience: packet.next_experience,
                        skill_points: packet.skill_points,
                        attack_range: packet.attack_range,
                    },
                    summary,
                );
                self.logic.show_window(WindowClass::Companions);
            }
            0x0235 => {
                let packet = read_packet::<HomunculusSkillListPacket>(bytes)?;
                let skills = packet
                    .skills
                    .into_iter()
                    .map(|skill| {
                        let name = if skill.name.is_empty() {
                            format!("Skill {}", skill.skill_id)
                        } else {
                            skill.name
                        };
                        let upgradable = skill.upgradable != 0;
                        let mut label = format!("{name}  Lv {}  SP {}", skill.level, skill.sp_cost);
                        if upgradable {
                            label.push_str("  · Level up");
                        }
                        CompanionSkill {
                            skill_id: skill.skill_id,
                            level: skill.level,
                            sp_cost: skill.sp_cost,
                            attack_range: skill.attack_range,
                            name,
                            label,
                            upgradable,
                            texture: None,
                        }
                    })
                    .collect();
                self.window.set_homunculus_skills(skills);
            }
            _ => return Err(format!("companion feed has no handler for {header:#06x}")),
        }
        Ok(())
    }

    /// `InputEvent::PlayerInteract` on a monster. Capture sends `0x019F` and
    /// does not also send an attack command.
    pub fn inject_monster_click(&mut self, entity_id: u32) -> Result<(), String> {
        let event = InputEvent::PlayerInteract {
            entity_id: EntityId(entity_id),
        };
        let InputEvent::PlayerInteract { entity_id } = event else {
            return Err("monster click was not PlayerInteract".to_owned());
        };
        if self.window.accept_capture_click() {
            return self.push(&TryCaptureMonsterPacket { target_id: entity_id.0 });
        }
        self.window.remember_attack_target(entity_id.0);
        if let Some((actor, target)) = self.window.attack_follow_up(entity_id.0) {
            self.push(&ragnarok_packets::ActNpcPacket {
                entity_id: actor,
                target_id: target,
                action: 0,
            })?;
        }
        Ok(())
    }

    /// Egg row click. Sends `0x01A7` with the wire index from the list.
    pub fn inject_egg_row(&mut self, row: usize) -> Result<(), String> {
        let Some(wire_index) = self.window.choose_egg(row) else {
            return Err(format!("egg row {row} is not in the list"));
        };
        let event = InputEvent::SelectPetEgg { wire_index };
        let InputEvent::SelectPetEgg { wire_index } = event else {
            return Err("egg click was not SelectPetEgg".to_owned());
        };
        self.push(&SelectPetEggPacket { index: wire_index })
    }

    /// Close button. The picker sends nothing.
    pub fn inject_close_eggs(&mut self) {
        let _event = InputEvent::ClosePetEggs;
        self.window.cancel_eggs();
    }

    /// Typed name plus Enter, through the injected character frame, then
    /// `0x01A5`.
    pub fn inject_pet_rename(&mut self, name: &str) -> Result<(), String> {
        self.confirm_rename(name, true)
    }

    /// Typed name plus Enter, then `0x0231`.
    pub fn inject_homunculus_rename(&mut self, name: &str) -> Result<(), String> {
        self.confirm_rename(name, false)
    }

    /// Skill row click. Sends `CZ_UPGRADE_SKILLLEVEL` (`0x0112`), not `0x0237`.
    pub fn inject_skill_up(&mut self, skill_id: u16) -> Result<(), String> {
        let event = InputEvent::LevelUpCompanionSkill { skill_id };
        let InputEvent::LevelUpCompanionSkill { skill_id } = event else {
            return Err("skill click was not LevelUpCompanionSkill".to_owned());
        };
        if !self.window.can_level_homunculus_skill(skill_id) {
            return Err(format!("skill {skill_id} cannot be raised"));
        }
        let bytes = korangar_networking::homunculus_skill_up_bytes(skill_id);
        self.window.note_skill_up_sent(skill_id);
        self.outbound.push(bytes);
        Ok(())
    }

    fn confirm_rename(&mut self, name: &str, pet: bool) -> Result<(), String> {
        self.logic.buffer_text(name);
        self.logic.buffer_text("\r");
        let characters = self.logic.take_frame_characters();
        if characters.is_empty() {
            return Err("injected text did not reach the input frame".to_owned());
        }
        let mut confirm = false;
        for character in characters {
            if character == '\r' || character == '\n' {
                confirm = true;
            } else if pet {
                self.window.append_pet_name(character);
            } else {
                self.window.append_homunculus_name(character);
            }
        }
        if !confirm {
            return Err("enter did not confirm the rename".to_owned());
        }
        let event = if pet {
            InputEvent::ConfirmPetRename
        } else {
            InputEvent::ConfirmHomunculusRename
        };
        let name = match event {
            InputEvent::ConfirmPetRename => self.window.confirm_pet_rename().map_err(str::to_owned)?,
            InputEvent::ConfirmHomunculusRename => self.window.confirm_homunculus_rename().map_err(str::to_owned)?,
            _ => return Err("rename confirm was not a companion event".to_owned()),
        };
        if pet {
            self.push(&RenamePetPacket { name })
        } else {
            self.push(&RenameHomunculusPacket { name })
        }
    }

    fn push<T: PacketExt>(&mut self, packet: &T) -> Result<(), String> {
        let mut writer = ByteWriter::new();
        packet.packet_to_bytes(&mut writer).map_err(|error| format!("encode: {error:?}"))?;
        self.outbound.push(writer.into_inner());
        Ok(())
    }
}

fn packet_header(bytes: &[u8]) -> Option<u16> {
    let pair = bytes.get(..2)?;
    Some(u16::from_le_bytes([pair[0], pair[1]]))
}

fn read_packet<T: PacketExt>(bytes: &[u8]) -> Result<T, String> {
    let mut reader = ByteReader::without_metadata(bytes);
    let packet = T::packet_from_bytes(&mut reader).map_err(|error| format!("parse: {error:?}"))?;
    if !reader.is_empty() {
        return Err("packet left unread bytes".to_owned());
    }
    Ok(packet)
}
