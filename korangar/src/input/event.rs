#[cfg(feature = "debug")]
use cgmath::Vector2;
#[cfg(feature = "debug")]
use korangar_debug::profiling::FrameMeasurement;
use korangar_interface::event::{ClickHandler, Event, EventQueue};
use korangar_networking::{InventoryItem, ShopItem};
use ragnarok_packets::{
    AccountId, BuyOrSellOption, CharacterId, CharacterServerInformation, EntityId, HotbarSlot, ItemId, PartyId, ShopId, SkillId,
    SoldItemInformation, StatUpType, TilePosition,
};
use rust_state::State;

use crate::interface::resource::{ItemSource, SkillSource};
use crate::loaders::ServiceId;
use crate::state::ClientState;
use crate::state::skills::LearnableSkill;
#[cfg(feature = "debug")]
use crate::world::MarkerIdentifier;
use crate::world::ResourceMetadata;

/// An event triggered by the user through mouse or keyboard input.
#[derive(Clone, Debug)]
pub enum InputEvent {
    /// Log in to the login server.
    LogIn {
        /// Id of the selected service.
        service_id: ServiceId,
        /// Account username.
        username: String,
        /// Account password.
        password: String,
    },
    /// Select a character server.
    SelectServer {
        /// Selected character server.
        character_server_information: CharacterServerInformation,
    },
    /// Respawn the player.
    Respawn,
    /// Log out of the map server.
    LogOut,
    /// Log out of the character server.
    LogOutCharacter,
    /// Exit Korangar.
    Exit,
    /// Zoom the player camera.
    ZoomCamera {
        /// Amount to zoom.
        zoom_factor: f32,
    },
    /// Rotate the player camera.
    RotateCamera {
        /// Amount of rotation.
        rotation: f32,
    },
    /// Reset the player camera rotation.
    ResetCameraRotation,
    /// Open or close the menu window. Only works while playing.
    ToggleMenuWindow,
    /// Menu-bar button. `index` is the position in
    /// [`crate::interface::windows::menu_bar::menu_bar_entries`].
    MenuBarPress {
        index: u8,
    },
    /// Switch the menu bar between fit, two rows, three rows and vertical.
    CycleMenuBarLayout,
    /// Open or close the chat window.
    ToggleChatWindow,
    /// Open or close the hotbar.
    ToggleHotbarWindow,
    /// Open or close the emotion picker.
    ToggleEmotesWindow,
    /// Open or close the character overview.
    ToggleCharacterOverviewWindow,
    /// Open or close the inventory window. Only works while playing.
    ToggleInventoryWindow,
    /// Open or close the player cart window. Only works while playing.
    ToggleCartWindow,
    /// Open or close the equipment window. Only works while playing.
    ToggleEquipmentWindow,
    /// Open or close the alternate equipment set window.
    ToggleSwitchEquipWindow,
    /// Ask the server to swap worn equipment with the alternate set.
    ActivateEquipSwitch,
    /// Remove one item from the alternate equipment set.
    RemoveEquipSwitchItem {
        /// Inventory index of the item to remove.
        index: ragnarok_packets::InventoryIndex,
    },
    /// Snapshot the live switch set into a named Gear loadout.
    SaveEquipLoadout,
    /// Open or close the saved-set menu on Gear.
    ToggleGearSets,
    /// Edit the selected set name inline.
    BeginRenameLoadout,
    /// Remove the selected saved set. No-op when none is selected.
    DeleteEquipLoadout,
    /// Rename the selected Gear loadout from the draft name.
    RenameEquipLoadout,
    /// Leave inline rename and drop the draft name.
    CancelRenameLoadout,
    /// Write a saved loadout back to the switch set.
    ///
    /// `wear` also asks the server to swap that set onto the character.
    ApplyEquipLoadout {
        index: usize,
        wear: bool,
    },
    /// Open or close the skill tree window. Only works while playing.
    ToggleSkillTreeWindow,
    /// Open or close the stats window. Only works while playing.
    ToggleStatsWindow,
    /// Open or close the game settings window.
    ToggleGameSettingsWindow,
    /// Open or close the interface settings window.
    ToggleInterfaceSettingsWindow,
    /// Open or close the graphics settings window.
    ToggleGraphicsSettingsWindow,
    /// Open or close the audio settings window.
    ToggleAudioSettingsWindow,
    /// Open or close the friend list window. Only works while playing.
    ToggleFriendListWindow,
    /// Open or close the guild window. Only works while playing.
    ToggleGuildWindow,
    /// Open or close the clan window. Only works while playing.
    ToggleClanWindow,
    /// Open or close the local Item Finder. Only works while playing.
    ToggleItemFinderWindow,
    /// Re-run the Item Finder query already stored in client state.
    SearchItemFinder,
    /// Switch the Item Finder category and search again.
    SetItemFinderCategory {
        category: &'static str,
    },
    /// Ask the map server for guild info, members, positions, and the notice.
    RequestGuildWindow,
    /// Send a new guild subject and notice. The server accepts this from the
    /// guild master.
    UpdateGuildNotice {
        subject: String,
        notice: String,
    },
    /// `CZ_REQ_MAKE_GUILD` (`0x0165`).
    CreateGuild {
        name: String,
    },
    /// `CZ_REQ_JOIN_GUILD2` (`0x0916`). The window invites by character name.
    InviteToGuild {
        name: String,
    },
    /// Accept `ZC_REQ_JOIN_GUILD` (`0x016B`, answer 1).
    AcceptGuildInvite {
        guild_id: u32,
    },
    /// Refuse `ZC_REQ_JOIN_GUILD` (`0x016B`, answer 0).
    RejectGuildInvite {
        guild_id: u32,
    },
    /// `CZ_REQ_LEAVE_GUILD` (`0x0159`).
    LeaveGuild {
        reason: String,
    },
    /// `CZ_REQ_BAN_GUILD` (`0x015B`).
    ExpelGuildMember {
        account_id: AccountId,
        character_id: CharacterId,
        reason: String,
    },
    /// `CZ_REQ_CHANGE_MEMBERPOS` (`0x0155`).
    SetGuildMemberPosition {
        account_id: AccountId,
        character_id: CharacterId,
        position_id: i32,
    },
    /// `CZ_REG_CHANGE_GUILD_POSITIONINFO` (`0x0161`). One position.
    SaveGuildPosition {
        position_id: i32,
        name: String,
        mode: i32,
        ranking: i32,
        pay_rate: i32,
    },
    /// Level a guild skill with the existing skill-up packet (`0x0112`).
    LevelUpGuildSkill {
        skill_id: u16,
    },
    /// `CZ_GUILD_CHAT` (`0x017E`).
    SendGuildMessage {
        text: String,
    },
    /// Read `guild-emblem.bmp` / `.png` (or `emblem.bmp` / `.png`) and upload
    /// `0x0153`.
    UploadGuildEmblem,
    /// `CZ_REQ_DELETE_RELATED_GUILD` (`0x0183`).
    BreakGuildRelation {
        guild_id: u32,
        opposition: i32,
    },
    /// Open or close the quest journal. Only works while playing.
    ToggleQuestJournalWindow,
    /// Ask the map server to mark a quest active or inactive
    /// (`CZ_ACTIVE_QUEST`).
    SetQuestActive {
        quest_id: u32,
        active: bool,
    },
    /// Open or close the skill description for one skill-tree slot.
    ToggleSkillDescription {
        skill_id: SkillId,
        title: String,
        body: String,
    },
    /// Open or close the achievement and attendance list.
    ToggleAchievementsWindow,
    /// Ask the map server for the attendance period.
    RequestAttendance,
    /// Ask the map server to grant the first completed achievement reward.
    ClaimAchievementReward,
    /// Cycle the achievement list: all, complete, incomplete, rewarded.
    CycleAchievementFilter,
    CycleAchievementCategory,
    /// Open or close the navigation and minimap-mark window.
    ToggleNavigationWindow,
    /// Open or close the status effects strip. Only works while playing.
    ToggleStatusEffectsWindow,
    ToggleStatusPanelWindow,
    /// Close the most recently opened or clicked closable window.
    CloseTopWindow,
    /// Escape closes the world map when it is open. Otherwise unfocus, close
    /// the top closable window, or toggle the menu.
    Escape,
    /// Re-read the active theme file from disk.
    ReloadTheme,
    /// Open or focus a pinned item-details window.
    OpenItemDetails {
        key: u64,
        title: String,
        body: String,
    },
    /// Toggle if the user interface should be rendered or not.
    ToggleShowInterface,
    /// Press one location on the shuffled PIN pad. The digit is the label
    /// there.
    PincodeDigit {
        index: usize,
    },
    /// Press the shuffled pad location that shows this digit (autopilot).
    PincodeTypeDigit {
        digit: u8,
    },
    /// Clear the PIN entry and move the pad labels.
    PincodeReset,
    /// Submit the hidden PIN entry.
    PincodeSubmit,
    /// Start the change-PIN flow from the check screen.
    PincodeChange,
    /// Ask the server for the PIN window, or reopen it.
    PincodeRequest,
    /// Leave the PIN screen and return to login.
    PincodeCancel,
    /// Select a character to start playing.
    SelectCharacter {
        /// Slot that the selected character is in.
        slot: usize,
    },
    /// Open a window to create a new character.
    OpenCharacterCreationWindow {
        /// Slot in which to create the new character.
        slot: usize,
    },
    /// Create a new character.
    CreateCharacter {
        /// Slot in which to create the new character.
        slot: usize,
        /// Name of the new character.
        name: String,
        hair_color: u16,
        hair_style: u16,
        /// Novice (`0`) or Summoner (`4218`).
        start_job: u16,
        /// `0` female, `1` male.
        sex: u8,
    },
    /// Reserve deletion (`0x0827`).
    DeleteCharacter {
        /// Id of the character to be deleted.
        character_id: CharacterId,
    },
    /// Open the six-character confirmation window for `0x0829`.
    OpenCharacterDeletionWindow {
        character_id: CharacterId,
    },
    /// Confirm a reserved deletion (`0x0829`).
    AcceptCharacterDeletion {
        character_id: CharacterId,
        confirmation: String,
    },
    /// Cancel a reserved deletion (`0x082B`).
    CancelCharacterDeletion {
        character_id: CharacterId,
    },
    /// Switch the characters of two slots.
    SwitchCharacterSlot {
        /// First slot.
        origin_slot: usize,
        /// Second slot.
        destination_slot: usize,
    },
    /// Start moving the player.
    PlayerMove {
        /// Destination of the move.
        destination: TilePosition,
    },
    /// Interact with an entity. The type of interaction depends on the entity
    /// type.
    PlayerInteract {
        /// Id of the entity to interact with.
        entity_id: EntityId,
    },
    /// Pick up an item from the ground.
    PickUpItem {
        /// Id of the item entity to pick up.
        entity_id: EntityId,
    },
    /// Send a chat message.
    SendMessage {
        /// Text of the message.
        text: String,
    },
    /// Action for the "Next"-button in a dialog.
    NextDialog {
        /// Id of the NPC the player is in a dialog with.
        npc_id: EntityId,
    },
    /// Close Kafra storage and tell the map server.
    CloseStorage,
    /// Random enchant (`CZ_REQUEST_RANDOM_ENCHANT`).
    RequestRandomEnchant,
    /// Perfect enchant (`CZ_REQUEST_PERFECT_ENCHANT`).
    RequestPerfectEnchant,
    /// Upgrade an existing enchant (`CZ_REQUEST_UPGRADE_ENCHANT`).
    RequestUpgradeEnchant,
    /// Reset an enchant (`CZ_REQUEST_RESET_ENCHANT`).
    RequestResetEnchant,
    /// Close the enchant UI and send `CZ_CLOSE_UI_ENCHANT`.
    CloseEnchant,
    /// Ask the map server for the Turoran stable board (`0x0EF0`).
    OpenStable,
    /// Request one stored homunculus (`0x0EF1`). Id 0 stores the active one.
    RequestStableHomunculus {
        homunculus_id: i32,
    },
    /// Pet stable action (`0x0EF2`). `id` is a pet id, an evolution egg, or an
    /// inventory index.
    RequestStablePet {
        action: i16,
        id: i32,
    },
    /// Ask the map server for the Turoran Eden board (`0x0EF3`, action 0).
    OpenEden,
    /// One or more Eden board buttons (`0x0EF3`). Buy repeats action 4 once per
    /// stack.
    EdenPackets {
        packets: Vec<ragnarok_packets::RequestEdenPacket>,
    },
    /// Open the Warpra menu and ask for the destination list (`0x0EF4`).
    OpenWarpra,
    /// Ask for the Warpra destination list again.
    RequestWarpraList,
    /// Warp to the kind and location id typed in the Warpra window.
    RequestWarpraWarp,
    /// Unlock the kind and location id typed in the Warpra window.
    RequestWarpraBuy,
    /// Open or close the world map. Only works while on a map.
    ToggleWorldMap,
    /// Warp to an unlocked world-map square (`0x0EF4` action warp).
    WorldMapWarp {
        /// Warpra destination kind.
        kind: i16,
        /// Warpra location id.
        location_id: i16,
    },
    /// Open the companion info window.
    OpenCompanions,
    /// `CZ_COMMAND_PET` feed.
    PetFeed,
    /// `CZ_COMMAND_PET` performance.
    PetPerformance,
    /// `CZ_COMMAND_PET` return to egg.
    PetReturn,
    /// `CZ_COMMAND_PET` unequip accessory.
    PetUnequip,
    /// `CZ_COMMAND_MER` feed.
    HomunculusFeed,
    /// `CZ_COMMAND_MER` release.
    HomunculusRelease,
    /// `CZ_MER_COMMAND` dismiss.
    MercenaryDismiss,
    /// Open or close the alive-MVP minimap.
    ToggleMvpMinimap,
    /// Open the Rodex window and reload the inbox (`0x0AC0` then `0x0AC1`).
    OpenMail,
    /// Reload the inbox (`0x0AC1`).
    RefreshMail,
    /// Ask for the next inbox page (`0x0AC1` with the last mail id).
    NextMailPage,
    /// Read the mail id typed in the mail window (`0x09EA`).
    ReadMail,
    /// Take attached zeny (`0x09F1`).
    TakeMailZeny,
    /// Take attached items (`0x09F3`).
    TakeMailItems,
    /// Delete the mail id typed in the mail window (`0x09F5`).
    DeleteMail,
    /// Tell the map server the mailbox closed (`0x09E9`).
    CloseMail,
    /// Open a blank Rodex compose form.
    OpenMailCompose,
    /// Open the compose form addressed to a character (context menu).
    ComposeMailTo {
        receiver: String,
    },
    /// Put a mail id in the Mail window and read it (autopilot).
    ReadMailId {
        mail_id: i64,
    },
    /// Reply to the mail last read in the Mail window.
    ReplyMail,
    /// Filter the loaded mail page by the search box.
    SearchMail,
    /// Check the receiver, then send the compose form.
    SendMail,
    /// Close the compose form without sending.
    CancelMailCompose,
    /// Open the bank and ask for the vault (`0x09B6`, `0x09AB`).
    OpenBank,
    /// Ask for the vault again (`0x09AB`).
    RefreshBank,
    /// Deposit the typed zeny (`0x09A7`).
    DepositBank,
    /// Withdraw the typed zeny (`0x09A9`).
    WithdrawBank,
    /// Add 100,000 to the bank amount box.
    BankAdd100k,
    /// Add 1,000,000 to the bank amount box.
    BankAdd1m,
    /// Add 10,000,000 to the bank amount box.
    BankAdd10m,
    /// Fill the bank amount box with the last known pocket zeny.
    BankSetMax,
    /// Close the bank (`0x09B8`).
    CloseBank,
    /// Open the roulette and ask for the prize table (`0x0A19`, `0x0A1B`).
    OpenRoulette,
    /// Ask for the prize table again (`0x0A1B`).
    RequestRouletteInfo,
    /// Spin the roulette (`0x0A1F`).
    SpinRoulette,
    /// Claim the pending prize (`0x0A21`).
    ClaimRoulette,
    /// Close the roulette (`0x0A1D`).
    CloseRoulette,
    /// OK on the NPC number or string input.
    SubmitNpcInput,
    /// Cancel on the NPC number or string input. Still answers the script.
    CancelNpcInput,
    /// Action for the "Close"-button in a dialog.
    CloseDialog {
        /// Id of the NPC the player is in a dialog with.
        npc_id: EntityId,
    },
    /// Choose an option in a dialog.
    ChooseDialogOption {
        /// Id of the NPC the player is in a dialog with.
        npc_id: EntityId,
        /// Id of the option.
        option: i8,
    },
    /// Use the inventory item at `index`. Healing, usable, delayed-consume, and
    /// item type 18 are sent as `CZ_USE_ITEM2`. Equippable types equip. The
    /// hotbar (G07) sends this without opening the bag.
    UseItem {
        /// Inventory index.
        index: ragnarok_packets::InventoryIndex,
    },
    /// Drop an inventory item on the ground. Stacks may open the amount window
    /// instead of sending the packet immediately.
    DropItem {
        /// Item dragged out of the inventory.
        item: InventoryItem<ResourceMetadata>,
    },
    /// Send the amount typed into the drop window.
    ConfirmItemDrop,
    /// Close the drop window without sending a packet.
    CancelItemDrop,
    /// Highlight a row in the item or skill selection list.
    SelectItemSkillRow {
        /// Row index in the open list.
        index: u32,
    },
    /// Send the selection. Weapon refine stays open.
    ConfirmItemSkillSelection,
    /// Send the protocol cancel, then close the list.
    CancelItemSkillSelection,
    /// Move an item in the user interface.
    MoveItem {
        /// Source of the move.
        source: ItemSource,
        /// Destination of the move.
        destination: ItemSource,
        /// Item to move.
        item: InventoryItem<ResourceMetadata>,
    },
    /// Show an emotion.
    SendEmotion {
        /// Id of the emotion.
        emotion: u8,
    },
    /// Move a skill in the user interface.
    MoveSkill {
        /// Source of the move.
        source: SkillSource,
        /// Destination of the move.
        destination: SkillSource,
        /// Skill to move.
        skill: LearnableSkill,
    },
    /// Cast a skill.
    CastSkill {
        /// Slot of the hotbar that the skill is bound to.
        slot: HotbarSlot,
    },
    /// Stop a skill.
    StopSkill {
        /// Slot of the hotbar that the skill is bound to.
        slot: HotbarSlot,
    },
    /// Press a hotbar slot. Items are used or equipped. Skills cast.
    ActivateHotbar {
        tab: ragnarok_packets::HotbarTab,
        slot: HotbarSlot,
    },
    /// Release a hotbar slot. Stops a channeled skill.
    ReleaseHotbar {
        tab: ragnarok_packets::HotbarTab,
        slot: HotbarSlot,
    },
    /// Sit down or stand up (`CZ_REQUEST_ACT` action 2 / 3).
    ToggleSit,
    /// Clear one hotbar slot and tell the map server.
    ClearHotbar {
        tab: ragnarok_packets::HotbarTab,
        slot: HotbarSlot,
    },
    /// Show the next hotbar row count (1 through 4).
    CycleHotbarRows,
    /// Rotate which stored row the F-keys hit and send `0x0B22`.
    RotateHotbar,
    /// Switch between the two Hercules hotkey tabs.
    SwitchHotbarTab,
    /// Open or close the keybind editor.
    ToggleKeybindsWindow,
    /// The next non-modifier key replaces this shortcut. Escape cancels.
    BeginKeybindCapture {
        index: u16,
    },
    /// Restore roBrowser / official default shortcuts.
    ResetKeybinds,
    /// Add a new friend.
    AddFriend {
        /// Name of the character to befriend.
        character_name: String,
    },
    /// Remove a current friend.
    RemoveFriend {
        /// Account id of the friend.
        account_id: AccountId,
        /// Character id of the friend.
        character_id: CharacterId,
    },
    /// Reject a pending friend request.
    RejectFriendRequest {
        /// Account id of the requestor.
        account_id: AccountId,
        /// Character id of the requestor.
        character_id: CharacterId,
    },
    /// Accept a pending friend request.
    AcceptFriendRequest {
        /// Account id of the requestor.
        account_id: AccountId,
        /// Character id of the requestor.
        character_id: CharacterId,
    },
    RejectPartyInvite {
        party_id: PartyId,
    },
    AcceptPartyInvite {
        party_id: PartyId,
    },
    /// Accept the invite stored on
    /// [`crate::state::party::PartyState::pending_invite`].
    AcceptPendingPartyInvite,
    /// `CZ_MAKE_GROUP2`.
    CreateParty {
        name: String,
        item_pickup: bool,
        item_division: bool,
    },
    /// `CZ_REQ_LEAVE_GROUP`.
    LeaveParty,
    /// `CZ_PARTY_JOIN_REQ`.
    InviteToParty {
        character_name: String,
    },
    /// `CZ_REQ_EXPEL_GROUP_MEMBER`.
    KickPartyMember {
        account_id: AccountId,
        name: String,
    },
    /// `CZ_CHANGE_GROUP_MASTER`.
    ChangePartyLeader {
        account_id: AccountId,
    },
    /// `CZ_GROUPINFO_CHANGE_V2`.
    SetPartyOptions {
        experience_share: u32,
        item_pickup: bool,
        item_division: bool,
    },
    /// `CZ_PARTY_CONFIG`. `refuse` true blocks incoming party invites.
    SetPartyInviteBlock {
        refuse: bool,
    },
    /// Buy one vend slot, or sell one matching inventory item into a buying
    /// store.
    VendorTrade {
        /// Index into the open vendor window.
        list_index: usize,
    },
    /// Request a player shop's item list.
    OpenPlayerShop {
        account_id: AccountId,
        /// When true, the shop is a buying store.
        buying_store: bool,
    },
    /// Ask the server for the map position of a shop-search row.
    LocateShop {
        account_id: AccountId,
        store_id: u32,
        item_id: ItemId,
    },
    /// Buy items from an NPC shop. This is not a player vend.
    BuyItems {
        /// Items to buy.
        items: Vec<ShopItem<u32>>,
    },
    /// Close the shop.
    CloseShop,
    /// Exchange the barter cart.
    PurchaseBarter,
    /// Close the barter shop and tell the NPC.
    CloseBarter,
    /// Choose whether to buy or sell items at a shop.
    BuyOrSell {
        /// Id of the open shop.
        shop_id: ShopId,
        /// Whether to sell or buy items.
        buy_or_sell: BuyOrSellOption,
    },
    /// Sell items to a shop.
    SellItems {
        /// Items to sell.
        items: Vec<SoldItemInformation>,
    },
    /// Up a stat.
    StatUp {
        stat_type: StatUpType,
    },
    /// Distribute skill points to meet all requirements for a given skill and
    /// put a single point into the provided skill. If the player does not
    /// have enough skill points, this will skill as much of the
    /// dependencies as possible.
    DistributePointsForSkill {
        /// Id of the skill to level up.
        skill_id: SkillId,
    },
    /// Accept an incoming player trade.
    AcceptTrade,
    /// Decline an incoming player trade.
    DeclineTrade,
    /// Offer one inventory stack. `index` is the stored inventory index.
    TradeAddItem {
        index: u16,
        amount: u16,
    },
    /// Offer zeny.
    TradeAddZeny {
        amount: u32,
    },
    /// Lock this side of the trade.
    TradeConfirm,
    /// Commit after both sides have locked.
    TradeDeal,
    /// Cancel the trade.
    TradeCancel,
    /// Place an inventory item into the refine UI.
    RefineSelectItem {
        index: u16,
    },
    /// Refine the selected item with this ore.
    RefineWith {
        item_id: u32,
    },
    /// Close the refine UI.
    RefineClose,
    /// Reform the typed inventory index with the open catalyst.
    ItemReformSubmit,
    /// Close the item reform UI.
    ItemReformClose,
    /// Ask the grade UI for materials on the typed inventory index.
    GradeEnchantSelect,
    /// Choose one grade option by its list index.
    GradeEnchantPick {
        material_index: i32,
    },
    /// Send the grade request.
    GradeEnchantSubmit,
    /// Close the grade UI.
    GradeEnchantClose,
    /// Queue one synthesis stack.
    LaphineSynthesisAdd,
    /// Clear the synthesis queue.
    LaphineSynthesisClear,
    /// Submit Laphine synthesis.
    LaphineSynthesisSubmit,
    /// Close the synthesis UI.
    LaphineSynthesisClose,
    /// Submit Laphine upgrade.
    LaphineUpgradeSubmit,
    /// Close the upgrade UI.
    LaphineUpgradeClose,
    /// Hair tab or headgear tab in the styling shop.
    StylingShopTab {
        headgear: bool,
    },
    /// Step one catalog row. `delta` is -1 or 1. Index 0 keeps the worn look.
    StylingShopStep {
        row: u8,
        delta: i8,
    },
    /// Send the selected catalog indexes.
    StylingShopBuy,
    /// Clear the styling shop selection.
    StylingShopReset,
    /// Close the styling shop and tell the server.
    StylingShopClose,
    /// Select one cart or bag row in the shop setup window.
    SelectShopDraft {
        index: usize,
    },
    /// Write the amount and price boxes onto the selected shop row.
    CommitShopDraft,
    /// Take the selected row back off the shop.
    UnlistShopDraft,
    /// Send the open-shop packet when the draft is valid.
    SubmitShopSetup,
    /// Tell the server the shop setup was cancelled.
    CancelShopSetup,
    /// Close this client's open vend or buying store.
    CloseOwnShop,
    /// Fill the shop-search picker from the item name query.
    SuggestShopSearch,
    /// Add one picker row to the search.
    AddShopSearchItem {
        index: usize,
    },
    /// Remove one selected search item.
    RemoveShopSearchItem {
        index: usize,
    },
    /// Send the search-store query.
    SubmitShopSearch,
    /// Ask for the next search-store page.
    ShopSearchNextPage,
    /// Close the search store.
    CloseShopSearch,
    /// Right-click a player, including this character.
    OpenPlayerContextMenu {
        entity_id: EntityId,
    },
    /// A row in the open player context menu. The action lives on menu state.
    ContextMenuPick {
        index: usize,
    },
    /// Toggle "show my equipment" and tell the map server.
    SetShowEquipment {
        enabled: bool,
    },
    /// `buying` selects buying stores. Otherwise the search looks at vending
    /// shops.
    SetShopSearchBuying {
        buying: bool,
    },
    /// Level up a skill.
    LevelUpSkills {
        /// List of skills to level up by one. This list is allowed to contain
        /// the same skill id multiple times and they will be applied
        /// sequentially from start to end.
        skill_ids: Vec<SkillId>,
    },
    /// Reload the language from disk.
    #[cfg(feature = "debug")]
    ReloadLanguage,
    /// Save the language to disk.
    #[cfg(feature = "debug")]
    SaveLanguage,
    /// Warp the player.
    #[cfg(feature = "debug")]
    WarpToMap {
        /// Map name. Can be the same as the current map.
        map_name: String,
        /// Position on the new map after the warp.
        position: TilePosition,
    },
    /// Open a window with the details for a marker.
    #[cfg(feature = "debug")]
    OpenMarkerDetails {
        /// Id of the marker to inspect.
        marker_identifier: MarkerIdentifier,
    },
    /// Open or close the render options window.
    #[cfg(feature = "debug")]
    ToggleRenderOptionsWindow,
    /// Open the map data window.
    #[cfg(feature = "debug")]
    OpenMapDataWindow,
    /// Open or close the client state inspector window.
    #[cfg(feature = "debug")]
    ToggleClientStateInspectorWindow,
    /// Open or close the maps window. Only works while playing.
    #[cfg(feature = "debug")]
    ToggleMapsWindow,
    /// Open or close the commands window. Only works while playing.
    #[cfg(feature = "debug")]
    ToggleCommandsWindow,
    /// Open the theme inspector window.
    #[cfg(feature = "debug")]
    ToggleThemeInspectorWindow,
    /// Open or close the profiler window.
    #[cfg(feature = "debug")]
    ToggleProfilerWindow,
    /// Open or close the packet inspector window.
    #[cfg(feature = "debug")]
    TogglePacketInspectorWindow,
    /// Open the cache statistics window.
    #[cfg(feature = "debug")]
    ToggleCacheStatisticsWindow,
    /// Move the view direction of the debug camera.
    #[cfg(feature = "debug")]
    CameraLookAround {
        /// Offset of the view direction.
        offset: Vector2<f32>,
    },
    /// Move the debug camera forward.
    #[cfg(feature = "debug")]
    CameraMoveForward,
    /// Move the debug camera backward.
    #[cfg(feature = "debug")]
    CameraMoveBackward,
    /// Move the debug camera left.
    #[cfg(feature = "debug")]
    CameraMoveLeft,
    /// Move the debug camera right.
    #[cfg(feature = "debug")]
    CameraMoveRight,
    /// Move the debug camera up.
    #[cfg(feature = "debug")]
    CameraMoveUp,
    /// Set the debug camera speed to its higher value.
    #[cfg(feature = "debug")]
    CameraAccelerate,
    /// Set the debug camera speed to its lower value.
    #[cfg(feature = "debug")]
    CameraDecelerate,
    /// Open a window to inspect a frame.
    #[cfg(feature = "debug")]
    InspectFrame {
        measurement: FrameMeasurement,
    },
}

impl From<InputEvent> for Event<ClientState> {
    fn from(custom_event: InputEvent) -> Self {
        Event::Application { custom_event }
    }
}

impl ClickHandler<ClientState> for InputEvent {
    fn handle_click(&self, _: &State<ClientState>, queue: &mut EventQueue<ClientState>) {
        queue.queue(self.clone());
    }
}
