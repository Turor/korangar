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
    /// Rename the selected Gear loadout from the draft name.
    RenameEquipLoadout,
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
    /// Ask the map server for guild info, members, positions, and the notice.
    RequestGuildWindow,
    /// Send a new guild subject and notice. The server accepts this from the
    /// guild master.
    UpdateGuildNotice {
        subject: String,
        notice: String,
    },
    /// Open or close the quest journal. Only works while playing.
    ToggleQuestJournalWindow,
    /// Open or close the navigation and minimap-mark window.
    ToggleNavigationWindow,
    /// Open or close the status effects strip. Only works while playing.
    ToggleStatusEffectsWindow,
    /// Close the most recently opened or clicked closable window.
    CloseTopWindow,
    /// Toggle if the user interface should be rendered or not.
    ToggleShowInterface,
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
    },
    /// Delete a character.
    DeleteCharacter {
        /// Id of the character to be deleted.
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
    /// Request one stored homunculus (`0x0EF1`).
    RequestStableHomunculus,
    /// Request one stored pet (`0x0EF2`).
    RequestStablePet,
    /// Ask the map server for the Turoran Eden board (`0x0EF3`, action 0).
    OpenEden,
    /// Accept the typed Eden quest (`0x0EF3`, action 1).
    EdenAccept,
    /// Turn in the typed Eden quest (`0x0EF3`, action 2).
    EdenComplete,
    /// Abandon the typed Eden quest (`0x0EF3`, action 3).
    EdenAbandon,
    /// Buy the typed Eden shop slot (`0x0EF3`, action 4).
    EdenBuy,
    /// Open the Warpra menu and ask for the destination list (`0x0EF4`).
    OpenWarpra,
    /// Ask for the Warpra destination list again.
    RequestWarpraList,
    /// Warp to the kind and location id typed in the Warpra window.
    RequestWarpraWarp,
    /// Unlock the kind and location id typed in the Warpra window.
    RequestWarpraBuy,
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
    /// Read the mail id typed in the mail window (`0x09EA`).
    ReadMail,
    /// Delete the mail id typed in the mail window (`0x09F5`).
    DeleteMail,
    /// Tell the map server the mailbox closed (`0x09E9`).
    CloseMail,
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
