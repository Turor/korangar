mod achievements;
mod audio_settings;
mod bank;
mod barter;
mod basic_info_view;
mod buy;
mod buy_cart;
mod buy_or_sell;
mod cache;
mod cart;
mod character_appearance;
mod character_creation;
mod character_deletion;
mod character_overview;
mod character_preview;
mod character_selection;
mod chat;
mod clan;
#[cfg(feature = "debug")]
mod commands;
mod companions;
mod context_menu;
mod dialog;
mod eden;
mod emotes;
mod enchant;
pub(crate) mod equipment;
mod error;
#[cfg(feature = "debug")]
mod frame_inspector;
mod friend_list;
mod friend_request;
mod game_settings;
mod graphics_settings;
mod guild;
mod guild_invite;
mod hotbar;
mod interface_settings;
mod inventory;
mod item_details;
mod item_drop;
mod item_finder;
pub(crate) mod item_skill_selection;
mod item_workshop;
mod keybinds;
mod login;
mod mail;
#[cfg(feature = "debug")]
mod maps;
mod menu;
mod menu_bar;
mod misc_info;
mod mvp_minimap;
mod navigation;
mod npc_input;
#[cfg(feature = "debug")]
mod packet_inspector;
mod party;
mod party_invite;
mod pincode;
mod placement;
#[cfg(feature = "debug")]
mod profiler;
mod quest_journal;
mod refine;
#[cfg(feature = "debug")]
mod render_options;
mod respawn;
mod roulette;
mod search_store;
mod sell;
mod sell_cart;
mod server_selection;
mod skill_description;
mod skill_tree;
pub(crate) mod stable;
mod stats;
mod status_effects;
mod status_panel;
mod storage;
mod styling_shop;
mod switch_equip;
#[cfg(feature = "debug")]
mod theme_inspector;
mod trade;
mod vending_report;
mod vending_setup;
mod vendor;
mod view_equip;
mod warpra;
mod window_groups;
mod world_map;

use serde::{Deserialize, Serialize};

pub use self::achievements::AchievementsWindow;
pub use self::audio_settings::AudioSettingsWindow;
pub use self::bank::BankWindow;
pub use self::barter::BarterWindow;
pub use self::buy::BuyWindow;
pub use self::buy_cart::BuyCartWindow;
pub use self::buy_or_sell::BuyOrSellWindow;
pub use self::cache::WindowCache;
pub use self::cart::CartWindow;
pub use self::character_creation::CharacterCreationWindow;
pub use self::character_deletion::CharacterDeletionWindow;
pub use self::character_overview::CharacterOverviewWindow;
pub(crate) use self::character_preview::{sync_preview, take_preview_animation};
pub use self::character_selection::CharacterSelectionWindow;
pub use self::chat::{ChatTextBox, ChatWindow, ChatWindowState};
pub use self::clan::ClanWindow;
#[cfg(feature = "debug")]
pub use self::commands::CommandsWindow;
pub use self::companions::{CompanionWindow, CompanionWindowState, MercenaryPanel, PetPanel};
pub use self::context_menu::ContextMenuWindow;
pub use self::dialog::{DialogWindow, DialogWindowState};
pub use self::eden::EdenWindow;
pub use self::emotes::EmotesWindow;
pub use self::enchant::{EnchantWindow, EnchantWindowState};
pub use self::equipment::EquipmentWindow;
pub(crate) use self::equipment::draw_player_in_world;
pub use self::error::ErrorWindow;
#[cfg(feature = "debug")]
pub use self::frame_inspector::FrameInspectorWindow;
pub use self::friend_list::{FriendListWindow, FriendListWindowState};
pub use self::friend_request::FriendRequestWindow;
pub use self::game_settings::GameSettingsWindow;
pub use self::graphics_settings::GraphicsSettingsWindow;
pub use self::guild::{GuildWindow, GuildWindowState};
pub use self::guild_invite::GuildInviteWindow;
pub use self::hotbar::HotbarWindow;
pub use self::interface_settings::InterfaceSettingsWindow;
#[allow(unused_imports)]
pub use self::inventory::InventoryWindow;
pub use self::item_details::{ItemDetailsWindow, format_item_details, item_details_key};
pub use self::item_drop::{ItemDropAmountBox, ItemDropWindow};
pub use self::item_finder::ItemFinderWindow;
pub use self::item_workshop::{EnchantGradeWindow, ItemReformWindow, LaphineSynthesisWindow, LaphineUpgradeWindow};
pub use self::keybinds::KeybindsWindow;
pub use self::login::{LoginWindow, LoginWindowState, LoginWindowStatePathExt};
pub use self::mail::{MailWindow, MailWindowState};
#[cfg(feature = "debug")]
pub use self::maps::MapsWindow;
pub use self::menu::MenuWindow;
pub use self::menu_bar::{
    MenuBarIcons, MenuBarLayout, MenuBarPress, MenuBarWindow, docked_menu_bar_window, menu_bar_open_bits, next_distinct_layout,
    press_menu_bar,
};
pub use self::misc_info::{
    BattlegroundWindow, ItemObtainWindow, MemorialDungeonWindow, MiscInfoState, ReputationWindow, SenseWindow, now_unix,
};
pub use self::mvp_minimap::{MvpMinimapState, MvpMinimapWindow};
pub use self::window_groups::{WindowGroup, WindowGroupMemory, WindowGroupTabs, group_member_open_event};
pub use self::navigation::NavigationWindow;
pub use self::npc_input::{NpcInputBox, NpcInputWindow};
#[cfg(feature = "debug")]
pub use self::packet_inspector::PacketInspectorWindow;
pub use self::party_invite::PartyInviteWindow;
pub use self::pincode::PincodeWindow;
#[cfg(feature = "debug")]
pub use self::profiler::{ProfilerWindow, ProfilerWindowState};
pub use self::quest_journal::QuestJournalWindow;
pub use self::refine::RefineWindow;
#[cfg(feature = "debug")]
pub use self::render_options::RenderOptionsWindow;
pub use self::respawn::RespawnWindow;
pub use self::roulette::RouletteWindow;
pub use self::search_store::SearchStoreWindow;
pub use self::sell::SellWindow;
pub use self::sell_cart::SellCartWindow;
pub use self::server_selection::ServerSelectionWindow;
pub use self::skill_description::SkillDescriptionWindow;
pub use self::skill_tree::{SkillTreeWindow, SkillTreeWindowState, SkillTreeWindowStatePathExt};
pub use self::stable::{StableWindow, StableWindowState};
pub use self::stats::StatsWindow;
pub use self::status_effects::StatusEffectsWindow;
pub use self::status_panel::StatusPanelWindow;
pub use self::storage::StorageWindow;
pub use self::styling_shop::StylingShopWindow;
pub use self::switch_equip::SwitchEquipWindow;
#[cfg(feature = "debug")]
pub use self::theme_inspector::{ThemeInspectorWindow, ThemeInspectorWindowState};
pub use self::trade::{TradeRequestWindow, TradeWindow};
pub use self::vending_report::VendingReportWindow;
pub use self::vending_setup::VendingSetupWindow;
pub use self::vendor::VendorWindow;
pub use self::view_equip::ViewEquipWindow;
pub use self::warpra::{WarpraWindow, WarpraWindowState};
#[cfg(test)]
pub use self::world_map::interface_texture_candidates;
pub use self::world_map::{
    InterfaceTextureAttempt, WorldMapWindow, WorldMapWindowState, select_interface_texture, should_close_after_result,
};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowClass {
    AudioSettings,
    Bank,
    Barter,
    Buy,
    BuyCart,
    Cart,
    BuyOrSell,
    Chat,
    Companions,
    CharacterCreation,
    CharacterDeletion,
    CharacterOverview,
    CharacterSelection,
    Dialog,
    NpcInput,
    Enchant,
    Eden,
    GameSettings,
    InterfaceSettings,
    GraphicsSettings,
    Hotbar,
    Keybinds,
    Inventory,
    ItemDrop,
    Storage,
    Equipment,
    SwitchEquip,
    SkillTree,
    SkillDescription,
    Stable,
    Warpra,
    Sense,
    MemorialDungeon,
    Battleground,
    Reputation,
    ItemObtain,
    WorldMap,
    Stats,
    StatusEffects,
    StatusPanel,
    QuestJournal,
    Achievements,
    Navigation,
    FriendList,
    FriendRequest,
    Guild,
    GuildInvite,
    Clan,
    PartyInvite,
    Pincode,
    Login,
    Menu,
    Mail,
    MvpMinimap,
    Respawn,
    Roulette,
    SelectServer,
    Sell,
    SellCart,
    Trade,
    TradeRequest,
    ContextMenu,
    ViewEquip,
    Vendor,
    SearchStore,
    VendingSetup,
    VendingReport,
    Refine,
    ItemSkillSelection,
    ItemFinder,
    ItemReform,
    EnchantGrade,
    LaphineSynthesis,
    LaphineUpgrade,
    StylingShop,
    MenuBar,
    Emotes,
    #[cfg(feature = "debug")]
    Maps,
    #[cfg(feature = "debug")]
    ClientStateInspector,
    #[cfg(feature = "debug")]
    PacketInspector,
    #[cfg(feature = "debug")]
    RenderOptions,
    #[cfg(feature = "debug")]
    Commands,
    #[cfg(feature = "debug")]
    ThemeInspector,
    #[cfg(feature = "debug")]
    Profiler,
    #[cfg(feature = "debug")]
    CacheStatistics,
}
