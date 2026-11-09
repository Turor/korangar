use std::time::Instant;

use ragnarok_packets::*;

use crate::hotkey::HotkeyState;
use crate::items::ShopItem;
use crate::{
    CharacterServerLoginData, EntityData, InventoryItem, LoginServerLoginData, MessageColor, NoMetadata,
    UnifiedCharacterSelectionFailedReason, UnifiedLoginFailedReason,
};

/// An event triggered by one of the Ragnarok Online servers.
#[derive(Debug)]
pub enum NetworkEvent {
    LoginServerConnected {
        character_servers: Vec<CharacterServerInformation>,
        login_data: LoginServerLoginData,
    },
    LoginServerConnectionFailed {
        reason: UnifiedLoginFailedReason,
        message: String,
    },
    LoginServerDisconnected {
        reason: DisconnectReason,
    },
    CharacterServerConnected {
        normal_slot_count: usize,
    },
    CharacterServerConnectionFailed {
        reason: LoginFailedReason,
        message: &'static str,
    },
    CharacterServerDisconnected {
        reason: DisconnectReason,
    },
    AccountId {
        account_id: AccountId,
    },
    CharacterList {
        characters: Vec<CharacterInformation>,
    },
    CharacterSelected {
        login_data: CharacterServerLoginData,
    },
    CharacterSelectionFailed {
        reason: UnifiedCharacterSelectionFailedReason,
        message: &'static str,
    },
    CharacterCreated {
        character_information: CharacterInformation,
    },
    CharacterCreationFailed {
        reason: CharacterCreationFailedReason,
        message: &'static str,
    },
    CharacterDeleted,
    CharacterDeletionFailed {
        reason: CharacterDeletionFailedReason,
        message: &'static str,
    },
    MapServerDisconnected {
        reason: DisconnectReason,
    },
    /// Initial player status.
    InitialStats {
        strength_stat_points_cost: u8,
        agility_stat_points_cost: u8,
        vitality_stat_points_cost: u8,
        intelligence_stat_points_cost: u8,
        dexterity_stat_points_cost: u8,
        luck_stat_points_cost: u8,
    },
    /// Resurrect a player.
    ResurrectPlayer {
        entity_id: EntityId,
    },
    /// Make a player stand up.
    PlayerStandUp {
        entity_id: EntityId,
    },
    /// Show an emotion above an entity.
    DisplayEmotion {
        entity_id: EntityId,
        emotion: u8,
    },
    /// Add an entity to the list of entities that the client is aware of.
    AddEntity {
        entity_data: EntityData,
    },
    /// Remove an entity from the list of entities that the client is aware of
    /// by its id.
    RemoveEntity {
        entity_id: EntityId,
        reason: DisappearanceReason,
    },
    /// Add an item to the ground.
    AddGroundItem {
        entity_id: EntityId,
        item_id: ItemId,
        is_identified: bool,
        quantity: u16,
        position: TilePosition,
        x_offset: u8,
        y_offset: u8,
    },
    /// Remove an item from the ground.
    RemoveGroundItem {
        entity_id: EntityId,
    },
    /// The player is pathing to a new position.
    PlayerMove {
        origin: WorldPosition,
        destination: WorldPosition,
        starting_timestamp: ClientTick,
    },
    /// An Entity nearby is pathing to a new position.
    EntityMove {
        entity_id: EntityId,
        origin: WorldPosition,
        destination: WorldPosition,
        starting_timestamp: ClientTick,
    },
    /// Player was moved to a new position on a different map or the current map
    ChangeMap {
        map_name: String,
        position: TilePosition,
    },
    /// Update the client side to keep server and client synchronized.
    UpdateClientTick {
        client_tick: ClientTick,
        received_at: Instant,
    },
    /// New chat message for the client.
    ChatMessage {
        text: String,
        color: MessageColor,
    },
    CharacterSlotSwitched,
    CharacterSlotSwitchFailed,
    /// Update entity details. Mostly received when the client sends
    /// [RequestDetailsPacket] after the player hovered an entity.
    UpdateEntityDetails {
        entity_id: EntityId,
        name: String,
    },
    UpdateEntityHealth {
        entity_id: EntityId,
        health_points: usize,
        maximum_health_points: usize,
    },
    DamageEffect {
        source_entity_id: EntityId,
        destination_entity_id: EntityId,
        /// Damage amount. [`None`] on miss, [`Some`] otherwise.
        damage_amount: Option<usize>,
        attack_duration: u32,
        is_critical: bool,
    },
    EntityPickUpItem {
        entity_id: EntityId,
        item_entity_id: EntityId,
    },
    HealEffect {
        entity_id: EntityId,
        heal_amount: usize,
    },
    UpdateStat {
        stat_type: StatType,
    },
    OpenDialog {
        text: String,
        npc_id: EntityId,
    },
    AddNextButton {
        npc_id: EntityId,
    },
    AddCloseButton {
        npc_id: EntityId,
    },
    AddChoiceButtons {
        choices: Vec<String>,
        npc_id: EntityId,
    },
    AddQuestEffect {
        quest_effect: QuestEffectPacket,
    },
    RemoveQuestEffect {
        entity_id: EntityId,
    },
    SetInventory {
        items: Vec<InventoryItem<NoMetadata>>,
    },
    /// Kafra storage (context 2) or guild storage (context 3) snapshot.
    SetStorage {
        items: Vec<InventoryItem<NoMetadata>>,
    },
    /// Full cart snapshot. Distinct from [`Self::SetInventory`].
    SetCart {
        items: Vec<InventoryItem<NoMetadata>>,
    },
    CartItemAdded {
        item: InventoryItem<NoMetadata>,
    },
    CartItemRemoved {
        index: InventoryIndex,
        amount: u16,
    },
    IventoryItemAdded {
        item: InventoryItem<NoMetadata>,
    },
    ItemObtained {
        item_id: ItemId,
        quantity: u16,
        is_identified: bool,
    },
    SkillTree {
        skill_information: Vec<SkillInformation>,
    },
    UpdateEquippedPosition {
        index: InventoryIndex,
        equipped_position: EquipPosition,
    },
    /// Full alternate equipment set from `EquippableSwitchItemList` (`0x0A9B`).
    SetEquipSwitch {
        entries: Vec<EquippableSwitchItemInformation>,
    },
    /// One switch-set assignment changed. An empty position removes the item.
    UpdateEquipSwitch {
        index: InventoryIndex,
        position: EquipPosition,
    },
    /// Result of requesting a full equipment switch (`0x0A9D`).
    EquipSwitchReply {
        failed: bool,
    },
    ChangeJob {
        account_id: AccountId,
        job_id: JobId,
    },
    ChangeHair {
        account_id: AccountId,
        hair_id: u32,
    },
    LoggedOut,
    FriendRequest {
        requestee: Friend,
    },
    VisualEffect {
        effect_path: &'static str,
        entity_id: EntityId,
    },
    AddSkillUnit {
        entity_id: EntityId,
        unit_id: UnitId,
        position: TilePosition,
    },
    RemoveSkillUnit {
        entity_id: EntityId,
    },
    SetFriendList {
        friend_list: Vec<Friend>,
    },
    FriendAdded {
        friend: Friend,
    },
    FriendRemoved {
        account_id: AccountId,
        character_id: CharacterId,
    },
    /// Friend came online or went offline. `online` is true for
    /// [`OnlineState::Online`].
    FriendOnlineStatus {
        account_id: AccountId,
        character_id: CharacterId,
        online: bool,
        name: String,
    },
    /// Another player invited this client into a party.
    PartyInvite {
        party_id: PartyId,
        party_name: String,
    },
    /// Whether this client may receive party invitations (`0x02C9`).
    PartyInvitationState {
        invitations_allowed: bool,
    },
    SetHotkeyData {
        tab: HotbarTab,
        hotkeys: Vec<HotkeyState>,
    },
    OpenShop {
        items: Vec<ShopItem<NoMetadata>>,
    },
    AskBuyOrSell {
        shop_id: ShopId,
    },
    BuyingCompleted {
        result: BuyShopItemsResult,
    },
    SellItemList {
        items: Vec<SellItemInformation>,
    },
    SellingCompleted {
        result: SellItemsResult,
    },
    /// Another player asked to trade (`ZC_REQ_EXCHANGE_ITEM2`).
    TradeRequested {
        partner_name: String,
        partner_character_id: CharacterId,
        level: i16,
    },
    /// The server accepted or rejected opening a trade
    /// (`ZC_ACK_EXCHANGE_ITEM2`). Result `3` means the trade window should
    /// open.
    TradeOpened {
        result: u8,
        partner_character_id: CharacterId,
        level: i16,
    },
    /// The partner placed an item on their side (`ZC_ADD_EXCHANGE_ITEM`).
    TradeItemAdded {
        item_id: ItemId,
        item_type: u8,
        amount: i32,
        identified: bool,
        refinement_level: u8,
    },
    /// The partner added zeny. Item id zero on `ZC_ADD_EXCHANGE_ITEM`.
    TradeZenyAdded {
        amount: i32,
    },
    /// This client's add-item was accepted or rejected
    /// (`ZC_ACK_ADD_EXCHANGE_ITEM`). Zeny is not acked. Index 0 is the dummy
    /// success `trade_tradeok` sends and must not be booked as an offer.
    TradeAddResult {
        /// Client inventory index. Zero is the confirm dummy, not zeny.
        index: u16,
        result: u8,
    },
    /// One side locked the offer (`ZC_CONCLUDE_EXCHANGE_ITEM`).
    /// `partner` is false when this client locked.
    TradeConfirmed {
        partner: bool,
    },
    /// The trade was cancelled (`ZC_CANCEL_EXCHANGE_ITEM`).
    TradeCancelled,
    /// The trade finished (`ZC_EXEC_EXCHANGE_ITEM`). Result 0 is success.
    TradeCompleted {
        result: u8,
    },
    /// Locks were cleared because an offer changed (`ZC_EXCHANGEITEM_UNDO`).
    TradeLockRemoved,
    InventoryItemRemoved {
        reason: RemoveItemReason,
        index: InventoryIndex,
        amount: u16,
    },
    AttackFailed {
        target_entity_id: EntityId,
        target_position: TilePosition,
        player_position: TilePosition,
        attack_range: AttackRange,
    },
    UpdateSkill {
        skill_id: SkillId,
        skill_level: SkillLevel,
        spell_point_cost: u16,
        attack_range: AttackRange,
        upgradable: bool,
    },
    /// Delete a skill from the skill tree.
    RemoveSkill {
        skill_id: SkillId,
    },
    /// Remaining cooldown for a skill (`DisplaySkillCooldownPacket`, 0x043D).
    SkillCooldown {
        skill_id: SkillId,
        until: ClientTick,
    },
    /// A skill cast was acknowledged (`UseSkillSuccessPacket`, 0x07FB).
    UseSkillSuccess {
        source_entity_id: EntityId,
        destination_entity_id: EntityId,
        position: TilePosition,
        skill_id: SkillId,
        element: u32,
        delay_time: u32,
        disposable: u8,
    },
    /// The server accepted a skill the client is about to use
    /// (`ToUseSkillSuccessPacket`, 0x0110).
    ToUseSkillSuccess {
        skill_id: SkillId,
        btype: i32,
        item_id: ItemId,
        flag: u8,
        cause: u8,
    },
    /// A ground skill was placed (`NotifyGroundSkillPacket`, 0x0117).
    NotifyGroundSkill {
        skill_id: SkillId,
        entity_id: EntityId,
        level: SkillLevel,
        position: TilePosition,
        start_time: ClientTick,
    },
    /// Timed status effect on an entity (`StatusChangePacket`, header
    /// `0x0983`).
    StatusChange {
        status_index: u16,
        entity_id: EntityId,
        state: StatusChangeState,
        total_duration_milliseconds: u32,
        remaining_milliseconds: u32,
        values: [u32; 3],
    },
    /// Status on/off without durations (`StatusChangeSequencePacket`, header
    /// `0x0196`).
    StatusChangeSequence {
        status_index: u16,
        entity_id: EntityId,
        state: StatusChangeState,
    },
    /// Full quest list (`ZC_ALL_QUEST_LIST`, 0x09F8). No journal UI consumes
    /// this yet.
    QuestList {
        quests: Vec<Quest>,
    },
    /// One quest was added or its definition changed (`ZC_ADD_QUEST`, 0x09F9).
    /// Only the first `objective_count` objective slots are included; the
    /// packet pads to three.
    QuestNotification {
        quest_id: u32,
        active: bool,
        start_time: u32,
        expire_time: u32,
        objectives: Vec<ObjectiveDetails1>,
    },
    /// Hunting objectives for the current quests (0x08FE).
    HuntingQuestNotification {
        objectives: Vec<HuntingObjective>,
    },
    /// Kill-count progress for hunting objectives (0x09FA).
    HuntingQuestObjectiveUpdate {
        objectives: Vec<HuntingObjective>,
    },
    /// A quest was removed from the character (0x02B4).
    QuestRemoved {
        quest_id: u32,
    },
    /// Entity effect (`DisplaySpecialEffectPacket`, 0x01F3).
    SpecialEffect {
        entity_id: EntityId,
        effect_id: EffectId,
    },
    /// Skill hit that also carries damage (`DisplaySkillEffectAndDamagePacket`,
    /// 0x01DE).
    SkillEffectAndDamage {
        skill_id: SkillId,
        source_entity_id: EntityId,
        destination_entity_id: EntityId,
        start_time: ClientTick,
        source_delay: u32,
        destination_delay: u32,
        damage: u32,
        level: SkillLevel,
        hit_count: u16,
        skill_type: u8,
    },
    /// The local player recovered HP or SP (`DisplayPlayerHealEffect`, 0x0A27).
    PlayerHealEffect {
        heal_type: HealType,
        heal_amount: u32,
    },
    /// Base or job experience (`DisplayGainedExperiencePacket`, 0x0ACC).
    GainedExperience {
        account_id: AccountId,
        amount: u64,
        experience_type: ExperienceType,
        experience_source: ExperienceSource,
    },
}

/// Wire `state` byte of the status-change packets.
///
/// `0` ends the status. `1` starts or refreshes it. `2` is the extra non-zero
/// flag some servers send for a status that stays active. Any other value is
/// kept so it is not dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusChangeState {
    Ended,
    Active,
    StillActive,
    Other(u8),
}

impl StatusChangeState {
    pub(crate) fn from_wire(state: u8) -> Self {
        match state {
            0 => Self::Ended,
            1 => Self::Active,
            2 => Self::StillActive,
            other => Self::Other(other),
        }
    }
}

/// New-type so we can implement some `From` traits. This will help when
/// registering the packet handlers.
#[derive(Default)]
pub(crate) struct NetworkEventList(pub Vec<NetworkEvent>);

pub(crate) struct NoNetworkEvents;

impl From<NetworkEvent> for NetworkEventList {
    fn from(event: NetworkEvent) -> Self {
        Self(vec![event])
    }
}

impl From<Vec<NetworkEvent>> for NetworkEventList {
    fn from(events: Vec<NetworkEvent>) -> Self {
        Self(events)
    }
}

impl From<Option<NetworkEvent>> for NetworkEventList {
    fn from(event: Option<NetworkEvent>) -> Self {
        match event {
            Some(event) => Self(vec![event]),
            None => Self(Vec::new()),
        }
    }
}

impl From<NoNetworkEvents> for NetworkEventList {
    fn from(_: NoNetworkEvents) -> Self {
        Self(Vec::new())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisconnectReason {
    ClosedByClient,
    ConnectionError,
}

pub(crate) trait DisconnectedEvent {
    fn create_event(reason: DisconnectReason) -> NetworkEvent;
}

pub(crate) struct LoginServerDisconnectedEvent;
pub(crate) struct CharacterServerDisconnectedEvent;
pub(crate) struct MapServerDisconnectedEvent;

impl DisconnectedEvent for LoginServerDisconnectedEvent {
    fn create_event(reason: DisconnectReason) -> NetworkEvent {
        NetworkEvent::LoginServerDisconnected { reason }
    }
}

impl DisconnectedEvent for CharacterServerDisconnectedEvent {
    fn create_event(reason: DisconnectReason) -> NetworkEvent {
        NetworkEvent::CharacterServerDisconnected { reason }
    }
}

impl DisconnectedEvent for MapServerDisconnectedEvent {
    fn create_event(reason: DisconnectReason) -> NetworkEvent {
        NetworkEvent::MapServerDisconnected { reason }
    }
}
