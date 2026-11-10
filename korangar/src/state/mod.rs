pub mod achievements;
pub mod bank_cash_roulette;
#[cfg(feature = "debug")]
pub mod cache_statistics;
pub mod character_slots;
pub mod chat;
pub mod clan;
pub mod equip_switch;
pub mod friends;
pub mod guild;
pub mod hotbar;
pub mod inventory;
pub mod item_drop;
pub mod item_finder;
pub mod item_workshop;
pub mod localization;
pub mod navigation;
pub mod pincode;
pub mod quest_log;
pub mod refine;
pub mod skill_cooldowns;
pub mod skills;
pub mod status_effects;
pub mod status_names;
pub mod theme;
pub mod trade;

use std::cell::Cell;
use std::sync::Arc;

use korangar_interface::application::Application;
use korangar_interface::components::button::ButtonTheme;
use korangar_interface::components::collapsible::CollapsibleTheme;
use korangar_interface::components::drop_down::DropDownTheme;
use korangar_interface::components::field::FieldTheme;
use korangar_interface::components::state_button::StateButtonTheme;
use korangar_interface::components::text::TextTheme;
use korangar_interface::components::text_box::TextBoxTheme;
use korangar_interface::element::StateElement;
use korangar_interface::layout::tooltip::TooltipTheme;
use korangar_interface::theme::ThemePathGetter;
use korangar_interface::window::{StateWindow, WindowTheme};
use korangar_networking::{MessageColor, SellItem, ShopItem};
use localization::Localization;
#[cfg(feature = "debug")]
use ragnarok_formats::map::{EffectSource, LightSource, MapData, SoundSource};
use ragnarok_packets::{AccountId, CharacterId, CharacterServerInformation, ClientTick, EntityId, ItemId};
#[cfg(feature = "debug")]
use rust_state::{ManuallyAssertExt, VecIndexExt};
use rust_state::{Path, PathExt, RustState, Selector};
use theme::{InterfaceTheme, InterfaceThemePathExt, InterfaceThemeType};

#[cfg(feature = "debug")]
use self::cache_statistics::CacheStatistics;
use self::clan::ClanState;
use self::friends::FriendListEntry;
use self::guild::GuildState;
use self::item_finder::ItemFinderState;
#[cfg(feature = "debug")]
use crate::PacketHistory;
#[cfg(feature = "debug")]
use crate::graphics::RenderOptions;
use crate::graphics::{Color, CornerDiameter, ScreenClip, ScreenPosition, ScreenSize, ShadowPadding};
use crate::input::{InputEvent, MouseInputMode};
use crate::interface::windows::{
    ChatWindowState, DialogWindowState, FriendListWindowState, GuildWindowState, LoginWindowState, LoginWindowStatePathExt,
    SkillTreeWindowState, WindowCache, WindowClass,
};
#[cfg(feature = "debug")]
use crate::interface::windows::{ProfilerWindowState, ThemeInspectorWindowState};
use crate::loaders::{ClientInfo, FontLoader, FontSize, GameFileLoader, OverflowBehavior, load_client_info};
use crate::renderer::InterfaceRenderer;
use crate::settings::{
    GameSettings, GraphicsSettingsCapabilities, InterfaceSettings, InterfaceSettingsCapabilities, LoginSettings, ServiceSettings,
    session_theme_name,
};
use crate::state::achievements::Achievements;
use crate::state::character_slots::CharacterSlots;
use crate::state::equip_switch::EquipSwitch;
use crate::state::hotbar::Hotbar;
use crate::state::inventory::Inventory;
use crate::state::item_drop::ItemDropPrompt;
use crate::state::item_workshop::{EnchantGradeState, ItemReformState, LaphineSynthesisState, LaphineUpgradeState};
use crate::state::navigation::Navigation;
use crate::state::pincode::PincodeWindowState;
use crate::state::quest_log::QuestLog;
use crate::state::refine::RefineState;
use crate::state::skill_cooldowns::SkillCooldowns;
use crate::state::skills::SkillTree;
use crate::state::status_effects::StatusEffects;
use crate::state::theme::WorldTheme;
use crate::state::trade::TradeState;
#[cfg(feature = "debug")]
use crate::world::Object;
use crate::world::{Entity, GroundItem, Player, ResourceMetadata};
use crate::{AudioSettings, GraphicsSettings};

/// A message in the in-game chat.
///
/// The message stores the color separately rather than baking it into the
/// message so the chat window can use the correct colors when switching themes.
/// A player currently showing a vend or buying-store sign.
#[derive(Clone, Debug, RustState, StateElement)]
pub struct ShopSign {
    pub account_id: AccountId,
    pub buying_store: bool,
}

/// One row in the player-shop window.
#[derive(Clone, Debug, RustState, StateElement)]
pub struct VendorRow {
    pub label: String,
    pub name: String,
    pub price: u32,
    pub amount: u32,
}

/// One row in the shop-search window.
#[derive(Clone, Debug, RustState, StateElement)]
pub struct SearchStoreRow {
    pub label: String,
    pub account_id: AccountId,
    pub store_id: u32,
    pub item_id: ItemId,
}

#[derive(Debug, Clone, RustState, StateElement)]
pub struct ChatMessage {
    /// Raw message.
    pub text: String,
    /// Color of the message.
    pub color: MessageColor,
    /// Tab this line belongs to. Derived from [`MessageColor`] unless a caller
    /// sets it with [`ChatMessage::with_channel`].
    pub channel: chat::ChatChannel,
}

impl ChatMessage {
    pub fn new(text: String, color: MessageColor) -> Self {
        let channel = chat::ChatChannel::from_message_color(color);
        Self { text, color, channel }
    }

    /// Party and guild tasks should pass [`chat::ChatChannel::Party`] or
    /// [`chat::ChatChannel::Guild`] when the color alone is not enough.
    pub fn with_channel(text: String, color: MessageColor, channel: chat::ChatChannel) -> Self {
        Self { text, color, channel }
    }
}

#[derive(Debug, Clone, Copy, RustState, StateElement)]
pub enum BufferedAction {
    AttackEntity { entity_id: EntityId },
    PickUpItem { entity_id: EntityId },
}

impl BufferedAction {
    pub fn is_attack_entity(&self, entity_id: EntityId) -> bool {
        matches!(self, BufferedAction::AttackEntity { entity_id: buffered_entity_id } if *buffered_entity_id == entity_id)
    }

    pub fn is_pick_up_item(&self, entity_id: EntityId) -> bool {
        matches!(self, BufferedAction::PickUpItem { entity_id: buffered_entity_id } if *buffered_entity_id == entity_id)
    }
}

/// Internal state of the client. Everything that can be viewed or modified via
/// the user interface should be in here. State that takes care of managing OS
/// or rendering resources should be in [`Client`](super::Client).
#[derive(RustState, StateElement, StateWindow)]
#[cfg_attr(feature = "debug", window_class(WindowClass::ClientStateInspector))]
#[window_title("Client State Inspector")]
#[state_root]
pub struct ClientState {
    /// Localization for the selected language.
    localization: Localization,

    /// Saved settings of previous connections and credentials.
    login_settings: LoginSettings,
    /// Saved audio settings.
    audio_settings: AudioSettings,
    /// Saved game settings.
    game_settings: GameSettings,
    /// Saved interface settings.
    interface_settings: InterfaceSettings,
    /// Interface capabilities used in the interface settings window.
    interface_settings_capabilities: InterfaceSettingsCapabilities,
    /// Saved graphics settings.
    graphics_settings: GraphicsSettings,
    /// Graphics capabilities used in the graphics settings window.
    graphics_settings_capabilities: GraphicsSettingsCapabilities,

    /// The interface theme for the menu windows.
    menu_theme: InterfaceTheme,
    /// The interface theme for in-game windows.
    in_game_theme: InterfaceTheme,
    /// Theme for themeable elements that don't change between the menu
    /// and playing theme.
    world_theme: WorldTheme,

    /// Loaded `sclientinfo.xml`.
    client_info: ClientInfo,
    /// Internal state of the login window.
    login_window: LoginWindowState,
    /// Internal state of the chat window.
    chat_window: ChatWindowState,
    /// Internal state of the friend list window.
    friend_list_window: FriendListWindowState,
    /// Internal state of the dialog window.
    dialog_window: DialogWindowState,
    /// Internal state of the enchant window.
    enchant_window: crate::interface::windows::EnchantWindowState,
    /// Internal state of the Turoran stable board.
    stable_window: crate::interface::windows::StableWindowState,
    /// Internal state of the Turoran Eden Group board.
    eden_window: crate::interface::windows::EdenWindowState,
    /// Internal state of the Turoran Warpra menu.
    warpra_window: crate::interface::windows::WarpraWindowState,
    /// World map region, hover, and dungeon floor picker.
    world_map_window: crate::interface::windows::WorldMapWindowState,
    /// Homunculus, pet, mercenary, and elemental info.
    companion_window: crate::interface::windows::CompanionWindowState,
    /// Alive MVPs from `0x0EF5` and the loaded map size used to plot them.
    mvp_minimap: crate::interface::windows::MvpMinimapState,
    /// Rodex inbox and the unread-mail flag.
    mail_window: crate::interface::windows::MailWindowState,
    /// Kafra bank vault.
    bank_window: crate::state::bank_cash_roulette::BankWindowState,
    /// Cash shop catalog. The menu still shows "Cash shop". Open, tab, and buy
    /// requests run only while `GameSettings.enable_cash_shop` is on.
    cash_shop_window: crate::state::bank_cash_roulette::CashShopWindowState,
    /// Roulette points and prize table.
    roulette_window: crate::state::bank_cash_roulette::RouletteWindowState,
    /// Internal state of the skill tree window.
    skill_tree_window: SkillTreeWindowState,

    /// All entities on the map.
    entities: Vec<Entity>,
    /// All dead entities on the map.
    dead_entities: Vec<Entity>,
    /// All ground items on the map.
    ground_items: Vec<GroundItem>,

    /// List of all received chat messages.
    chat_messages: Vec<ChatMessage>,
    /// Public speech drawn above speakers. Expires after about five seconds.
    chat_bubbles: Vec<chat::ChatBubble>,
    /// Top-of-screen broadcast (`0x009A` / `0x01C3`).
    announce: Option<chat::AnnounceBanner>,
    /// List of all friends.
    friend_list: Vec<FriendListEntry>,
    /// Guild info, members, and notice for the local character.
    guild: GuildState,
    /// Notice drafts edited in the guild window.
    guild_window: GuildWindowState,
    /// Clan info and online counts for the local character.
    clan: ClanState,
    /// Local Item Finder catalog, query, and result rows.
    item_finder: ItemFinderState,
    /// List of items offered in the shop.
    // TODO: Unhide this
    #[hidden_element]
    shop_items: Vec<ShopItem<ResourceMetadata>>,
    /// List of items in the buying cart.
    // TODO: Unhide this
    #[hidden_element]
    buy_cart: Vec<ShopItem<(ResourceMetadata, u32)>>,
    /// List of items that should be sold.
    // TODO: Unhide this
    #[hidden_element]
    sell_items: Vec<SellItem<(ResourceMetadata, u16)>>,
    /// List of items in the selling cart.
    // TODO: Unhide this
    #[hidden_element]
    sell_cart: Vec<SellItem<(ResourceMetadata, u16)>>,
    /// Players on this map who have a vend or buying store open.
    player_shops: Vec<ShopSign>,
    /// Account that owns the shop currently shown in the vendor window.
    vendor_account_id: AccountId,
    /// Vend id or buying-store id for the open shop.
    vendor_shop_id: u32,
    /// The open shop is a buying store (this client sells) rather than a vend.
    vendor_buying: bool,
    /// Cart indexes aligned with [`Self::vendor_rows`] for a vend.
    #[hidden_element]
    vendor_indexes: Vec<u16>,
    /// Item ids aligned with [`Self::vendor_rows`].
    #[hidden_element]
    vendor_item_ids: Vec<ItemId>,
    /// Rows shown in the vendor window.
    vendor_rows: Vec<VendorRow>,
    /// Rows shown in the shop-search window.
    search_store_hits: Vec<SearchStoreRow>,
    /// The name of the active character. This information is not available
    /// while playing if we don't save it here.
    player_name: String,
    /// Player configured hotbar.
    hotbar: Hotbar,
    /// Player inventory.
    inventory: Inventory,
    /// Amount typed when a stack is dropped on the ground.
    item_drop: ItemDropPrompt,
    /// Kafra or guild storage. Replaced when a storage list arrives.
    storage: Inventory,
    /// Alternate equipment set. This does not change what is currently worn.
    equip_switch: EquipSwitch,
    /// Player cart. Separate from the bag and from NPC buy/sell drafts.
    cart: Inventory,
    /// Player skill tree.
    skill_tree: SkillTree,
    /// Player-to-player trade.
    trade: TradeState,
    /// NPC refine UI.
    refine: RefineState,
    /// Item reform UI.
    item_reform: ItemReformState,
    /// Enchant grade UI.
    enchant_grade: EnchantGradeState,
    /// Laphine synthesis UI.
    laphine_synthesis: LaphineSynthesisState,
    /// Laphine upgrade UI.
    laphine_upgrade: LaphineUpgradeState,
    /// Local-player status effects (buffs/debuffs).
    status_effects: StatusEffects,
    /// Skill cooldown deadlines for hotbar overlays.
    skill_cooldowns: SkillCooldowns,
    /// Quest journal entries.
    quest_log: QuestLog,
    /// Achievement list and attendance period.
    achievements: Achievements,
    /// Navigation target and minimap viewpoint marks.
    navigation: Navigation,
    /// Latest client tick mirrored for UI overlays.
    client_tick: ClientTick,

    /// List of all available character servers.
    character_servers: Vec<CharacterServerInformation>,
    /// List of all the slots and characters on the current character server.
    character_slots: CharacterSlots,
    /// Id of the character that is currently being deleted. The server does not
    /// send back information about which character was deleted
    /// successfully, so we store it instead.
    currently_deleting: Option<CharacterId>,
    /// Slot of the character currently being moved.
    switch_request: Option<usize>,
    /// Name of the character being created currently.
    create_character_name: String,
    /// Second password asked by the character server before character select.
    pincode_window: PincodeWindowState,

    /// Size of the Korangar window.
    window_size: ScreenSize,

    /// Buffered player action. For example, attacking a target that is out of
    /// range.
    buffered_action: Option<BufferedAction>,

    /// Map data that is viewed in the inspector. Once added to this vector they
    /// are never removed so we can ensure the user interface remains valid.
    #[cfg(feature = "debug")]
    #[hidden_element]
    inspecting_maps: Vec<MapData>,
    /// Objects that are viewed in the inspector. Once added to this vector they
    /// are never removed so we can ensure the user interface remains valid.
    #[cfg(feature = "debug")]
    inspecting_objects: Vec<Object>,
    /// Light sources that are viewed in the inspector. Once added to this
    /// vector they are never removed so we can ensure the user interface
    /// remains valid.
    #[cfg(feature = "debug")]
    inspecting_light_sources: Vec<LightSource>,
    /// Sound sources that are viewed in the inspector. Once added to this
    /// vector they are never removed so we can ensure the user interface
    /// remains valid.
    #[cfg(feature = "debug")]
    inspecting_sound_sources: Vec<SoundSource>,
    /// Effect sources that are viewed in the inspector. Once added to this
    /// vector they are never removed so we can ensure the user interface
    /// remains valid.
    #[cfg(feature = "debug")]
    inspecting_effect_sources: Vec<EffectSource>,
    /// Special render options for debugging the client.
    #[cfg(feature = "debug")]
    render_options: RenderOptions,
    /// Internal state of the profiler window.
    #[cfg(feature = "debug")]
    profiler_window: ProfilerWindowState,
    /// Internal state of the theme inspector window.
    #[cfg(feature = "debug")]
    theme_inspector_window: ThemeInspectorWindowState,
    /// List of packets sent and received for the packet inspector. Also
    /// contains information about which packets to display in the
    /// inspector.
    #[cfg(feature = "debug")]
    packet_history: PacketHistory,
    /// Statistics of all caches of the loaders.
    #[cfg(feature = "debug")]
    cache_statistics: CacheStatistics,
}

impl ClientState {
    pub fn new(
        game_file_loader: &GameFileLoader,
        graphics_settings: GraphicsSettings,
        #[cfg(feature = "debug")] packet_history: PacketHistory,
    ) -> Self {
        time_phase!("load settings", {
            let mut login_settings = LoginSettings::new();
            let audio_settings = AudioSettings::new();
            let game_settings = GameSettings::new();
            let interface_settings = InterfaceSettings::new();
            let interface_settings_capabilities = InterfaceSettingsCapabilities::default();
        });

        time_phase!("load localization", {
            let localization = Localization::load_language(game_file_loader, interface_settings.language);
        });

        time_phase!("load themes", {
            let menu_theme = InterfaceTheme::load(InterfaceThemeType::Menu, &interface_settings.menu_theme);
            let in_game_theme = InterfaceTheme::load(
                InterfaceThemeType::InGame,
                session_theme_name(&interface_settings.in_game_theme),
            );
            let world_theme = WorldTheme::load(session_theme_name(&interface_settings.world_theme));
        });

        time_phase!("create login window state", {
            let client_info = load_client_info(game_file_loader);

            let selected_service = login_settings
                .recent_service_id
                // Make sure that the recent service id is still valid.
                .and_then(|service_id| {
                    client_info
                        .services
                        .iter()
                        .any(|service| service.service_id() == service_id)
                        .then_some(service_id)
                })
                // If there is no recent service id or it was no longer valid, select the first
                // service instead.
                .or_else(|| Some(client_info.services.first()?.service_id()))
                .expect("There are no services available. Check your sclientinfo.yaml.");

            // Make sure that every service has a service settings entry. Without a service
            // settings entry the login window will panic.
            for service in &client_info.services {
                login_settings.service_settings.entry(service.service_id()).or_default();
            }

            let login_window = LoginWindowState::new(selected_service);
        });

        time_phase!("create window state", {
            let welcome_string = format!(
                "Welcome to ^ff8800Korangar^000000 version ^ff8800{}^000000!",
                env!("CARGO_PKG_VERSION")
            );
            let chat_messages = vec![ChatMessage::new(welcome_string, MessageColor::Server)];

            let chat_window = ChatWindowState::default();
        });

        time_phase!("create character server resources", {
            // TODO: This could be in a single struct.
            let character_servers = Vec::new();
            // TODO: This could be in a single struct.
            let character_slots = CharacterSlots::default();
            // TODO: This could be in a single struct.
            let currently_deleting = None;
            // TODO: This could be in a single struct.
            let switch_request = None;
            let create_character_name = String::new();
            let pincode_window = PincodeWindowState::default();
        });

        time_phase!("create friend list state", {
            let friend_list = Vec::default();
            let friend_list_window = FriendListWindowState::default();
            let guild = GuildState::default();
            let guild_window = GuildWindowState::default();
            let clan = ClanState::default();
            let item_finder = ItemFinderState::default();
        });

        time_phase!("create player resources", {
            let dialog_window = DialogWindowState::default();
            let enchant_window = crate::interface::windows::EnchantWindowState::default();
            let stable_window = crate::interface::windows::StableWindowState::default();
            let eden_window = crate::interface::windows::EdenWindowState::default();
            let warpra_window = crate::interface::windows::WarpraWindowState::default();
            let world_map_window = crate::interface::windows::WorldMapWindowState::default();
            let companion_window = crate::interface::windows::CompanionWindowState::default();
            let mvp_minimap = crate::interface::windows::MvpMinimapState::default();
            let mail_window = crate::interface::windows::MailWindowState::default();
            let bank_window = crate::state::bank_cash_roulette::BankWindowState::default();
            let cash_shop_window = crate::state::bank_cash_roulette::CashShopWindowState::default();
            let roulette_window = crate::state::bank_cash_roulette::RouletteWindowState::default();

            let shop_items = Vec::default();
            let buy_cart = Vec::default();
            let sell_items = Vec::default();
            let sell_cart = Vec::default();
            let player_shops = Vec::default();
            let vendor_account_id = AccountId(0);
            let vendor_shop_id = 0;
            let vendor_buying = false;
            let vendor_indexes = Vec::default();
            let vendor_item_ids = Vec::default();
            let vendor_rows = Vec::default();
            let search_store_hits = Vec::default();
            let player_name = String::new();
            let hotbar = Hotbar::default();
            let inventory = Inventory::default();
            let item_drop = ItemDropPrompt::default();
            let storage = Inventory::default();
            let cart = Inventory::default();
            let skill_tree = SkillTree::default();
            let skill_tree_window = SkillTreeWindowState::default();
            let trade = TradeState::default();
            let refine = RefineState::default();
            let item_reform = ItemReformState::default();
            let enchant_grade = EnchantGradeState::default();
            let laphine_synthesis = LaphineSynthesisState::default();
            let laphine_upgrade = LaphineUpgradeState::default();
            let status_effects = StatusEffects::default();
            let skill_cooldowns = SkillCooldowns::default();
            let quest_log = QuestLog::default();
            let achievements = Achievements::default();
            let navigation = Navigation::default();
            let client_tick = ClientTick(0);
        });

        time_phase!("create window resources", {
            let window_size = ScreenSize::default();
            let graphics_settings_capabilities = GraphicsSettingsCapabilities::default();
        });

        let buffered_action = None;

        #[cfg(feature = "debug")]
        let debug_timer = korangar_debug::logging::Timer::new("creating debug resources");

        #[cfg(feature = "debug")]
        let inspecting_maps = Vec::new();
        #[cfg(feature = "debug")]
        let inspecting_objects = Vec::new();
        #[cfg(feature = "debug")]
        let inspecting_light_sources = Vec::new();
        #[cfg(feature = "debug")]
        let inspecting_sound_sources = Vec::new();
        #[cfg(feature = "debug")]
        let inspecting_effect_sources = Vec::new();

        #[cfg(feature = "debug")]
        let render_options = RenderOptions::new();

        #[cfg(feature = "debug")]
        let profiler_window = ProfilerWindowState::default();
        #[cfg(feature = "debug")]
        let theme_inspector_window = ThemeInspectorWindowState::default();

        #[cfg(feature = "debug")]
        let cache_statistics = CacheStatistics::default();

        #[cfg(feature = "debug")]
        debug_timer.stop();

        ClientState {
            localization,
            login_settings,
            audio_settings,
            game_settings,
            interface_settings,
            interface_settings_capabilities,
            graphics_settings,
            graphics_settings_capabilities,
            menu_theme,
            in_game_theme,
            world_theme,
            client_info,
            login_window,
            chat_window,
            friend_list_window,
            dialog_window,
            enchant_window,
            stable_window,
            eden_window,
            warpra_window,
            world_map_window,
            companion_window,
            mvp_minimap,
            mail_window,
            bank_window,
            cash_shop_window,
            roulette_window,
            skill_tree_window,
            entities: Vec::new(),
            dead_entities: Vec::new(),
            ground_items: Vec::new(),
            chat_messages,
            chat_bubbles: Vec::new(),
            announce: None,
            friend_list,
            guild,
            guild_window,
            clan,
            item_finder,
            shop_items,
            buy_cart,
            sell_items,
            sell_cart,
            player_shops,
            vendor_account_id,
            vendor_shop_id,
            vendor_buying,
            vendor_indexes,
            vendor_item_ids,
            vendor_rows,
            search_store_hits,
            player_name,
            hotbar,
            inventory,
            item_drop,
            storage,
            equip_switch: EquipSwitch::default(),
            cart,
            skill_tree,
            trade,
            refine,
            item_reform,
            enchant_grade,
            laphine_synthesis,
            laphine_upgrade,
            status_effects,
            skill_cooldowns,
            quest_log,
            achievements,
            navigation,
            client_tick,
            character_servers,
            character_slots,
            currently_deleting,
            switch_request,
            create_character_name,
            pincode_window,
            window_size,
            buffered_action,
            #[cfg(feature = "debug")]
            inspecting_maps,
            #[cfg(feature = "debug")]
            inspecting_objects,
            #[cfg(feature = "debug")]
            inspecting_light_sources,
            #[cfg(feature = "debug")]
            inspecting_sound_sources,
            #[cfg(feature = "debug")]
            inspecting_effect_sources,
            #[cfg(feature = "debug")]
            render_options,
            #[cfg(feature = "debug")]
            profiler_window,
            #[cfg(feature = "debug")]
            theme_inspector_window,
            #[cfg(feature = "debug")]
            packet_history,
            #[cfg(feature = "debug")]
            cache_statistics,
        }
    }
}

impl ClientState {
    pub(crate) fn expire_chat_overlays(&mut self, now: u32) {
        chat::expire_bubbles(&mut self.chat_bubbles, now);
        chat::expire_announce(&mut self.announce, now);
    }

    pub(crate) fn outgoing_chat(&self, text: &str) -> chat::OutgoingChat {
        chat::parse_outgoing_chat(text, self.chat_window.whisper_target_text())
    }

    pub(crate) fn begin_whisper(&mut self, target: String, text: String) -> (String, String) {
        self.chat_window.begin_whisper(target, text)
    }

    pub(crate) fn note_local_speech(&mut self, entity_id: EntityId, text: String, now: u32) {
        chat::upsert_bubble(&mut self.chat_bubbles, entity_id, text, now);
    }

    pub(crate) fn record_entity_speech(&mut self, entity_id: EntityId, text: String, color: MessageColor, now: u32) {
        chat::record_entity_speech(&mut self.chat_messages, &mut self.chat_bubbles, entity_id, text, color, now);
    }

    pub(crate) fn record_broadcast(&mut self, text: String, color: MessageColor, now: u32) {
        chat::record_broadcast(&mut self.chat_messages, &mut self.announce, text, color, now);
    }

    pub(crate) fn record_whisper(&mut self, sender: String, text: String) {
        let ClientState {
            chat_messages,
            chat_window,
            ..
        } = self;
        chat::record_whisper(chat_messages, chat_window.recent_whisper_targets_mut(), &sender, &text);
    }

    pub(crate) fn record_whisper_ack(&mut self, result: u8) {
        let ClientState {
            chat_messages,
            chat_window,
            ..
        } = self;
        let (recent, pending_name, pending_text) = chat_window.pending_whisper_mut();
        chat::record_whisper_ack(chat_messages, recent, pending_name, pending_text, result);
    }
}

/// Thread local used to create a path without arguments that points to the
/// current theme when creating the layout for a given window.
#[thread_local]
static CURRENT_THEME: Cell<InterfaceThemeType> = Cell::new(InterfaceThemeType::InGame);

/// Path resolving to the selected theme for the window.
#[derive(Clone, Copy)]
pub struct ThemePath;

impl Path<ClientState, InterfaceTheme> for ThemePath {
    fn follow<'a>(&self, state: &'a ClientState) -> Option<&'a InterfaceTheme> {
        match CURRENT_THEME.get() {
            InterfaceThemeType::Menu => Some(&state.menu_theme),
            InterfaceThemeType::InGame => Some(&state.in_game_theme),
        }
    }

    fn follow_mut<'a>(&self, state: &'a mut ClientState) -> Option<&'a mut InterfaceTheme> {
        match CURRENT_THEME.get() {
            InterfaceThemeType::Menu => Some(&mut state.menu_theme),
            InterfaceThemeType::InGame => Some(&mut state.in_game_theme),
        }
    }
}

impl Selector<ClientState, InterfaceTheme> for ThemePath {
    fn select<'a>(&'a self, state: &'a ClientState) -> Option<&'a InterfaceTheme> {
        self.follow(state)
    }
}

/// Glue between our [`InterfaceTheme`] and [`korangar_interface`].
#[derive(Clone, Copy)]
pub struct ClientThemeGetter;

impl ThemePathGetter<ClientState> for ClientThemeGetter {
    fn new() -> Self {
        Self
    }

    fn window(self) -> impl Path<ClientState, WindowTheme<ClientState>> {
        ThemePath.window()
    }

    fn text(self) -> impl Path<ClientState, TextTheme<ClientState>> {
        ThemePath.text()
    }

    fn button(self) -> impl Path<ClientState, ButtonTheme<ClientState>> {
        ThemePath.button()
    }

    fn state_button(self) -> impl Path<ClientState, StateButtonTheme<ClientState>> {
        ThemePath.state_button()
    }

    fn text_box(self) -> impl Path<ClientState, TextBoxTheme<ClientState>> {
        ThemePath.text_box()
    }

    fn collapsible(self) -> impl Path<ClientState, CollapsibleTheme<ClientState>> {
        ThemePath.collapsible()
    }

    fn drop_down(self) -> impl Path<ClientState, DropDownTheme<ClientState>> {
        ThemePath.drop_down()
    }

    fn field(self) -> impl Path<ClientState, FieldTheme<ClientState>> {
        ThemePath.field()
    }

    fn tooltip(self) -> impl Path<ClientState, TooltipTheme<ClientState>> {
        ThemePath.tooltip()
    }
}

impl Application for ClientState {
    type Cache = WindowCache;
    type Clip = ScreenClip;
    type Color = Color;
    type CornerDiameter = CornerDiameter;
    type CustomEvent = InputEvent;
    type CustomMouseMode = MouseInputMode;
    type FontSize = FontSize;
    type OverflowBehavior = OverflowBehavior;
    type Position = ScreenPosition;
    type Renderer = InterfaceRenderer;
    type ShadowPadding = ShadowPadding;
    type Size = ScreenSize;
    type TextLayouter = Arc<FontLoader>;
    type ThemeGetter = ClientThemeGetter;
    type ThemeType = InterfaceThemeType;
    type WindowClass = WindowClass;

    fn set_current_theme_type(theme: InterfaceThemeType) {
        CURRENT_THEME.set(theme);
    }
}

/// Path to the [`ClientState`] root.
pub fn client_state() -> impl Path<ClientState, ClientState> {
    ClientState::path()
}

/// Path to the current theme, same as [`korangar_interface::theme::theme`] but
/// more strongly typed so we can use the additional fields.
pub fn client_theme() -> impl Path<ClientState, InterfaceTheme> {
    ThemePath
}

/// Path to the player as [`Player`].
pub fn this_player() -> impl Path<ClientState, Player, false> {
    #[derive(Clone, Copy)]
    struct CustomPath;

    impl Selector<ClientState, Player, false> for CustomPath {
        fn select<'a>(&'a self, state: &'a ClientState) -> Option<&'a Player> {
            self.follow(state)
        }
    }

    impl Path<ClientState, Player, false> for CustomPath {
        fn follow<'a>(&self, state: &'a ClientState) -> Option<&'a Player> {
            // TODO: Select our player better.
            match state.entities.first()? {
                Entity::Player(player) => Some(player),
                _ => unreachable!(),
            }
        }

        fn follow_mut<'a>(&self, state: &'a mut ClientState) -> Option<&'a mut Player> {
            // TODO: Select our player better.
            match state.entities.first_mut()? {
                Entity::Player(player) => Some(player),
                _ => unreachable!(),
            }
        }
    }

    CustomPath
}

/// Path to the player as [`Entity`].
pub fn this_entity() -> impl Path<ClientState, Entity, false> {
    #[derive(Clone, Copy)]
    struct CustomPath;

    impl Selector<ClientState, Entity, false> for CustomPath {
        fn select<'a>(&'a self, state: &'a ClientState) -> Option<&'a Entity> {
            self.follow(state)
        }
    }

    impl Path<ClientState, Entity, false> for CustomPath {
        fn follow<'a>(&self, state: &'a ClientState) -> Option<&'a Entity> {
            // TODO: Select our player better.
            state.entities.first()
        }

        fn follow_mut<'a>(&self, state: &'a mut ClientState) -> Option<&'a mut Entity> {
            // TODO: Select our player better.
            state.entities.first_mut()
        }
    }

    CustomPath
}

pub struct SelectedServicePath<P, S> {
    window_state_path: P,
    service_settings_path: S,
}

impl<P, S> SelectedServicePath<P, S>
where
    P: Path<ClientState, LoginWindowState>,
    S: Path<ClientState, LoginSettings>,
{
    pub fn new(window_state_path: P, service_settings_path: S) -> Self {
        Self {
            window_state_path,
            service_settings_path,
        }
    }
}

impl<P, S> Clone for SelectedServicePath<P, S>
where
    P: Path<ClientState, LoginWindowState>,
    S: Path<ClientState, LoginSettings>,
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<P, S> Copy for SelectedServicePath<P, S>
where
    P: Path<ClientState, LoginWindowState>,
    S: Path<ClientState, LoginSettings>,
{
}

impl<P, S> Selector<ClientState, ServiceSettings> for SelectedServicePath<P, S>
where
    P: Path<ClientState, LoginWindowState>,
    S: Path<ClientState, LoginSettings>,
{
    fn select<'a>(&'a self, state: &'a ClientState) -> Option<&'a ServiceSettings> {
        self.follow(state)
    }
}

impl<P, S> Path<ClientState, ServiceSettings> for SelectedServicePath<P, S>
where
    P: Path<ClientState, LoginWindowState>,
    S: Path<ClientState, LoginSettings>,
{
    fn follow<'a>(&self, state: &'a ClientState) -> Option<&'a ServiceSettings> {
        let selected_service_path = self.window_state_path.selected_service();
        let selected_service = selected_service_path.follow_safe(state);

        self.service_settings_path.follow_safe(state).service_settings.get(selected_service)
    }

    fn follow_mut<'a>(&self, state: &'a mut ClientState) -> Option<&'a mut ServiceSettings> {
        let selected_service_path = self.window_state_path.selected_service();
        let selected_service = *selected_service_path.follow_mut_safe(state);

        self.service_settings_path
            .follow_mut_safe(state)
            .service_settings
            .get_mut(&selected_service)
    }
}

#[cfg(feature = "debug")]
pub fn prepare_map_inspection(inspecting_maps: &mut Vec<MapData>, map_data: &MapData) -> impl Path<ClientState, MapData> {
    let index = inspecting_maps
        .iter()
        .position(|item| {
            item.ground_file == map_data.ground_file
                && item._ini_file == map_data._ini_file
                && item.gat_file == map_data.gat_file
                && item.build_version == map_data.build_version
        })
        .unwrap_or_else(|| {
            let index = inspecting_maps.len();
            inspecting_maps.push(map_data.clone());
            index
        });

    client_state().inspecting_maps().index(index).manually_asserted()
}

#[cfg(feature = "debug")]
pub fn prepare_object_inspection(inspecting_objects: &mut Vec<Object>, object: &Object) -> impl Path<ClientState, Object> {
    let index = inspecting_objects
        .iter()
        .position(|item| item.name == object.name && item.model_name == object.model_name && item.transform == object.transform)
        .unwrap_or_else(|| {
            let index = inspecting_objects.len();
            inspecting_objects.push(object.clone());
            index
        });

    client_state().inspecting_objects().index(index).manually_asserted()
}

#[cfg(feature = "debug")]
pub fn prepare_light_source_inspection(
    inspecting_light_sources: &mut Vec<LightSource>,
    light_source: &LightSource,
) -> impl Path<ClientState, LightSource> {
    let index = inspecting_light_sources
        .iter()
        .position(|item| item == light_source)
        .unwrap_or_else(|| {
            let index = inspecting_light_sources.len();
            inspecting_light_sources.push(light_source.clone());
            index
        });

    client_state().inspecting_light_sources().index(index).manually_asserted()
}

#[cfg(feature = "debug")]
pub fn prepare_sound_source_inspection(
    inspecting_sound_sources: &mut Vec<SoundSource>,
    sound_source: &SoundSource,
) -> impl Path<ClientState, SoundSource> {
    let index = inspecting_sound_sources
        .iter()
        .position(|item| item == sound_source)
        .unwrap_or_else(|| {
            let index = inspecting_sound_sources.len();
            inspecting_sound_sources.push(sound_source.clone());
            index
        });

    client_state().inspecting_sound_sources().index(index).manually_asserted()
}

#[cfg(feature = "debug")]
pub fn prepare_effect_source_inspection(
    inspecting_effect_sources: &mut Vec<EffectSource>,
    effect_source: &EffectSource,
) -> impl Path<ClientState, EffectSource> {
    let index = inspecting_effect_sources
        .iter()
        .position(|item| item == effect_source)
        .unwrap_or_else(|| {
            let index = inspecting_effect_sources.len();
            inspecting_effect_sources.push(effect_source.clone());
            index
        });

    client_state().inspecting_effect_sources().index(index).manually_asserted()
}
