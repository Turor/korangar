use std::cell::RefCell;
use std::net::IpAddr;
use std::rc::Rc;
use std::time::Instant;

use ragnarok_packets::handler::{DuplicateHandlerError, PacketCallback, PacketHandler};
use ragnarok_packets::version_20250716::{
    AddItemToCartPacket, AddSkillPacket, DisplayExpandedEmotionPacket, EquippableItemListPacket, ItemPickupPacket, TradeAddItemPacket,
    TradeAddItemResultPacket, TradeCancelPacket, TradeCompletePacket, TradeConfirmPacket, TradeRequestPacket, TradeStartPacket,
    TradeUnlockPacket,
};
use ragnarok_packets::*;

use crate::event::{NetworkEventList, NoNetworkEvents, minimap_mark_event, navigate_to_event};
use crate::items::ItemQuantity;
use crate::packet_versions::economy::open_market_event;
use crate::{
    CharacterServerLoginData, GuildMemberSnapshot, HotkeyState, InventoryItem, InventoryItemDetails, LoginServerLoginData, MessageColor,
    NetworkEvent, NoMetadata, PincodeNotice, ShopItem, UnifiedCharacterSelectionFailedReason, UnifiedLoginFailedReason,
};

pub fn register_login_server_packets<Callback>(
    packet_handler: &mut PacketHandler<NetworkEventList, Callback>,
) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: LoginServerLoginSuccessPacket| NetworkEvent::LoginServerConnected {
        character_servers: packet.character_server_information,
        login_data: LoginServerLoginData {
            account_id: packet.account_id,
            login_id1: packet.login_id1,
            login_id2: packet.login_id2,
            sex: packet.sex,
        },
    })?;
    packet_handler.register(|packet: LoginFailedPacket| {
        let (reason, message) = match packet.reason {
            LoginFailedReason::ServerClosed => (UnifiedLoginFailedReason::ServerClosed, "Server closed".to_string()),
            LoginFailedReason::AlreadyLoggedIn => (
                UnifiedLoginFailedReason::AlreadyLoggedIn,
                "Someone has already logged in with this id".to_string(),
            ),
            LoginFailedReason::AlreadyOnline => (UnifiedLoginFailedReason::AlreadyOnline, "Already online".to_string()),
        };

        NetworkEvent::LoginServerConnectionFailed { reason, message }
    })?;
    packet_handler.register(|packet: LoginFailedPacket2| {
        let (reason, message) = match packet.reason {
            LoginFailedReason2::UnregisteredId => (UnifiedLoginFailedReason::UnregisteredId, "Unregistered id".to_string()),
            LoginFailedReason2::IncorrectPassword => (UnifiedLoginFailedReason::IncorrectPassword, "Incorrect password".to_string()),
            LoginFailedReason2::IdExpired => (UnifiedLoginFailedReason::IdExpired, "Id has expired".to_string()),
            LoginFailedReason2::RejectedFromServer => (UnifiedLoginFailedReason::RejectedFromServer, "Rejected from server".to_string()),
            LoginFailedReason2::BlockedByGMTeam => (UnifiedLoginFailedReason::BlockedByGMTeam, "Blocked by gm team".to_string()),
            LoginFailedReason2::GameOutdated => (UnifiedLoginFailedReason::GameOutdated, "Game outdated".to_string()),
            LoginFailedReason2::LoginProhibitedUntil => (
                UnifiedLoginFailedReason::LoginProhibitedUntil,
                format!("You are prohibited to log in until {}.", packet.date),
            ),
            LoginFailedReason2::ServerFull => (UnifiedLoginFailedReason::ServerFull, "Server is full".to_string()),
            LoginFailedReason2::CompanyAccountLimitReached => (
                UnifiedLoginFailedReason::CompanyAccountLimitReached,
                "Company account limit reached".to_string(),
            ),
            LoginFailedReason2::BannedByDBATeam => (UnifiedLoginFailedReason::BannedByDBATeam, "Banned by DBA team".to_string()),
            LoginFailedReason2::UnconfirmedEmail => (UnifiedLoginFailedReason::UnconfirmedEmail, "Email not confirmed".to_string()),
            LoginFailedReason2::BannedByGMTeam => (UnifiedLoginFailedReason::BannedByGMTeam, "Banned by GM team".to_string()),
            LoginFailedReason2::TemporaryBanForDatabaseWork => (
                UnifiedLoginFailedReason::TemporaryBanForDatabaseWork,
                "Working in DB".to_string(),
            ),
            LoginFailedReason2::SelfLocked => (UnifiedLoginFailedReason::SelfLocked, "Self lock".to_string()),
            LoginFailedReason2::NotPermittedGroup => (UnifiedLoginFailedReason::NotPermittedGroup, "Not Permitted Group".to_string()),
            LoginFailedReason2::AccountIdErased => (
                UnifiedLoginFailedReason::AccountIdErased,
                "This ID has been totally erased".to_string(),
            ),
            LoginFailedReason2::LoginInformationRemains => (
                UnifiedLoginFailedReason::LoginInformationRemains,
                format!("Login information remains at {}", packet.date),
            ),
            LoginFailedReason2::LockedForHackingInvestigation => (
                UnifiedLoginFailedReason::LockedForHackingInvestigation,
                "Account has been locked for a hacking investigation. Please contact the GM Team for more information".to_string(),
            ),
            LoginFailedReason2::TemporaryLockedForBugInvestigation => (
                UnifiedLoginFailedReason::TemporaryLockedForBugInvestigation,
                "This account has been temporarily prohibited from login due to a bug-related investigation".to_string(),
            ),
            LoginFailedReason2::DeletingCharacter => (
                UnifiedLoginFailedReason::DeletingCharacter,
                "This character is being deleted. Login is temporarily unavailable for the time being".to_string(),
            ),
            LoginFailedReason2::DeletingSpouseCharacter => (
                UnifiedLoginFailedReason::DeletingSpouseCharacter,
                "This character is being deleted. Login is temporarily unavailable for the time being".to_string(),
            ),
            LoginFailedReason2::UnknownError => (UnifiedLoginFailedReason::UnknownError, "Unknown error".to_string()),
        };

        NetworkEvent::LoginServerConnectionFailed { reason, message }
    })?;

    Ok(())
}

pub fn register_character_server_packets<Callback>(
    packet_handler: &mut PacketHandler<NetworkEventList, Callback>,
) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: LoginFailedPacket| {
        let reason = packet.reason;
        let message = match reason {
            LoginFailedReason::ServerClosed => "Server closed",
            LoginFailedReason::AlreadyLoggedIn => "Someone has already logged in with this id",
            LoginFailedReason::AlreadyOnline => "Already online",
        };

        NetworkEvent::CharacterServerConnectionFailed { reason, message }
    })?;
    packet_handler.register(
        |packet: CharacterServerLoginSuccessPacket| NetworkEvent::CharacterServerConnected {
            normal_slot_count: packet.normal_slot_count as usize,
        },
    )?;
    packet_handler.register(|packet: RequestCharacterListSuccessPacket| NetworkEvent::CharacterList {
        characters: packet.character_information,
    })?;
    packet_handler.register_noop::<CharacterListPacket>()?;
    packet_handler.register_noop::<CharacterSlotPagePacket>()?;
    packet_handler.register_noop::<CharacterBanListPacket>()?;
    packet_handler.register(|packet: LoginPincodePacket| NetworkEvent::Pincode {
        notice: PincodeNotice::Login {
            seed: packet.pincode_seed,
            account_id: packet.account_id,
            state: packet.state,
            flag: None,
        },
    })?;
    packet_handler.register(|packet: SecondPasswdLogin2Packet| NetworkEvent::Pincode {
        notice: PincodeNotice::Login {
            seed: packet.seed,
            account_id: packet.account_id,
            state: packet.state,
            flag: Some(packet.flag),
        },
    })?;
    packet_handler.register(|packet: MakeSecondPasswdResultPacket| NetworkEvent::Pincode {
        notice: PincodeNotice::MakeResult {
            state: packet.state,
            seed: packet.seed,
        },
    })?;
    packet_handler.register(|packet: EditSecondPasswdResultPacket| NetworkEvent::Pincode {
        notice: PincodeNotice::EditResult {
            state: packet.state,
            seed: packet.seed,
        },
    })?;
    packet_handler.register_noop::<Packet0b18>()?;
    packet_handler.register(|packet: CharacterSelectionSuccessPacket| {
        let login_data = CharacterServerLoginData {
            server_ip: IpAddr::V4(packet.map_server_ip.into()),
            server_port: packet.map_server_port,
            character_id: packet.character_id,
        };

        NetworkEvent::CharacterSelected { login_data }
    })?;
    packet_handler.register(|packet: CharacterSelectionFailedPacket| {
        let (reason, message) = match packet.reason {
            CharacterSelectionFailedReason::RejectedFromServer => (
                UnifiedCharacterSelectionFailedReason::RejectedFromServer,
                "Rejected from server",
            ),
        };

        NetworkEvent::CharacterSelectionFailed { reason, message }
    })?;
    packet_handler.register(|_: MapServerUnavailablePacket| {
        let reason = UnifiedCharacterSelectionFailedReason::MapServerUnavailable;
        let message = "Map server currently unavailable";

        NetworkEvent::CharacterSelectionFailed { reason, message }
    })?;
    packet_handler.register(|packet: CreateCharacterSuccessPacket| NetworkEvent::CharacterCreated {
        character_information: packet.character_information,
    })?;
    packet_handler.register(|packet: CharacterCreationFailedPacket| {
        let reason = packet.reason;
        let message = match reason {
            CharacterCreationFailedReason::CharacterNameAlreadyUsed => "Character name is already used",
            CharacterCreationFailedReason::NotOldEnough => "You are not old enough to create a character",
            CharacterCreationFailedReason::NotAllowedToUseSlot => "You are not allowed to use this character slot",
            CharacterCreationFailedReason::CharacterCerationFailed => "Character creation failed",
        };

        NetworkEvent::CharacterCreationFailed { reason, message }
    })?;
    packet_handler.register(|_: CharacterDeletionSuccessPacket| NetworkEvent::CharacterDeleted)?;
    packet_handler.register(|packet: CharacterDeletionFailedPacket| {
        let reason = packet.reason;
        let message = match reason {
            CharacterDeletionFailedReason::NotAllowed => "You are not allowed to delete this character",
            CharacterDeletionFailedReason::CharacterNotFound => "Character was not found",
            CharacterDeletionFailedReason::NotEligible => "Character is not eligible for deletion",
        };
        NetworkEvent::CharacterDeletionFailed { reason, message }
    })?;
    packet_handler.register(|packet: SwitchCharacterSlotResponsePacket| match packet.status {
        SwitchCharacterSlotResponseStatus::Success => NetworkEvent::CharacterSlotSwitched,
        SwitchCharacterSlotResponseStatus::Error => NetworkEvent::CharacterSlotSwitchFailed,
    })?;

    Ok(())
}

pub fn register_map_server_packets<Callback>(
    packet_handler: &mut PacketHandler<NetworkEventList, Callback>,
) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    // This is a bit of a workaround for the way that the inventory is
    // sent. There is a single packet to start the inventory list,
    // followed by an arbitary number of item packets, and in the
    // end a sinle packet to mark the list as complete.
    //
    // This variable provides some transient storage shared by all the inventory
    // handlers.
    let inventory_items: Rc<RefCell<crate::inventory_lists::InventoryLists<InventoryItem<NoMetadata>>>> =
        Rc::new(RefCell::new(Default::default()));

    packet_handler.register(|_: MapServerPingPacket| NoNetworkEvents)?;
    packet_handler.register(|packet: BroadcastMessagePacket| NetworkEvent::BroadcastMessage {
        text: packet.message,
        color: MessageColor::Broadcast,
    })?;
    packet_handler.register(|packet: Broadcast2MessagePacket| {
        // Drop the alpha channel because it might be 0.
        let color = MessageColor::Rgb {
            red: packet.font_color.red,
            green: packet.font_color.green,
            blue: packet.font_color.blue,
        };
        NetworkEvent::BroadcastMessage {
            text: packet.message,
            color,
        }
    })?;
    packet_handler.register(|packet: OverheadMessagePacket| NetworkEvent::EntitySpeech {
        entity_id: packet.entity_id,
        text: packet.message,
        color: MessageColor::Public,
    })?;
    packet_handler.register(|packet: ServerMessagePacket| NetworkEvent::ChatMessage {
        text: packet.message,
        color: MessageColor::Server,
    })?;
    packet_handler.register_noop::<MessageTablePacket>()?;
    packet_handler.register(|packet: EntityMessagePacket| {
        // Drop the alpha channel because it might be 0.
        let color = MessageColor::Rgb {
            red: packet.color.red,
            green: packet.color.green,
            blue: packet.color.blue,
        };
        NetworkEvent::EntitySpeech {
            entity_id: packet.entity_id,
            text: packet.message,
            color,
        }
    })?;
    packet_handler.register(|packet: WhisperReceivedPacket| NetworkEvent::WhisperReceived {
        sender: packet.sender,
        text: packet.message,
        is_admin: packet.is_admin != 0,
    })?;
    packet_handler.register(|packet: WhisperReceived2Packet| NetworkEvent::WhisperReceived {
        sender: packet.sender,
        text: packet.message,
        is_admin: packet.is_admin != 0,
    })?;
    packet_handler.register(|packet: WhisperAckPacket| NetworkEvent::WhisperAck { result: packet.result })?;
    packet_handler.register(|packet: WhisperAck2Packet| NetworkEvent::WhisperAck { result: packet.result })?;
    packet_handler.register(|packet: DisplayEmotionPacket| NetworkEvent::DisplayEmotion {
        entity_id: packet.entity_id,
        emotion: packet.emotion,
    })?;
    packet_handler.register(|packet: EntityMovePacket| {
        let EntityMovePacket {
            entity_id,
            from_to,
            starting_timestamp,
        } = packet;

        let (origin, destination) = from_to.to_origin_destination();

        NetworkEvent::EntityMove {
            entity_id,
            origin,
            destination,
            starting_timestamp,
        }
    })?;
    packet_handler.register(|packet: EntityStopMovePacket| NetworkEvent::EntityStopMove {
        entity_id: packet.entity_id,
        position: packet.position,
    })?;
    packet_handler.register(|packet: PlayerMovePacket| {
        let PlayerMovePacket {
            starting_timestamp,
            from_to,
        } = packet;

        let (origin, destination) = from_to.to_origin_destination();

        NetworkEvent::PlayerMove {
            origin,
            destination,
            starting_timestamp,
        }
    })?;
    packet_handler.register(|packet: ChangeMapPacket| {
        let ChangeMapPacket { map_name, position } = packet;

        let map_name = map_name.replace(".gat", "");

        NetworkEvent::ChangeMap { map_name, position }
    })?;
    packet_handler.register(|packet: ResurrectionPacket| NetworkEvent::ResurrectPlayer {
        entity_id: packet.entity_id,
    })?;
    packet_handler.register(|packet: EntityAppearPacket| NetworkEvent::AddEntity {
        entity_data: packet.into(),
    })?;
    packet_handler.register(|packet: EntityAppear2Packet| NetworkEvent::AddEntity {
        entity_data: packet.into(),
    })?;
    packet_handler.register(|packet: MovingEntityAppearPacket| NetworkEvent::AddEntity {
        entity_data: packet.into(),
    })?;
    packet_handler.register(|packet: EntityDisAppearPacket| NetworkEvent::RemoveEntity {
        entity_id: packet.entity_id,
        reason: packet.reason,
    })?;
    packet_handler.register(|packet: GroundItemAppearPacket| NetworkEvent::AddGroundItem {
        entity_id: packet.entity_id,
        item_id: packet.item_id,
        is_identified: packet.is_identified != 0,
        quantity: packet.quantity,
        position: packet.position,
        x_offset: packet.x_offset,
        y_offset: packet.y_offset,
    })?;
    packet_handler.register(|packet: GroundItemAppear2Packet| NetworkEvent::AddGroundItem {
        entity_id: packet.entity_id,
        item_id: packet.item_id,
        is_identified: packet.is_identified != 0,
        quantity: packet.quantity,
        position: packet.position,
        x_offset: packet.x_offset,
        y_offset: packet.y_offset,
    })?;
    packet_handler.register(|packet: GroundItemAppear3Packet| NetworkEvent::AddGroundItem {
        entity_id: packet.entity_id,
        item_id: packet.item_id,
        is_identified: packet.is_identified != 0,
        quantity: packet.quantity,
        position: packet.position,
        x_offset: packet.x_offset,
        y_offset: packet.y_offset,
    })?;
    packet_handler.register(|packet: GroundItemAppear4Packet| NetworkEvent::AddGroundItem {
        entity_id: packet.entity_id,
        item_id: packet.item_id,
        is_identified: packet.is_identified != 0,
        quantity: packet.quantity,
        position: packet.position,
        x_offset: packet.x_offset,
        y_offset: packet.y_offset,
    })?;
    packet_handler.register(|packet: ItemDisappearPacket| NetworkEvent::RemoveGroundItem {
        entity_id: packet.entity_id,
    })?;
    packet_handler.register(|packet: UpdateStatPacket| {
        let UpdateStatPacket { stat_type } = packet;
        NetworkEvent::UpdateStat { stat_type }
    })?;
    packet_handler.register(|packet: UpdateStatPacket1| {
        let UpdateStatPacket1 { stat_type } = packet;
        NetworkEvent::UpdateStat { stat_type }
    })?;
    packet_handler.register(|packet: UpdateStatPacket2| {
        let UpdateStatPacket2 { stat_type } = packet;
        NetworkEvent::UpdateStat { stat_type }
    })?;
    packet_handler.register(|packet: UpdateStatPacket3| {
        let UpdateStatPacket3 { stat_type } = packet;
        NetworkEvent::UpdateStat { stat_type }
    })?;
    packet_handler.register_noop::<UpdateAttackRangePacket>()?;
    crate::packet_versions::rodex::register_rodex_packets(packet_handler)?;
    packet_handler.register_noop::<CriticalWeightUpdatePacket>()?;
    packet_handler.register(crate::packet_versions::styling_shop::sprite_change_event)?;
    packet_handler.register({
        let inventory_items = inventory_items.clone();

        move |packet: InventoyStartPacket| {
            inventory_items.borrow_mut().start(packet.inventory_type);
            NoNetworkEvents
        }
    })?;
    packet_handler.register({
        let inventory_items = inventory_items.clone();

        move |packet: RegularItemListPacket| {
            inventory_items.borrow_mut().extend(
                packet.inventory_type,
                packet.item_information.into_iter().map(|item_information| {
                    let RegularItemInformation {
                        index,
                        item_id,
                        item_type,
                        amount,
                        equipped_position,
                        slot,
                        hire_expiration_date,
                        flags,
                    } = item_information;

                    InventoryItem {
                        index,
                        metadata: NoMetadata,
                        item_id,
                        item_type,
                        slot,
                        hire_expiration_date,
                        details: InventoryItemDetails::Regular {
                            amount,
                            equipped_position,
                            flags,
                        },
                    }
                }),
            );
            NoNetworkEvents
        }
    })?;
    packet_handler.register({
        let inventory_items = inventory_items.clone();

        move |packet: EquippableItemListPacket| {
            inventory_items.borrow_mut().extend(
                packet.inventory_type,
                packet.item_information.into_iter().map(|item| {
                    let forge_data = item.forge_data.clone();
                    let EquippableItemInformation {
                        index,
                        item_id,
                        item_type,
                        equip_position,
                        equipped_position,
                        slot,
                        hire_expiration_date,
                        bind_on_equip_type,
                        w_item_sprite_number,
                        option_count,
                        option_data,
                        refinement_level,
                        enchantment_level,
                        flags,
                    } = ragnarok_packets::EquippableItemInformation::from(item);

                    InventoryItem {
                        index,
                        metadata: NoMetadata,
                        item_id,
                        item_type,
                        slot,
                        hire_expiration_date,
                        details: InventoryItemDetails::Equippable {
                            equip_position,
                            equipped_position,
                            bind_on_equip_type,
                            w_item_sprite_number,
                            option_count,
                            option_data,
                            forge_data: Some(forge_data),
                            refinement_level,
                            enchantment_level,
                            flags,
                        },
                    }
                }),
            );
            NoNetworkEvents
        }
    })?;
    packet_handler.register({
        let inventory_items = inventory_items.clone();

        move |packet: InventoyEndPacket| match inventory_items.borrow_mut().finish(packet.inventory_type, packet.flag) {
            Some((crate::inventory_lists::InventoryListKind::Player, items)) => Some(NetworkEvent::SetInventory { items }),
            Some((crate::inventory_lists::InventoryListKind::Cart, items)) => Some(NetworkEvent::SetCart { items }),
            Some((crate::inventory_lists::InventoryListKind::Storage, items)) => Some(NetworkEvent::SetStorage { items }),
            None => None,
        }
    })?;
    crate::equip_switch::register_packets(packet_handler)?;
    packet_handler.register_noop::<MapTypePacket>()?;
    packet_handler.register(|packet: UpdateSkillTreePacket| {
        let UpdateSkillTreePacket { skill_information } = packet;
        NetworkEvent::SkillTree { skill_information }
    })?;
    packet_handler.register(|packet: UpdateHotkeysPacket| NetworkEvent::SetHotkeyData {
        tab: packet.tab,
        hotkeys: packet
            .hotkeys
            .into_iter()
            .map(|hotkey_data| match hotkey_data == HotkeyData::UNBOUND {
                true => HotkeyState::Unbound,
                false => HotkeyState::Bound(hotkey_data),
            })
            .collect(),
    })?;
    packet_handler.register(|packet: InitialStatsPacket| {
        let InitialStatsPacket {
            strength_stat_points_cost,
            agility_stat_points_cost,
            vitality_stat_points_cost,
            intelligence_stat_points_cost,
            dexterity_stat_points_cost,
            luck_stat_points_cost,
            ..
        } = packet;

        NetworkEvent::InitialStats {
            strength_stat_points_cost,
            agility_stat_points_cost,
            vitality_stat_points_cost,
            intelligence_stat_points_cost,
            dexterity_stat_points_cost,
            luck_stat_points_cost,
        }
    })?;
    packet_handler.register(|packet: UpdatePartyInvitationStatePacket| NetworkEvent::PartyInvitationState {
        // `0x02C9` is a refuse flag (`denyPartyInvites`): 0 allows invites, non-zero denies them.
        invitations_allowed: packet.allowed == 0,
    })?;
    packet_handler.register_noop::<UpdateShowEquipPacket>()?;
    packet_handler.register_noop::<UpdateConfigurationPacket>()?;
    packet_handler.register(navigate_to_event)?;
    packet_handler.register(minimap_mark_event)?;
    packet_handler.register(|packet: NextButtonPacket| {
        let NextButtonPacket { npc_id } = packet;

        NetworkEvent::AddNextButton { npc_id }
    })?;
    packet_handler.register(|packet: CloseButtonPacket| {
        let CloseButtonPacket { npc_id } = packet;

        NetworkEvent::AddCloseButton { npc_id }
    })?;
    packet_handler.register(|packet: DialogMenuPacket| {
        let DialogMenuPacket { npc_id, message } = packet;

        let choices = message.split(':').map(String::from).filter(|text| !text.is_empty()).collect();

        NetworkEvent::AddChoiceButtons { choices, npc_id }
    })?;
    packet_handler.register(|packet: DisplaySpecialEffectPacket| NetworkEvent::SpecialEffect {
        entity_id: packet.entity_id,
        effect_id: packet.effect_id,
    })?;
    packet_handler.register(|packet: DisplaySkillCooldownPacket| NetworkEvent::SkillCooldown {
        skill_id: packet.skill_id,
        until: packet.until,
    })?;
    packet_handler.register(|packet: DisplaySkillEffectAndDamagePacket| NetworkEvent::SkillEffectAndDamage {
        skill_id: packet.skill_id,
        source_entity_id: packet.source_entity_id,
        destination_entity_id: packet.destination_entity_id,
        start_time: packet.start_time,
        source_delay: packet.soruce_delay,
        destination_delay: packet.destination_delay,
        damage: packet.damage,
        level: packet.level,
        hit_count: packet.div,
        skill_type: packet.skill_type,
    })?;
    packet_handler.register(|packet: DisplaySkillEffectNoDamagePacket| NetworkEvent::HealEffect {
        entity_id: packet.destination_entity_id,
        source_entity_id: packet.source_entity_id,
        skill_id: packet.skill_id,
        heal_amount: packet.heal_amount as usize,
        result: packet.result,
    })?;
    packet_handler.register(|packet: DisplayPlayerHealEffect| NetworkEvent::PlayerHealEffect {
        heal_type: packet.heal_type,
        heal_amount: packet.heal_amount,
    })?;
    packet_handler.register(|packet: StatusChangePacket| {
        let StatusChangePacket {
            index,
            entity_id,
            state,
            duration_in_milliseconds,
            remaining_in_milliseconds,
            value,
        } = packet;

        NetworkEvent::StatusChange {
            status_index: index,
            entity_id,
            state: crate::event::StatusChangeState::from_wire(state),
            total_duration_milliseconds: duration_in_milliseconds,
            remaining_milliseconds: remaining_in_milliseconds,
            values: value,
        }
    })?;
    packet_handler.register(|packet: QuestNotificationPacket1| {
        let QuestNotificationPacket1 {
            quest_id,
            active,
            start_time,
            expire_time,
            objective_count,
            objective_details,
        } = packet;

        let objectives = objective_details.into_iter().take(objective_count as usize).collect();

        NetworkEvent::QuestNotification {
            quest_id,
            active: active != 0,
            start_time,
            expire_time,
            objectives,
        }
    })?;
    packet_handler.register(
        |packet: HuntingQuestNotificationPacket| NetworkEvent::HuntingQuestNotification {
            objectives: packet.objective_details,
        },
    )?;
    packet_handler.register(
        |packet: HuntingQuestUpdateObjectivePacket| NetworkEvent::HuntingQuestObjectiveUpdate {
            objectives: packet.objective_details,
        },
    )?;
    packet_handler.register(|packet: QuestRemovedPacket| NetworkEvent::QuestRemoved { quest_id: packet.quest_id })?;
    packet_handler.register(|packet: QuestListPacket| NetworkEvent::QuestList { quests: packet.quests })?;
    crate::packet_versions::quest_renewal::register_quest_renewal_packets(packet_handler)?;
    packet_handler.register(|packet: VisualEffectPacket| {
        let VisualEffectPacket { entity_id, effect } = packet;

        let effect_path = match effect {
            VisualEffect::BaseLevelUp => "angel.str",
            VisualEffect::JobLevelUp => "joblvup.str",
            VisualEffect::RefineFailure => "bs_refinefailed.str",
            VisualEffect::RefineSuccess => "bs_refinesuccess.str",
            VisualEffect::GameOver => "help_angel\\help_angel\\help_angel.str",
            VisualEffect::PharmacySuccess => "p_success.str",
            VisualEffect::PharmacyFailure => "p_failed.str",
            VisualEffect::BaseLevelUpSuperNovice => "help_angel\\help_angel\\help_angel.str",
            VisualEffect::JobLevelUpSuperNovice => "help_angel\\help_angel\\help_angel.str",
            VisualEffect::BaseLevelUpTaekwon => "help_angel\\help_angel\\help_angel.str",
        };

        NetworkEvent::VisualEffect { effect_path, entity_id }
    })?;
    packet_handler.register(|packet: DisplayGainedExperiencePacket| NetworkEvent::GainedExperience {
        account_id: packet.account_id,
        amount: packet.amount,
        experience_type: packet.experience_type,
        experience_source: packet.experience_source,
    })?;
    crate::packet_versions::npc_dialog::register_npc_dialog_packets_20250716(packet_handler)?;
    packet_handler.register_noop::<StateChangePacket>()?;

    packet_handler.register(|packet: QuestEffectPacket| match packet.effect {
        QuestEffect::None => NetworkEvent::RemoveQuestEffect {
            entity_id: packet.entity_id,
        },
        _ => NetworkEvent::AddQuestEffect { quest_effect: packet },
    })?;
    packet_handler.register(|packet: ItemPickupPacket| {
        let forge_data = packet.forge_data.clone();
        let packet: ragnarok_packets::ItemPickupPacket = packet.into();
        let ragnarok_packets::ItemPickupPacket {
            index,
            quantity,
            item_id,
            is_identified,
            is_broken,
            cards,
            equip_position,
            item_type,
            result,
            hire_expiration_date,
            bind_on_equip_type,
            option_data,
            favorite,
            look,
            refinement_level,
            enchantment_level,
        } = packet;

        if result != ItemPickupResult::Success {
            return vec![NetworkEvent::ChatMessage {
                text: "Failed to pick up item.".to_string(),
                color: MessageColor::Error,
            }];
        }

        // TODO: Not sure where to store these, since the *InventoryItem packets are not
        // sending these either. We will certainly use them at some point though.
        let _ = (favorite, look);

        let details = match equip_position.is_empty() {
            true => InventoryItemDetails::Regular {
                amount: quantity,
                equipped_position: equip_position,
                flags: {
                    let mut flags = RegularItemFlags::empty();
                    flags.set(RegularItemFlags::IDENTIFIED, is_identified != 0);
                    flags
                },
            },
            false => InventoryItemDetails::Equippable {
                equip_position,
                equipped_position: EquipPosition::empty(),
                bind_on_equip_type,
                w_item_sprite_number: 0,
                option_count: option_data.len() as u8,
                option_data,
                forge_data: Some(forge_data),
                refinement_level,
                enchantment_level,
                flags: {
                    let mut flags = EquippableItemFlags::empty();
                    flags.set(EquippableItemFlags::IDENTIFIED, is_identified != 0);
                    flags.set(EquippableItemFlags::IS_BROKEN, is_broken != 0);
                    flags
                },
            },
        };

        let item = InventoryItem {
            metadata: NoMetadata,
            index,
            item_id,
            item_type,
            slot: cards,
            hire_expiration_date,
            details,
        };

        let is_identified = is_identified != 0;

        vec![NetworkEvent::IventoryItemAdded { item }, NetworkEvent::ItemObtained {
            item_id,
            quantity,
            is_identified,
        }]
    })?;
    packet_handler.register(|packet: AddItemToCartPacket| {
        let amount = u16::try_from(packet.amount).unwrap_or(u16::MAX);
        let stackable = matches!(packet.item_type, 0 | 2 | 3 | 6 | 10 | 11 | 18);
        let details = match stackable {
            true => InventoryItemDetails::Regular {
                amount,
                equipped_position: EquipPosition::empty(),
                flags: {
                    let mut flags = RegularItemFlags::empty();
                    flags.set(RegularItemFlags::IDENTIFIED, packet.identified != 0);
                    flags
                },
            },
            false => InventoryItemDetails::Equippable {
                equip_position: EquipPosition::empty(),
                equipped_position: EquipPosition::empty(),
                bind_on_equip_type: 0,
                w_item_sprite_number: 0,
                option_count: packet.option_data.iter().filter(|option| option.index != 0).count() as u8,
                option_data: packet.option_data,
                forge_data: None,
                refinement_level: packet.refine,
                enchantment_level: 0,
                flags: {
                    let mut flags = EquippableItemFlags::empty();
                    flags.set(EquippableItemFlags::IDENTIFIED, packet.identified != 0);
                    flags.set(EquippableItemFlags::IS_BROKEN, packet.damaged != 0);
                    flags
                },
            },
        };

        NetworkEvent::CartItemAdded {
            item: InventoryItem {
                metadata: NoMetadata,
                index: packet.index,
                item_id: packet.item_id,
                item_type: packet.item_type,
                slot: packet.cards,
                hire_expiration_date: 0,
                details,
            },
        }
    })?;
    packet_handler.register(|packet: DeleteItemFromCartPacket| NetworkEvent::CartItemRemoved {
        index: packet.index,
        amount: u16::try_from(packet.amount).unwrap_or(u16::MAX),
    })?;
    packet_handler.register(|packet: RemoveItemFromInventoryPacket| NetworkEvent::InventoryItemRemoved {
        reason: packet.remove_reason,
        index: packet.index,
        amount: packet.amount,
    })?;
    crate::packet_versions::item_use::register_item_use_packets(packet_handler)?;
    packet_handler.register(|packet: ServerTickPacket| NetworkEvent::UpdateClientTick {
        client_tick: packet.client_tick,
        received_at: Instant::now(),
    })?;
    packet_handler.register(|packet: RequestPlayerDetailsSuccessPacket| NetworkEvent::UpdateEntityDetails {
        entity_id: EntityId(packet.character_id.0),
        name: packet.name,
    })?;
    packet_handler.register(|packet: RequestEntityDetailsSuccessPacket| NetworkEvent::UpdateEntityDetails {
        entity_id: packet.entity_id,
        name: packet.name,
    })?;
    packet_handler.register(|packet: UpdateEntityHealthPointsPacket| {
        let UpdateEntityHealthPointsPacket {
            entity_id,
            health_points,
            maximum_health_points,
        } = packet;

        NetworkEvent::UpdateEntityHealth {
            entity_id,
            health_points: health_points as usize,
            maximum_health_points: maximum_health_points as usize,
        }
    })?;
    packet_handler.register(|packet: RequestPlayerAttackFailedPacket| {
        let RequestPlayerAttackFailedPacket {
            target_entity_id,
            target_position,
            player_position,
            attack_range,
        } = packet;

        NetworkEvent::AttackFailed {
            target_entity_id,
            target_position,
            player_position,
            attack_range,
        }
    })?;
    packet_handler.register(|packet: DamagePacket1| match packet.damage_type {
        DamageType::Damage => Some(NetworkEvent::DamageEffect {
            source_entity_id: packet.source_entity_id,
            destination_entity_id: packet.destination_entity_id,
            damage_amount: (packet.damage_amount > 0).then_some(packet.damage_amount as usize),
            attack_duration: packet.attack_duration,
            is_critical: false,
        }),
        DamageType::CriticalHit => Some(NetworkEvent::DamageEffect {
            source_entity_id: packet.source_entity_id,
            destination_entity_id: packet.destination_entity_id,
            damage_amount: (packet.damage_amount > 0).then_some(packet.damage_amount as usize),
            attack_duration: packet.attack_duration,
            is_critical: true,
        }),
        DamageType::PickUpItem => Some(NetworkEvent::EntityPickUpItem {
            entity_id: packet.source_entity_id,
            item_entity_id: packet.destination_entity_id,
        }),
        DamageType::StandUp => Some(NetworkEvent::PlayerStandUp {
            entity_id: packet.destination_entity_id,
        }),
        _ => None,
    })?;
    packet_handler.register(|packet: DamagePacket3| match packet.damage_type {
        DamageType::Damage => Some(NetworkEvent::DamageEffect {
            source_entity_id: packet.source_entity_id,
            destination_entity_id: packet.destination_entity_id,
            damage_amount: (packet.damage_amount > 0).then_some(packet.damage_amount as usize),
            attack_duration: packet.attack_duration,
            is_critical: false,
        }),
        DamageType::CriticalHit => Some(NetworkEvent::DamageEffect {
            source_entity_id: packet.source_entity_id,
            destination_entity_id: packet.destination_entity_id,
            damage_amount: (packet.damage_amount > 0).then_some(packet.damage_amount as usize),
            attack_duration: packet.attack_duration,
            is_critical: true,
        }),
        DamageType::PickUpItem => Some(NetworkEvent::EntityPickUpItem {
            entity_id: packet.source_entity_id,
            item_entity_id: packet.destination_entity_id,
        }),
        DamageType::StandUp => Some(NetworkEvent::PlayerStandUp {
            entity_id: packet.destination_entity_id,
        }),
        _ => None,
    })?;
    packet_handler.register(|packet: NpcDialogPacket| {
        let NpcDialogPacket { npc_id, text } = packet;

        NetworkEvent::OpenDialog { text, npc_id }
    })?;
    packet_handler.register(|packet: RequestEquipItemStatusPacket| match packet.result {
        RequestEquipItemStatus::Success => Some(NetworkEvent::UpdateEquippedPosition {
            index: packet.inventory_index,
            equipped_position: packet.equipped_position,
        }),
        _ => None,
    })?;
    packet_handler.register(|packet: RequestUnequipItemStatusPacket| match packet.result {
        RequestUnequipItemStatus::Success => Some(NetworkEvent::UpdateEquippedPosition {
            index: packet.inventory_index,
            equipped_position: EquipPosition::NONE,
        }),
        _ => None,
    })?;
    packet_handler.register_noop::<Packet8302>()?;
    packet_handler.register_noop::<Packet0b18>()?;
    packet_handler.register_noop::<ConnectionRefusedPacket>()?;
    packet_handler.register(|packet: MapServerLoginSuccessPacket| NetworkEvent::UpdateClientTick {
        client_tick: packet.client_tick,
        received_at: Instant::now(),
    })?;
    packet_handler.register(|packet: RestartResponsePacket| match packet.result {
        RestartResponseStatus::Ok => NetworkEvent::LoggedOut,
        RestartResponseStatus::Nothing => NetworkEvent::ChatMessage {
            text: "Failed to log out.".to_string(),
            color: MessageColor::Error,
        },
    })?;
    packet_handler.register(|packet: DisconnectResponsePacket| match packet.result {
        DisconnectResponseStatus::Ok => NetworkEvent::LoggedOut,
        DisconnectResponseStatus::Wait10Seconds => NetworkEvent::ChatMessage {
            text: "Please wait 10 seconds before trying to log out.".to_string(),
            color: MessageColor::Error,
        },
    })?;
    packet_handler.register(|packet: SkillCastCancelPacket| NetworkEvent::SkillCastCancelled {
        entity_id: packet.entity_id,
    })?;
    // 20250716 MAIN emits 0x0B1A. The pre-20181212 0x07FB ack is ignored here.
    packet_handler.register_noop::<UseSkillSuccessPacket>()?;
    packet_handler.register(|packet: UseSkillAck3Packet| NetworkEvent::UseSkillSuccess {
        source_entity_id: packet.source_entity,
        destination_entity_id: packet.destination_entity,
        position: packet.position,
        skill_id: packet.skill_id,
        element: packet.element,
        delay_time: packet.delay_time,
        disposable: packet.disposable,
    })?;
    packet_handler.register(|packet: ToUseSkillSuccessPacket| NetworkEvent::ToUseSkillSuccess {
        skill_id: packet.skill_id,
        btype: packet.btype,
        item_id: packet.item_id,
        flag: packet.flag,
        cause: packet.cause,
    })?;
    packet_handler.register(|packet: NotifySkillUnitPacket| {
        let NotifySkillUnitPacket {
            entity_id,
            position,
            unit_id,
            ..
        } = packet;

        NetworkEvent::AddSkillUnit {
            entity_id,
            unit_id,
            position,
        }
    })?;
    packet_handler.register(|packet: SkillUnitDisappearPacket| {
        let SkillUnitDisappearPacket { entity_id } = packet;
        NetworkEvent::RemoveSkillUnit { entity_id }
    })?;
    packet_handler.register(|packet: NotifyGroundSkillPacket| NetworkEvent::NotifyGroundSkill {
        skill_id: packet.skill_id,
        entity_id: packet.entity_id,
        level: packet.level,
        position: packet.position,
        start_time: packet.start_time,
    })?;
    packet_handler.register(|packet: FriendListPacket| NetworkEvent::SetFriendList {
        friend_list: packet.friend_list,
    })?;
    packet_handler.register(|packet: FriendOnlineStatusPacket| NetworkEvent::FriendOnlineStatus {
        account_id: packet.account_id,
        character_id: packet.character_id,
        online: matches!(packet.state, OnlineState::Online),
        name: packet.name,
    })?;
    packet_handler.register(|packet: FriendRequestPacket| NetworkEvent::FriendRequest {
        requestee: packet.requestee,
    })?;
    packet_handler.register(|packet: FriendRequestResultPacket| {
        let text = match packet.result {
            FriendRequestResult::Accepted => format!("You have become friends with {}.", packet.friend.name),
            FriendRequestResult::Rejected => format!("{} does not want to be friends with you.", packet.friend.name),
            FriendRequestResult::OwnFriendListFull => "Your Friend List is full.".to_owned(),
            FriendRequestResult::OtherFriendListFull => format!("{}'s Friend List is full.", packet.friend.name),
        };

        let mut events = vec![NetworkEvent::ChatMessage {
            text,
            color: MessageColor::Information,
        }];

        if matches!(packet.result, FriendRequestResult::Accepted) {
            events.push(NetworkEvent::FriendAdded { friend: packet.friend });
        }

        events
    })?;
    packet_handler.register(|packet: NotifyFriendRemovedPacket| NetworkEvent::FriendRemoved {
        account_id: packet.account_id,
        character_id: packet.character_id,
    })?;
    packet_handler.register(|packet: StatusChangeSequencePacket| NetworkEvent::StatusChangeSequence {
        status_index: packet.index,
        entity_id: EntityId(packet.id),
        state: crate::event::StatusChangeState::from_wire(packet.state),
    })?;
    packet_handler.register(|packet: PartyInvitePacket| NetworkEvent::PartyInvite {
        party_id: packet.party_id,
        party_name: packet.party_name,
    })?;
    packet_handler.register(|packet: GuildAffiliationPacket| NetworkEvent::GuildAffiliation {
        guild_id: packet.guild_id,
        name: packet.guild_name,
        is_master: packet.is_master != 0,
        master_character_id: packet.master_character_id,
        emblem_version: packet.emblem_version,
    })?;
    packet_handler.register(|packet: GuildInfoPacket| NetworkEvent::GuildInfo {
        guild_id: packet.guild_id,
        level: packet.level,
        online_members: packet.online_members,
        max_members: packet.max_members,
        average_level: packet.average_level,
        experience: packet.experience,
        next_experience: packet.next_experience,
        tax_points: packet.tax_points,
        honor: packet.honor,
        virtue: packet.virtue,
        emblem_version: packet.emblem_version,
        name: packet.name,
        master_name: packet.master_name,
        territory: packet.territory,
        master_character_id: packet.master_character_id,
    })?;
    packet_handler.register(|packet: GuildMemberListPacket| NetworkEvent::GuildMembers {
        members: packet
            .members
            .into_iter()
            .map(|member| GuildMemberSnapshot {
                account_id: member.account_id,
                character_id: member.character_id,
                name: member.name,
                job: member.job,
                level: member.level,
                position_id: member.position_id,
                online: member.current_state != 0,
                contribution: member.contribution_exp,
            })
            .collect(),
    })?;
    packet_handler.register(|packet: GuildPositionNamesPacket| NetworkEvent::GuildPositions {
        positions: packet
            .positions
            .into_iter()
            .map(|position| (position.position_id, position.name))
            .collect(),
    })?;
    packet_handler.register(|packet: GuildNoticePacket| NetworkEvent::GuildNotice {
        subject: packet.subject,
        notice: packet.notice,
    })?;
    packet_handler.register(|packet: GuildMemberOnlinePacket| NetworkEvent::GuildMemberOnline {
        account_id: packet.account_id,
        character_id: packet.character_id,
        online: packet.status != 0,
    })?;
    packet_handler.register(|packet: GuildMemberOnlineShortPacket| NetworkEvent::GuildMemberOnline {
        account_id: packet.account_id,
        character_id: packet.character_id,
        online: packet.status != 0,
    })?;
    packet_handler.register(|packet: GuildLeavePacket| NetworkEvent::GuildMemberRemoved {
        character_id: packet.character_id,
    })?;
    packet_handler.register(|packet: GuildExpulsionPacket| NetworkEvent::GuildMemberRemoved {
        character_id: packet.character_id,
    })?;
    packet_handler.register_noop::<ReputationPacket>()?;
    crate::packet_versions::clan::register_clan_packets(packet_handler)?;
    crate::packet_versions::party::register_party_packets(packet_handler)?;
    crate::packet_versions::guild::register_guild_action_packets(packet_handler)?;
    packet_handler.register_noop::<ChangeMapCellPacket>()?;
    packet_handler.register(open_market_event)?;
    crate::packet_versions::enchant::register_enchant_packets(packet_handler)?;
    crate::packet_versions::stable::register_stable_packets(packet_handler)?;
    crate::packet_versions::achievements::register_achievement_packets(packet_handler)?;
    crate::packet_versions::eden::register_eden_packets(packet_handler)?;
    crate::packet_versions::warpra::register_warpra_packets(packet_handler)?;
    crate::packet_versions::companions::register_companion_packets(packet_handler, true)?;
    crate::packet_versions::mvp_minimap::register_mvp_minimap_packets(packet_handler)?;
    crate::packet_versions::economy::register_player_shop_packets(packet_handler)?;
    crate::packet_versions::refine::register_refine_packets(packet_handler)?;
    crate::packet_versions::item_skill_selection::register_item_skill_selection_packets(packet_handler)?;
    crate::packet_versions::bank_cash_roulette::register_bank_cash_roulette(packet_handler)?;
    crate::packet_versions::item_workshop::register_reform_and_grade(packet_handler)?;
    crate::packet_versions::item_workshop::register_laphine(packet_handler)?;
    crate::packet_versions::styling_shop::register_style_packets(packet_handler, false)?;
    packet_handler.register(|packet: BuyOrSellPacket| NetworkEvent::AskBuyOrSell { shop_id: packet.shop_id })?;
    packet_handler.register(|packet: ShopItemListPacket| {
        let items = packet
            .items
            .into_iter()
            .map(|item| ShopItem {
                metadata: NoMetadata,
                item_id: item.item_id,
                item_type: item.item_type,
                price: item.price,
                quantity: ItemQuantity::Infinite,
                weight: 0,
                location: item.location,
            })
            .collect();

        NetworkEvent::OpenShop { items }
    })?;
    packet_handler.register(|packet: BuyShopItemsResultPacket| NetworkEvent::BuyingCompleted { result: packet.result })?;
    crate::packet_versions::barter::register_barter_packets(packet_handler)?;
    packet_handler.register_noop::<ParameterChangePacket>()?;
    packet_handler.register(|packet: SellListPacket| NetworkEvent::SellItemList { items: packet.items })?;
    packet_handler.register(|packet: SellItemsResultPacket| NetworkEvent::SellingCompleted { result: packet.result })?;
    packet_handler.register(|packet: RequestStatUpResponsePacket| NetworkEvent::StatUpResult {
        status_id: packet.staus_type,
        success: packet.success == RequestStatUpResult::Success,
        value: packet.value,
    })?;
    packet_handler.register_noop::<EquipAmmunitionPacket>()?;
    packet_handler.register_noop::<AmmunitionActionPacket>()?;
    packet_handler.register(|packet: UpdateSkillPacket| {
        let UpdateSkillPacket {
            skill_id,
            skill_level,
            spell_point_cost,
            attack_range,
            upgradable,
        } = packet;

        NetworkEvent::UpdateSkill {
            skill_id,
            skill_level,
            spell_point_cost,
            attack_range,
            upgradable: upgradable != 0,
        }
    })?;
    packet_handler.register(|packet: RemoveSkillPacket| NetworkEvent::RemoveSkill { skill_id: packet.skill_id })?;

    packet_handler.register(|packet: DisplayExpandedEmotionPacket| {
        if packet.group == 0 && packet.emotion <= u8::MAX as u16 {
            Some(NetworkEvent::DisplayEmotion {
                entity_id: packet.entity_id,
                emotion: packet.emotion as u8,
            })
        } else {
            None
        }
    })?;
    packet_handler.register(|packet: TradeRequestPacket| NetworkEvent::TradeRequested {
        partner_name: packet.partner_name,
        partner_character_id: packet.partner_character_id,
        level: packet.level,
    })?;
    packet_handler.register(|packet: TradeStartPacket| NetworkEvent::TradeOpened {
        result: packet.result,
        partner_character_id: packet.partner_character_id,
        level: packet.level,
    })?;
    packet_handler.register(|packet: TradeAddItemPacket| {
        if packet.item_id.0 == 0 {
            NetworkEvent::TradeZenyAdded { amount: packet.amount }
        } else {
            NetworkEvent::TradeItemAdded {
                item_id: packet.item_id,
                item_type: packet.item_type,
                amount: packet.amount,
                identified: packet.identified != 0,
                refinement_level: packet.refinement_level,
            }
        }
    })?;
    packet_handler.register(|packet: TradeAddItemResultPacket| NetworkEvent::TradeAddResult {
        index: packet.index,
        result: packet.result,
    })?;
    packet_handler.register(|packet: TradeConfirmPacket| NetworkEvent::TradeConfirmed {
        partner: packet.which != 0,
    })?;
    packet_handler.register(|_packet: TradeCancelPacket| NetworkEvent::TradeCancelled)?;
    packet_handler.register(|packet: TradeCompletePacket| NetworkEvent::TradeCompleted { result: packet.result })?;
    packet_handler.register(|_packet: TradeUnlockPacket| NetworkEvent::TradeLockRemoved)?;
    packet_handler.register(|packet: AddSkillPacket| {
        let p = packet.skill_information;
        NetworkEvent::UpdateSkill {
            skill_id: p.skill_id,
            skill_level: p.skill_level,
            spell_point_cost: p.spell_point_cost,
            attack_range: p.attack_range,
            upgradable: p.upgradable != 0,
        }
    })?;
    Ok(())
}
