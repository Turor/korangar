mod achievements;
mod audio_settings;
mod buy;
mod buy_cart;
mod buy_or_sell;
mod cache;
mod cart;
mod character_creation;
mod character_overview;
mod character_selection;
mod chat;
mod clan;
#[cfg(feature = "debug")]
mod commands;
mod companions;
mod dialog;
mod eden;
mod enchant;
mod equipment;
mod error;
#[cfg(feature = "debug")]
mod frame_inspector;
mod friend_list;
mod friend_request;
mod game_settings;
mod graphics_settings;
mod guild;
mod hotbar;
mod interface_settings;
mod inventory;
mod item_finder;
mod item_workshop;
mod login;
mod mail;
#[cfg(feature = "debug")]
mod maps;
mod menu;
mod mvp_minimap;
mod navigation;
#[cfg(feature = "debug")]
mod packet_inspector;
mod party_invite;
mod placement;
#[cfg(feature = "debug")]
mod profiler;
mod quest_journal;
mod refine;
#[cfg(feature = "debug")]
mod render_options;
mod respawn;
mod search_store;
mod sell;
mod sell_cart;
mod server_selection;
mod skill_tree;
mod stable;
mod stats;
mod status_effects;
mod storage;
mod switch_equip;
#[cfg(feature = "debug")]
mod theme_inspector;
mod trade;
mod vendor;
mod warpra;

use serde::{Deserialize, Serialize};

pub use self::achievements::AchievementsWindow;
pub use self::audio_settings::AudioSettingsWindow;
pub use self::buy::BuyWindow;
pub use self::buy_cart::BuyCartWindow;
pub use self::buy_or_sell::BuyOrSellWindow;
pub use self::cache::WindowCache;
pub use self::cart::CartWindow;
pub use self::character_creation::CharacterCreationWindow;
pub use self::character_overview::CharacterOverviewWindow;
pub use self::character_selection::CharacterSelectionWindow;
pub use self::chat::{ChatTextBox, ChatWindow, ChatWindowState};
pub use self::clan::ClanWindow;
#[cfg(feature = "debug")]
pub use self::commands::CommandsWindow;
pub use self::companions::{CompanionWindow, CompanionWindowState, MercenaryPanel, PetPanel};
pub use self::dialog::{DialogWindow, DialogWindowState};
pub use self::eden::{EdenWindow, EdenWindowState};
pub use self::enchant::{EnchantWindow, EnchantWindowState};
pub use self::equipment::EquipmentWindow;
pub(crate) use self::equipment::draw_world_entity;
pub use self::error::ErrorWindow;
#[cfg(feature = "debug")]
pub use self::frame_inspector::FrameInspectorWindow;
pub use self::friend_list::{FriendListWindow, FriendListWindowState};
pub use self::friend_request::FriendRequestWindow;
pub use self::game_settings::GameSettingsWindow;
pub use self::graphics_settings::GraphicsSettingsWindow;
pub use self::guild::{GuildWindow, GuildWindowState};
pub use self::hotbar::HotbarWindow;
pub use self::interface_settings::InterfaceSettingsWindow;
#[allow(unused_imports)]
pub use self::inventory::InventoryWindow;
pub use self::item_finder::ItemFinderWindow;
pub use self::item_workshop::{EnchantGradeWindow, ItemReformWindow, LaphineSynthesisWindow, LaphineUpgradeWindow};
pub use self::login::{LoginWindow, LoginWindowState, LoginWindowStatePathExt};
pub use self::mail::{MailWindow, MailWindowState};
#[cfg(feature = "debug")]
pub use self::maps::MapsWindow;
pub use self::menu::MenuWindow;
pub use self::mvp_minimap::{MvpMinimapState, MvpMinimapWindow};
pub use self::navigation::NavigationWindow;
#[cfg(feature = "debug")]
pub use self::packet_inspector::PacketInspectorWindow;
pub use self::party_invite::PartyInviteWindow;
#[cfg(feature = "debug")]
pub use self::profiler::{ProfilerWindow, ProfilerWindowState};
pub use self::quest_journal::QuestJournalWindow;
pub use self::refine::RefineWindow;
#[cfg(feature = "debug")]
pub use self::render_options::RenderOptionsWindow;
pub use self::respawn::RespawnWindow;
pub use self::search_store::SearchStoreWindow;
pub use self::sell::SellWindow;
pub use self::sell_cart::SellCartWindow;
pub use self::server_selection::ServerSelectionWindow;
pub use self::skill_tree::{SkillTreeWindow, SkillTreeWindowState, SkillTreeWindowStatePathExt};
pub use self::stable::{StableWindow, StableWindowState};
pub use self::stats::StatsWindow;
pub use self::status_effects::StatusEffectsWindow;
pub use self::storage::StorageWindow;
pub use self::switch_equip::SwitchEquipWindow;
#[cfg(feature = "debug")]
pub use self::theme_inspector::{ThemeInspectorWindow, ThemeInspectorWindowState};
pub use self::trade::{TradeRequestWindow, TradeWindow};
pub use self::vendor::VendorWindow;
pub use self::warpra::{WarpraWindow, WarpraWindowState};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowClass {
    AudioSettings,
    Buy,
    BuyCart,
    Cart,
    BuyOrSell,
    Chat,
    Companions,
    CharacterCreation,
    CharacterOverview,
    CharacterSelection,
    Dialog,
    Enchant,
    Eden,
    GameSettings,
    InterfaceSettings,
    GraphicsSettings,
    Hotbar,
    Inventory,
    Storage,
    Equipment,
    SwitchEquip,
    SkillTree,
    Stable,
    Warpra,
    Stats,
    StatusEffects,
    QuestJournal,
    Achievements,
    Navigation,
    FriendList,
    FriendRequest,
    Guild,
    Clan,
    PartyInvite,
    Login,
    Menu,
    Mail,
    MvpMinimap,
    Respawn,
    SelectServer,
    Sell,
    SellCart,
    Trade,
    TradeRequest,
    Vendor,
    SearchStore,
    Refine,
    ItemFinder,
    ItemReform,
    EnchantGrade,
    LaphineSynthesis,
    LaphineUpgrade,
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
