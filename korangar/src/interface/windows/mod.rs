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
mod cash_shop;
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
mod guild_invite;
mod hotbar;
mod interface_settings;
mod inventory;
mod item_details;
mod item_drop;
mod item_finder;
mod item_workshop;
mod login;
mod mail;
#[cfg(feature = "debug")]
mod maps;
mod menu;
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
mod warpra;
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
pub use self::cash_shop::CashShopWindow;
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
pub use self::login::{LoginWindow, LoginWindowState, LoginWindowStatePathExt};
pub use self::mail::{MailWindow, MailWindowState};
#[cfg(feature = "debug")]
pub use self::maps::MapsWindow;
pub use self::menu::MenuWindow;
pub use self::mvp_minimap::{MvpMinimapState, MvpMinimapWindow};
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
pub use self::warpra::{WarpraWindow, WarpraWindowState};
pub use self::world_map::{
    InterfaceTextureAttempt, WorldMapWindow, WorldMapWindowState, select_interface_texture, should_close_after_result,
};
#[cfg(test)]
pub use self::world_map::interface_texture_candidates;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowClass {
    AudioSettings,
    Bank,
    Barter,
    CashShop,
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
    NpcInput,
    Enchant,
    Eden,
    GameSettings,
    InterfaceSettings,
    GraphicsSettings,
    Hotbar,
    Inventory,
    ItemDrop,
    Storage,
    Equipment,
    SwitchEquip,
    SkillTree,
    SkillDescription,
    Stable,
    Warpra,
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
    Vendor,
    SearchStore,
    VendingSetup,
    VendingReport,
    Refine,
    ItemFinder,
    ItemReform,
    EnchantGrade,
    LaphineSynthesis,
    LaphineUpgrade,
    StylingShop,
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
