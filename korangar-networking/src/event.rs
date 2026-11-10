use std::time::Instant;

use ragnarok_packets::*;

use crate::hotkey::HotkeyState;
use crate::items::ShopItem;
use crate::{
    CharacterServerLoginData, EntityData, InventoryItem, LoginServerLoginData, MessageColor, NoMetadata, PincodeNotice,
    UnifiedCharacterSelectionFailedReason, UnifiedLoginFailedReason,
};

/// Blessing catalyst on a grade-enchant material list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GradeBlessingOffer {
    pub item_id: i32,
    pub amount: i32,
    pub max_blessing: i32,
    pub bonus: i32,
}

/// One option row on a grade-enchant material list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GradeMaterialOffer {
    pub item_id: ItemId,
    pub amount: i32,
    pub price: i32,
    pub downgrade: bool,
    pub breakable: bool,
}

/// One ore the refine UI can spend (`ZC_REFINING_MATERIAL_LIST`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefineMaterialOffer {
    pub item_id: ItemId,
    /// Success chance in percent.
    pub chance_percent: u8,
    pub zeny: u32,
}

/// One guild member row from `ZC_MEMBERMGR_INFO`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildMemberSnapshot {
    pub account_id: AccountId,
    pub character_id: CharacterId,
    pub name: String,
    pub job: i16,
    pub level: i16,
    pub position_id: i32,
    pub online: bool,
    pub contribution: i32,
}

/// One row from a shop-search result (`0x0B64`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchStoreHit {
    pub store_id: u32,
    pub account_id: AccountId,
    pub shop_name: String,
    pub item_id: ItemId,
    pub price: Price,
    pub amount: u16,
}

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
    /// Second password (`0x08B9`, `0x0AE9`, `0x08BB`, `0x08BF`).
    Pincode {
        notice: PincodeNotice,
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
    /// An entity stopped walking (`ZC_STOPMOVE` / `0x0088`). The position is
    /// the cell the server accepted, which can differ from the requested
    /// destination when the step was rejected.
    EntityStopMove {
        entity_id: EntityId,
        position: TilePosition,
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
    /// `ZC_UPDATE_GDID`. `guild_id == 0` means this character is not in a
    /// guild.
    GuildAffiliation {
        guild_id: u32,
        name: String,
        is_master: bool,
        master_character_id: u32,
    },
    /// `ZC_GUILD_INFO` (`0x0B7B`).
    GuildInfo {
        guild_id: u32,
        level: i32,
        online_members: i32,
        max_members: i32,
        average_level: i32,
        experience: i32,
        next_experience: i32,
        emblem_version: i32,
        name: String,
        master_name: String,
        territory: String,
        master_character_id: u32,
    },
    /// `ZC_MEMBERMGR_INFO` (`0x0B7D`).
    GuildMembers {
        members: Vec<GuildMemberSnapshot>,
    },
    /// `ZC_POSITION_ID_NAME_INFO` (`0x0166`).
    GuildPositions {
        positions: Vec<(i32, String)>,
    },
    /// `ZC_GUILD_NOTICE` (`0x016F`).
    GuildNotice {
        subject: String,
        notice: String,
    },
    /// Member came online or went offline (`0x01F2` / `0x016D`).
    GuildMemberOnline {
        account_id: AccountId,
        character_id: CharacterId,
        online: bool,
    },
    /// Member left or was expelled (`0x0A83` / `0x0A82`).
    GuildMemberRemoved {
        character_id: CharacterId,
    },
    /// `ZC_CLANINFO` (`0x098A`).
    ClanInfo {
        clan_id: u32,
        name: String,
        master_name: String,
        map_name: String,
        alliances: Vec<String>,
        antagonists: Vec<String>,
    },
    /// `ZC_NOTIFY_CLAN_CONNECTINFO` (`0x0988`).
    ClanOnlineCount {
        online_members: u16,
        maximum_members: u16,
    },
    /// `ZC_ACK_CLAN_LEAVE` (`0x0989`). Header-only ack that this character left
    /// the clan. Membership is cleared from this packet, not from `clan_id ==
    /// 0`.
    ClanLeft,
    SetHotkeyData {
        tab: HotbarTab,
        hotkeys: Vec<HotkeyState>,
    },
    OpenShop {
        items: Vec<ShopItem<NoMetadata>>,
    },
    /// A player started a vend or buying store. `buying_store` selects which
    /// click packet opens it.
    PlayerShopSign {
        account_id: AccountId,
        shop_name: String,
        buying_store: bool,
    },
    /// A vend or buying-store sign was removed.
    PlayerShopClosed {
        account_id: AccountId,
        buying_store: bool,
    },
    /// Item list for a player vend (`buying_store == false`) or a buying store.
    /// `indexes` aligns with `items` for vends (cart index). Buying stores
    /// leave it empty.
    PlayerShopOpened {
        account_id: AccountId,
        store_id: u32,
        buying_store: bool,
        items: Vec<ShopItem<NoMetadata>>,
        indexes: Vec<u16>,
    },
    /// This client may open a vend (`0x012D`).
    VendingPrepare {
        item_count: u16,
    },
    /// The server accepted this client's vend or buying store.
    OwnPlayerShopOpened {
        item_count: usize,
        buying_store: bool,
    },
    /// Result of buying from a player vend. `result == 0` is success.
    VendingPurchaseResult {
        index: u16,
        amount: u16,
        result: u8,
    },
    /// Buying store could not open (`0x0812`). `result` is the server reason
    /// code.
    BuyingStoreOpenFailed {
        result: u16,
        total_weight: u32,
    },
    /// A sale into a buying store was rejected (`0x081A`).
    BuyingStoreTradeFailed {
        result: u16,
    },
    /// Search-store UI was enabled.
    SearchStoreOpen {
        effect: u16,
        remaining_uses: u8,
    },
    SearchStoreFailed {
        reason: u8,
    },
    SearchStoreResults {
        first_page: bool,
        next_page: bool,
        uses_remaining: u8,
        hits: Vec<SearchStoreHit>,
    },
    /// Server pointed at a searched shop on the map.
    SearchStorePosition {
        x: i16,
        y: i16,
    },
    /// `ZC_NOTIFY_UNREADMAIL` (`0x09E7`).
    NewMailStatus {
        available: bool,
    },
    /// One page of `ZC_ACK_MAIL_LIST3` (`0x0AC2`).
    ///
    /// `next_mail_id` is the last row on this page. The next `0x0AC1` uses it
    /// as the cursor. It is 0 when the page is empty.
    RodexMailList {
        is_end: bool,
        next_mail_id: i64,
        summary: String,
    },
    /// `ZC_ACK_WRITE_MAIL` (`0x09ED`).
    RodexWriteResult {
        result: u8,
        summary: String,
    },
    /// `ZC_ACK_DELETE_MAIL` (`0x09F6`).
    RodexDelete {
        open_type: u8,
        mail_id: i64,
    },
    /// `ZC_ACK_ZENY_FROM_MAIL` (`0x09F2`).
    RodexZeny {
        mail_id: i64,
        result: u8,
        summary: String,
    },
    /// `ZC_ACK_ITEM_FROM_MAIL` (`0x09F4`).
    RodexItems {
        mail_id: i64,
        result: u8,
        summary: String,
    },
    /// `ZC_ACK_READ_MAIL` (`0x09EB`, or `0x0B63` on the 2025-07-16 profile).
    RodexRead {
        mail_id: i64,
        zeny: i64,
        item_count: u8,
        summary: String,
    },
    /// Alive MVPs on the current map (`0x0EF5`). Replaces the previous list.
    MvpMinimap {
        flags: u8,
        markers: Vec<ragnarok_packets::MvpMinimapEntry>,
    },
    /// The refine UI opened (`ZC_OPEN_REFINING_UI`).
    RefineOpened,
    /// Ores and chances for the item the player selected.
    RefineMaterials {
        index: InventoryIndex,
        blacksmith_blessing: u8,
        materials: Vec<RefineMaterialOffer>,
    },
    /// A refine attempt finished (`ZC_ACK_ITEMREFINING`).
    ///
    /// `result` is 0 success, 1 destroyed, 2 downgraded, 3 unchanged.
    RefineResult {
        result: u16,
        index: InventoryIndex,
        refine_level: u16,
    },
    /// `ZC_UI_OPEN` type 8 opened the grade enchant UI.
    EnchantGradeOpened {
        data: i32,
    },
    /// Materials for the equipment the player selected. `index` is `None` when
    /// the server sent -1.
    EnchantGradeMaterials {
        index: Option<u16>,
        success_chance: i32,
        blessing: GradeBlessingOffer,
        protect_item_id: ItemId,
        protect_amount: i32,
        materials: Vec<GradeMaterialOffer>,
    },
    /// A grade attempt finished. `result` is 0 success, 1 failed, 2 downgraded,
    /// 3 destroyed, 4 protected.
    EnchantGradeResult {
        index: InventoryIndex,
        grade: i16,
        result: i32,
    },
    /// Someone's grade result was announced.
    EnchantGradeBroadcast {
        name: String,
        item_id: ItemId,
        grade: i16,
        success: bool,
    },
    /// The item reform UI opened. `item_id` is the catalyst.
    ItemReformOpened {
        item_id: ItemId,
    },
    /// Reform finished. Result 0 is success and the server closes its side.
    ItemReformResult {
        index: InventoryIndex,
        result: i8,
    },
    /// Laphine synthesis UI opened (`ZC_RANDOM_COMBINE_ITEM_UI_OPEN`).
    LaphineSynthesisOpened {
        item_id: ItemId,
    },
    /// Synthesis finished. Result 0 is success.
    LaphineSynthesisResult {
        result: i16,
    },
    /// Laphine upgrade UI opened (`ZC_RANDOM_UPGRADE_ITEM_UI_OPEN`).
    LaphineUpgradeOpened {
        item_id: ItemId,
    },
    /// Upgrade finished. Result 0 is success.
    LaphineUpgradeResult {
        result: u16,
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
    /// Full achievement list (`ZC_ALL_ACH_LIST`, 0x0A23).
    AchievementList {
        total_score: u32,
        level: u16,
        experience: u32,
        experience_to_next_level: u32,
        entries: Vec<AchievementData>,
    },
    /// One achievement changed (`ZC_ACH_UPDATE`, 0x0A24).
    AchievementUpdate {
        total_score: u32,
        level: u16,
        experience: u32,
        experience_to_next_level: u32,
        entry: AchievementData,
    },
    /// Reward request result (`ZC_ACH_REWARD_ACK`, 0x0A26).
    AchievementReward {
        failed: bool,
        achievement_id: u32,
    },
    /// Attendance period and calendar (`ZC_ATTENDANCE_DATA`, 0x0AE3).
    Attendance {
        start_time: u32,
        end_time: u32,
        count: u32,
        rewards: Vec<AttendanceReward>,
    },
    /// Attendance claim result (`ZC_ATTENDANCE_RESPONSE`, 0x0AF0).
    /// `result` is the new day counter on rAthena.
    AttendanceClaim {
        result: u32,
    },
    /// Navigation destination (`ZC_NAVIGATION_ACTIVE`, `0x08E2`).
    /// `show_window` is the inverse of the packet's hide flag.
    NavigateTo {
        target_type: u8,
        service_flags: u8,
        show_window: bool,
        map_name: String,
        x: u16,
        y: u16,
        monster_id: u16,
    },
    /// Minimap viewpoint (`ZC_MARK_POSITION`, `0x0144`).
    /// `kind` matches [`ragnarok_packets::MarkerType`]: 0 for 15 seconds, 1
    /// until the map changes, 2 removes `slot`. Color channels are script RGB.
    MinimapMark {
        npc_id: EntityId,
        kind: u8,
        x: u32,
        y: u32,
        slot: u8,
        red: u8,
        green: u8,
        blue: u8,
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
    /// Enchant UI opened (`ZC_UI_OPEN2` / `0x0B9A`, type
    /// [`ragnarok_packets::ENCHANT_UI_TYPE`]). `enchant_group` is the full
    /// 64-bit id from the server.
    OpenEnchant {
        enchant_group: i64,
    },
    /// Enchant attempt result (`ZC_RESPONSE_ENCHANT`, `0x0B9F`).
    EnchantResult {
        message_id: i32,
        item_id: ItemId,
    },
    /// Turoran stable homunculus list (`0x0EF8`).
    StableHomunculusList {
        active_id: i32,
        summary: String,
    },
    /// Turoran stable pet list (`0x0EF9`).
    StablePetList {
        active_id: i32,
        summary: String,
    },
    /// Turoran stable evolution materials (`0x0EF6`).
    StableOwnedList {
        summary: String,
    },
    /// Turoran stable result (`0x0EFA`).
    StableResult {
        result: i16,
        id: i32,
        summary: String,
    },
    /// Turoran Eden quest list (`0x0EFB`).
    EdenQuestList {
        base_level: i16,
        merit: i32,
        summary: String,
    },
    /// Turoran Eden result (`0x0EFC`).
    EdenResult {
        result: i16,
        quest_id: i32,
        summary: String,
    },
    /// Turoran Eden shop (`0x0EFD`).
    EdenShop {
        merit: i32,
        summary: String,
    },
    /// Turoran Warpra destination chunk (`0x0EFE`).
    WarpraList {
        zeny: i32,
        price: i32,
        flags: u8,
        total: u16,
        destinations: Vec<ragnarok_packets::WarpraDestinationEntry>,
    },
    /// Turoran Warpra result (`0x0EFF`).
    WarpraResult {
        result: i16,
        kind: i16,
        location_id: i16,
        summary: String,
    },
    /// Owned pet (`ZC_PROPERTY_PET`, `0x01A2`).
    PetInfo {
        summary: String,
        name: String,
        renamed: bool,
        level: i16,
        hunger: i16,
        intimacy: i16,
        accessory_id: i16,
        class_id: i16,
    },
    /// One pet stat (`ZC_CHANGESTATE_PET`, `0x01A4`).
    PetState {
        kind: i8,
        entity_id: u32,
        value: i32,
    },
    /// Homunculus property (`ZC_PROPERTY_HOMUN`, `0x0BA4`).
    HomunculusInfo {
        summary: String,
        name: String,
        flags: u8,
        level: u16,
        hunger: u16,
        intimacy: u16,
        attack: u16,
        magic_attack: u16,
        hit: u16,
        critical: u16,
        defense: u16,
        magic_defense: u16,
        flee: u16,
        attack_motion: u16,
        health_points: u32,
        maximum_health_points: u32,
        spell_points: u32,
        maximum_spell_points: u32,
        experience: i64,
        next_experience: i64,
        skill_points: u16,
        attack_range: u16,
    },
    /// Mercenary info (`ZC_MER_INIT` or `ZC_MER_PROPERTY`).
    MercenaryInfo {
        summary: String,
        account_id: u32,
        name: String,
        level: i16,
        attack: i16,
        magic_attack: i16,
        hit: i16,
        critical: i16,
        defense: i16,
        magic_defense: i16,
        flee: i16,
        attack_speed: i16,
        health_points: i32,
        maximum_health_points: i32,
        spell_points: i32,
        maximum_spell_points: i32,
        expire_date: i32,
        faith: i16,
        summons: i32,
        kills: i32,
        attack_range: i16,
    },
    /// One mercenary parameter (`ZC_MER_PAR_CHANGE`, `0x02A2`).
    MercenaryParameter {
        variable: u16,
        value: i32,
    },
    /// Elemental spirit (`ZC_EL_INIT`, `0x081D`).
    ElementalInfo {
        summary: String,
        class_id: u32,
        health_points: u32,
        maximum_health_points: u32,
        spell_points: u32,
        maximum_spell_points: u32,
    },
    /// Elemental HP or SP (`ZC_EL_PAR_CHANGE`, `0x081E`).
    ElementalParameter {
        variable: u16,
        value: u32,
    },
    /// The bank UI may open (`ZC_ACK_OPEN_BANKING`).
    BankOpened {
        unknown: i16,
    },
    /// Vault balance (`ZC_BANKING_CHECK`).
    BankBalance {
        money: i64,
        reason: i16,
    },
    /// Deposit finished (`ZC_ACK_BANKING_DEPOSIT`).
    BankDeposit {
        reason: i16,
        money: i64,
        zeny: i32,
    },
    /// Withdraw finished (`ZC_ACK_BANKING_WITHDRAW`).
    BankWithdraw {
        reason: i16,
        money: i64,
        zeny: i32,
    },
    /// The bank UI closed (`ZC_ACK_CLOSE_BANKING`).
    BankClosed {
        unknown: i16,
    },
    /// Cash shop points. `tab` is 0 when the older 10-byte open packet arrives.
    CashShopOpened {
        cash_points: u32,
        kafra_points: u32,
        tab: u32,
    },
    /// One cash-shop tab (`ZC_ACK_SE_CASH_ITEM_LIST`).
    CashShopItems {
        tab: u32,
        items: Vec<CashShopOffer>,
    },
    /// A cash-shop purchase finished (`ZC_SE_PC_BUY_CASHITEM_RESULT`).
    CashShopBuyResult {
        item_id: u32,
        result: u16,
        cash_points: u32,
        kafra_points: u32,
    },
    /// Roulette opened (`ZC_ACK_OPEN_ROULETTE`).
    RouletteOpened {
        result: i8,
        serial: i32,
        step: i8,
        index: i8,
        bonus_item_id: u32,
        gold: i32,
        silver: i32,
        bronze: i32,
    },
    /// Prize table (`ZC_ACK_ROULETTE_INFO`).
    RoulettePrizes {
        serial: u32,
        prizes: Vec<RoulettePrize>,
    },
    /// Spin result (`ZC_ACK_GENERATE_ROULETTE`).
    RouletteGenerated {
        result: u8,
        step: u16,
        index: u16,
        bonus_item_id: u32,
        gold: i32,
        silver: i32,
        bronze: i32,
    },
    /// Claim result (`ZC_RECV_ROULETTE_ITEM`).
    RouletteClaimed {
        result: u8,
        item_id: u32,
    },
    /// Roulette closed (`ZC_ACK_CLOSE_ROULETTE`).
    RouletteClosed {
        result: u8,
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

/// One cash-shop catalog row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CashShopOffer {
    pub item_id: u32,
    pub price: i32,
}

/// One roulette prize cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoulettePrize {
    pub row: u16,
    pub position: u16,
    pub item_id: u32,
    pub count: u16,
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

fn script_rgb(color: &ColorRGBA) -> (u8, u8, u8) {
    let packed = u32::from_le_bytes([color.red, color.green, color.blue, color.alpha]);
    (
        ((packed >> 16) & 0xFF) as u8,
        ((packed >> 8) & 0xFF) as u8,
        (packed & 0xFF) as u8,
    )
}

pub(crate) fn navigate_to_event(packet: NavigateToMonsterPacket) -> NetworkEvent {
    NetworkEvent::NavigateTo {
        target_type: packet.target_type,
        service_flags: packet.flags,
        show_window: packet.hide_window == 0,
        map_name: packet.map_name,
        x: packet.target_position.x,
        y: packet.target_position.y,
        monster_id: packet.target_monster_id,
    }
}

pub(crate) fn minimap_mark_event(packet: MarkMinimapPositionPacket) -> NetworkEvent {
    let kind = match packet.marker_type {
        MarkerType::DisplayFor15Seconds => 0,
        MarkerType::DisplayUntilLeave => 1,
        MarkerType::RemoveMark => 2,
    };
    let (red, green, blue) = script_rgb(&packet.color);
    NetworkEvent::MinimapMark {
        npc_id: packet.npc_id,
        kind,
        x: packet.position.x,
        y: packet.position.y,
        slot: packet.id,
        red,
        green,
        blue,
    }
}
