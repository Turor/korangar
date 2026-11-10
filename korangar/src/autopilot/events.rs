//! Curated `InputEvent` names for the autopilot command channel.
//!
//! `InputEvent` does not implement serde. `event <spec>` accepts every unit
//! variant by its Rust name, plus the parameterized forms in
//! [`scripted_event`]. `toggle` accepts only unit variants whose name starts
//! with `Toggle` or `Open`. `LogIn` is never constructed here.

use ragnarok_packets::{HotbarSlot, InventoryIndex, StatUpType, TilePosition};
use serde::Deserialize;

use crate::input::InputEvent;
use crate::interface::windows::WindowClass;

pub struct WindowCommand {
    pub class: WindowClass,
    pub open: bool,
    pub event: InputEvent,
}

pub fn unit_event(name: &str) -> Option<InputEvent> {
    let event = match name {
        "Respawn" => Some(InputEvent::Respawn),
        "LogOut" => Some(InputEvent::LogOut),
        "LogOutCharacter" => Some(InputEvent::LogOutCharacter),
        "Exit" => Some(InputEvent::Exit),
        "ResetCameraRotation" => Some(InputEvent::ResetCameraRotation),
        "ToggleMenuWindow" => Some(InputEvent::ToggleMenuWindow),
        "ToggleWorldMap" => Some(InputEvent::ToggleWorldMap),
        "ToggleChatWindow" => Some(InputEvent::ToggleChatWindow),
        "ToggleHotbarWindow" => Some(InputEvent::ToggleHotbarWindow),
        "ToggleEmotesWindow" => Some(InputEvent::ToggleEmotesWindow),
        "ToggleKeybindsWindow" => Some(InputEvent::ToggleKeybindsWindow),
        "ToggleCharacterOverviewWindow" => Some(InputEvent::ToggleCharacterOverviewWindow),
        "ToggleInventoryWindow" => Some(InputEvent::ToggleInventoryWindow),
        "ToggleCartWindow" => Some(InputEvent::ToggleCartWindow),
        "ToggleEquipmentWindow" => Some(InputEvent::ToggleEquipmentWindow),
        "ToggleSwitchEquipWindow" => Some(InputEvent::ToggleSwitchEquipWindow),
        "ActivateEquipSwitch" => Some(InputEvent::ActivateEquipSwitch),
        "SaveEquipLoadout" => Some(InputEvent::SaveEquipLoadout),
        "ToggleGearSets" => Some(InputEvent::ToggleGearSets),
        "BeginRenameLoadout" => Some(InputEvent::BeginRenameLoadout),
        "DeleteEquipLoadout" => Some(InputEvent::DeleteEquipLoadout),
        "RenameEquipLoadout" => Some(InputEvent::RenameEquipLoadout),
        "CancelRenameLoadout" => Some(InputEvent::CancelRenameLoadout),
        "ToggleSkillTreeWindow" => Some(InputEvent::ToggleSkillTreeWindow),
        "ToggleStatsWindow" => Some(InputEvent::ToggleStatsWindow),
        "ToggleGameSettingsWindow" => Some(InputEvent::ToggleGameSettingsWindow),
        "ToggleInterfaceSettingsWindow" => Some(InputEvent::ToggleInterfaceSettingsWindow),
        "ToggleGraphicsSettingsWindow" => Some(InputEvent::ToggleGraphicsSettingsWindow),
        "ToggleAudioSettingsWindow" => Some(InputEvent::ToggleAudioSettingsWindow),
        "ToggleFriendListWindow" => Some(InputEvent::ToggleFriendListWindow),
        "ToggleGuildWindow" => Some(InputEvent::ToggleGuildWindow),
        "ToggleClanWindow" => Some(InputEvent::ToggleClanWindow),
        "ToggleItemFinderWindow" => Some(InputEvent::ToggleItemFinderWindow),
        "SearchItemFinder" => Some(InputEvent::SearchItemFinder),
        "RequestGuildWindow" => Some(InputEvent::RequestGuildWindow),
        "ToggleQuestJournalWindow" => Some(InputEvent::ToggleQuestJournalWindow),
        "ToggleAchievementsWindow" => Some(InputEvent::ToggleAchievementsWindow),
        "RequestAttendance" => Some(InputEvent::RequestAttendance),
        "ClaimAchievementReward" => Some(InputEvent::ClaimAchievementReward),
        "CycleAchievementFilter" => Some(InputEvent::CycleAchievementFilter),
        "CycleAchievementCategory" => Some(InputEvent::CycleAchievementCategory),
        "ToggleNavigationWindow" => Some(InputEvent::ToggleNavigationWindow),
        "ToggleStatusEffectsWindow" => Some(InputEvent::ToggleStatusEffectsWindow),
        "CloseTopWindow" => Some(InputEvent::CloseTopWindow),
        "ToggleShowInterface" => Some(InputEvent::ToggleShowInterface),
        "CloseStorage" => Some(InputEvent::CloseStorage),
        "RequestRandomEnchant" => Some(InputEvent::RequestRandomEnchant),
        "RequestPerfectEnchant" => Some(InputEvent::RequestPerfectEnchant),
        "RequestUpgradeEnchant" => Some(InputEvent::RequestUpgradeEnchant),
        "RequestResetEnchant" => Some(InputEvent::RequestResetEnchant),
        "CloseEnchant" => Some(InputEvent::CloseEnchant),
        "OpenStable" => Some(InputEvent::OpenStable),
        "OpenEden" => Some(InputEvent::OpenEden),
        "OpenWarpra" => Some(InputEvent::OpenWarpra),
        "RequestWarpraList" => Some(InputEvent::RequestWarpraList),
        "RequestWarpraWarp" => Some(InputEvent::RequestWarpraWarp),
        "RequestWarpraBuy" => Some(InputEvent::RequestWarpraBuy),
        "OpenCompanions" => Some(InputEvent::OpenCompanions),
        "PetFeed" => Some(InputEvent::PetFeed),
        "PetPerformance" => Some(InputEvent::PetPerformance),
        "PetReturn" => Some(InputEvent::PetReturn),
        "PetUnequip" => Some(InputEvent::PetUnequip),
        "HomunculusFeed" => Some(InputEvent::HomunculusFeed),
        "HomunculusRelease" => Some(InputEvent::HomunculusRelease),
        "MercenaryDismiss" => Some(InputEvent::MercenaryDismiss),
        "ToggleMvpMinimap" => Some(InputEvent::ToggleMvpMinimap),
        "OpenMail" => Some(InputEvent::OpenMail),
        "RefreshMail" => Some(InputEvent::RefreshMail),
        "NextMailPage" => Some(InputEvent::NextMailPage),
        "ReadMail" => Some(InputEvent::ReadMail),
        "TakeMailZeny" => Some(InputEvent::TakeMailZeny),
        "TakeMailItems" => Some(InputEvent::TakeMailItems),
        "DeleteMail" => Some(InputEvent::DeleteMail),
        "CloseMail" => Some(InputEvent::CloseMail),
        "OpenBank" => Some(InputEvent::OpenBank),
        "RefreshBank" => Some(InputEvent::RefreshBank),
        "DepositBank" => Some(InputEvent::DepositBank),
        "WithdrawBank" => Some(InputEvent::WithdrawBank),
        "BankAdd100k" => Some(InputEvent::BankAdd100k),
        "BankAdd1m" => Some(InputEvent::BankAdd1m),
        "BankAdd10m" => Some(InputEvent::BankAdd10m),
        "BankSetMax" => Some(InputEvent::BankSetMax),
        "CloseBank" => Some(InputEvent::CloseBank),
        "OpenRoulette" => Some(InputEvent::OpenRoulette),
        "RequestRouletteInfo" => Some(InputEvent::RequestRouletteInfo),
        "SpinRoulette" => Some(InputEvent::SpinRoulette),
        "ClaimRoulette" => Some(InputEvent::ClaimRoulette),
        "CloseRoulette" => Some(InputEvent::CloseRoulette),
        "CloseShop" => Some(InputEvent::CloseShop),
        "LeaveParty" => Some(InputEvent::LeaveParty),
        "AcceptPendingPartyInvite" => Some(InputEvent::AcceptPendingPartyInvite),
        "PurchaseBarter" => Some(InputEvent::PurchaseBarter),
        "CloseBarter" => Some(InputEvent::CloseBarter),
        "AcceptTrade" => Some(InputEvent::AcceptTrade),
        "DeclineTrade" => Some(InputEvent::DeclineTrade),
        "TradeConfirm" => Some(InputEvent::TradeConfirm),
        "TradeDeal" => Some(InputEvent::TradeDeal),
        "TradeCancel" => Some(InputEvent::TradeCancel),
        "RefineClose" => Some(InputEvent::RefineClose),
        "ConfirmItemSkillSelection" => Some(InputEvent::ConfirmItemSkillSelection),
        "CancelItemSkillSelection" => Some(InputEvent::CancelItemSkillSelection),
        "ItemReformSubmit" => Some(InputEvent::ItemReformSubmit),
        "ItemReformClose" => Some(InputEvent::ItemReformClose),
        "GradeEnchantSelect" => Some(InputEvent::GradeEnchantSelect),
        "GradeEnchantSubmit" => Some(InputEvent::GradeEnchantSubmit),
        "GradeEnchantClose" => Some(InputEvent::GradeEnchantClose),
        "LaphineSynthesisAdd" => Some(InputEvent::LaphineSynthesisAdd),
        "LaphineSynthesisClear" => Some(InputEvent::LaphineSynthesisClear),
        "LaphineSynthesisSubmit" => Some(InputEvent::LaphineSynthesisSubmit),
        "LaphineSynthesisClose" => Some(InputEvent::LaphineSynthesisClose),
        "LaphineUpgradeSubmit" => Some(InputEvent::LaphineUpgradeSubmit),
        "LaphineUpgradeClose" => Some(InputEvent::LaphineUpgradeClose),
        _ => None,
    };
    if event.is_some() {
        return event;
    }
    #[cfg(feature = "debug")]
    {
        match name {
            "ReloadLanguage" => Some(InputEvent::ReloadLanguage),
            "SaveLanguage" => Some(InputEvent::SaveLanguage),
            "ToggleRenderOptionsWindow" => Some(InputEvent::ToggleRenderOptionsWindow),
            "OpenMapDataWindow" => Some(InputEvent::OpenMapDataWindow),
            "ToggleClientStateInspectorWindow" => Some(InputEvent::ToggleClientStateInspectorWindow),
            "ToggleMapsWindow" => Some(InputEvent::ToggleMapsWindow),
            "ToggleCommandsWindow" => Some(InputEvent::ToggleCommandsWindow),
            "ToggleThemeInspectorWindow" => Some(InputEvent::ToggleThemeInspectorWindow),
            "ToggleProfilerWindow" => Some(InputEvent::ToggleProfilerWindow),
            "TogglePacketInspectorWindow" => Some(InputEvent::TogglePacketInspectorWindow),
            "ToggleCacheStatisticsWindow" => Some(InputEvent::ToggleCacheStatisticsWindow),
            "CameraMoveForward" => Some(InputEvent::CameraMoveForward),
            "CameraMoveBackward" => Some(InputEvent::CameraMoveBackward),
            "CameraMoveLeft" => Some(InputEvent::CameraMoveLeft),
            "CameraMoveRight" => Some(InputEvent::CameraMoveRight),
            "CameraMoveUp" => Some(InputEvent::CameraMoveUp),
            "CameraAccelerate" => Some(InputEvent::CameraAccelerate),
            "CameraDecelerate" => Some(InputEvent::CameraDecelerate),
            _ => None,
        }
    }
    #[cfg(not(feature = "debug"))]
    {
        let _ = name;
        None
    }
}

pub fn toggle_event(name: &str) -> Result<InputEvent, String> {
    if !(name.starts_with("Toggle") || name.starts_with("Open")) {
        return Err(format!("toggle only accepts Toggle* or Open* events, not {name}"));
    }
    unit_event(name).ok_or_else(|| format!("unknown unit event {name} (Open*/Toggle* with fields use `event`)"))
}

#[derive(Deserialize)]
enum ScriptedEvent {
    SendMessage(String),
    PlayerMove {
        x: u16,
        y: u16,
    },
    SelectCharacter(usize),
    CreateCharacter {
        slot: usize,
        name: String,
    },
    OpenCharacterCreationWindow(usize),
    CastSkill(u8),
    StopSkill(u8),
    UseItem(u16),
    SelectItemSkillRow(u32),
    ZoomCamera(f32),
    RotateCamera(f32),
    SendEmotion(u8),
    StatUpStrength(u8),
    StatUpAgility(u8),
    StatUpVitality(u8),
    StatUpIntelligence(u8),
    StatUpDexterity(u8),
    StatUpLuck(u8),
    RequestStableHomunculus {
        homunculus_id: i32,
    },
    RequestStablePet {
        action: i16,
        id: i32,
    },
    CreateParty {
        name: String,
        item_pickup: bool,
        item_division: bool,
    },
    InviteToParty(String),
}

pub fn scripted_event(spec: &str) -> Result<InputEvent, String> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Err("event needs an InputEvent name or RON".to_owned());
    }
    if spec.starts_with("LogIn") {
        return Err("LogIn is not accepted on the command channel".to_owned());
    }
    if let Some(event) = unit_event(spec) {
        return Ok(event);
    }
    let parsed: ScriptedEvent = ron::from_str(spec)
        .map_err(|error| format!("unsupported event {spec}: {error}. InputEvent has no serde; see docs/autopilot.md"))?;
    Ok(match parsed {
        ScriptedEvent::SendMessage(text) => InputEvent::SendMessage { text },
        ScriptedEvent::PlayerMove { x, y } => InputEvent::PlayerMove {
            destination: TilePosition { x, y },
        },
        ScriptedEvent::SelectCharacter(slot) => InputEvent::SelectCharacter { slot },
        ScriptedEvent::CreateCharacter { slot, name } => InputEvent::CreateCharacter { slot, name },
        ScriptedEvent::OpenCharacterCreationWindow(slot) => InputEvent::OpenCharacterCreationWindow { slot },
        ScriptedEvent::CastSkill(slot) => InputEvent::CastSkill {
            slot: HotbarSlot(u16::from(slot)),
        },
        ScriptedEvent::StopSkill(slot) => InputEvent::StopSkill {
            slot: HotbarSlot(u16::from(slot)),
        },
        ScriptedEvent::UseItem(index) => InputEvent::UseItem {
            index: InventoryIndex(index),
        },
        ScriptedEvent::SelectItemSkillRow(index) => InputEvent::SelectItemSkillRow { index },
        ScriptedEvent::ZoomCamera(zoom_factor) => InputEvent::ZoomCamera { zoom_factor },
        ScriptedEvent::RotateCamera(rotation) => InputEvent::RotateCamera { rotation },
        ScriptedEvent::SendEmotion(emotion) => InputEvent::SendEmotion { emotion },
        ScriptedEvent::StatUpStrength(amount) => InputEvent::StatUp {
            stat_type: StatUpType::Strength { amount },
        },
        ScriptedEvent::StatUpAgility(amount) => InputEvent::StatUp {
            stat_type: StatUpType::Agility { amount },
        },
        ScriptedEvent::StatUpVitality(amount) => InputEvent::StatUp {
            stat_type: StatUpType::Vitality { amount },
        },
        ScriptedEvent::StatUpIntelligence(amount) => InputEvent::StatUp {
            stat_type: StatUpType::Intelligence { amount },
        },
        ScriptedEvent::StatUpDexterity(amount) => InputEvent::StatUp {
            stat_type: StatUpType::Dexterity { amount },
        },
        ScriptedEvent::StatUpLuck(amount) => InputEvent::StatUp {
            stat_type: StatUpType::Luck { amount },
        },
        ScriptedEvent::RequestStableHomunculus { homunculus_id } => InputEvent::RequestStableHomunculus { homunculus_id },
        ScriptedEvent::RequestStablePet { action, id } => InputEvent::RequestStablePet { action, id },
        ScriptedEvent::CreateParty {
            name,
            item_pickup,
            item_division,
        } => InputEvent::CreateParty {
            name,
            item_pickup,
            item_division,
        },
        ScriptedEvent::InviteToParty(character_name) => InputEvent::InviteToParty { character_name },
    })
}

pub fn parse_window_class(name: &str) -> Option<WindowClass> {
    match name.trim().to_ascii_lowercase().as_str() {
        "audiosettings" => Some(WindowClass::AudioSettings),
        "bank" => Some(WindowClass::Bank),
        "barter" => Some(WindowClass::Barter),
        "buy" => Some(WindowClass::Buy),
        "buycart" => Some(WindowClass::BuyCart),
        "cart" => Some(WindowClass::Cart),
        "buyorsell" => Some(WindowClass::BuyOrSell),
        "chat" => Some(WindowClass::Chat),
        "companions" => Some(WindowClass::Companions),
        "charactercreation" => Some(WindowClass::CharacterCreation),
        "characteroverview" => Some(WindowClass::CharacterOverview),
        "characterselection" => Some(WindowClass::CharacterSelection),
        "dialog" => Some(WindowClass::Dialog),
        "enchant" => Some(WindowClass::Enchant),
        "eden" => Some(WindowClass::Eden),
        "gamesettings" => Some(WindowClass::GameSettings),
        "interfacesettings" => Some(WindowClass::InterfaceSettings),
        "graphicssettings" => Some(WindowClass::GraphicsSettings),
        "hotbar" => Some(WindowClass::Hotbar),
        "emotes" => Some(WindowClass::Emotes),
        "keybinds" | "hotkeys" => Some(WindowClass::Keybinds),
        "menubar" => Some(WindowClass::MenuBar),
        "inventory" => Some(WindowClass::Inventory),
        "storage" => Some(WindowClass::Storage),
        "equipment" | "gear" => Some(WindowClass::Equipment),
        "switchequip" => Some(WindowClass::SwitchEquip),
        "skilltree" => Some(WindowClass::SkillTree),
        "stable" => Some(WindowClass::Stable),
        "warpra" => Some(WindowClass::Warpra),
        "stats" => Some(WindowClass::Stats),
        "statuseffects" => Some(WindowClass::StatusEffects),
        "questjournal" => Some(WindowClass::QuestJournal),
        "achievements" => Some(WindowClass::Achievements),
        "navigation" => Some(WindowClass::Navigation),
        "friendlist" => Some(WindowClass::FriendList),
        "friendrequest" => Some(WindowClass::FriendRequest),
        "guild" => Some(WindowClass::Guild),
        "guildinvite" => Some(WindowClass::GuildInvite),
        "clan" => Some(WindowClass::Clan),
        "partyinvite" => Some(WindowClass::PartyInvite),
        "login" => Some(WindowClass::Login),
        "menu" => Some(WindowClass::Menu),
        "worldmap" => Some(WindowClass::WorldMap),
        "mail" => Some(WindowClass::Mail),
        "mvpminimap" => Some(WindowClass::MvpMinimap),
        "respawn" => Some(WindowClass::Respawn),
        "roulette" => Some(WindowClass::Roulette),
        "selectserver" => Some(WindowClass::SelectServer),
        "sell" => Some(WindowClass::Sell),
        "sellcart" => Some(WindowClass::SellCart),
        "trade" => Some(WindowClass::Trade),
        "traderequest" => Some(WindowClass::TradeRequest),
        "vendor" => Some(WindowClass::Vendor),
        "searchstore" => Some(WindowClass::SearchStore),
        "refine" => Some(WindowClass::Refine),
        "itemskillselection" => Some(WindowClass::ItemSkillSelection),
        "itemfinder" => Some(WindowClass::ItemFinder),
        "itemreform" => Some(WindowClass::ItemReform),
        "enchantgrade" => Some(WindowClass::EnchantGrade),
        "laphinesynthesis" => Some(WindowClass::LaphineSynthesis),
        "laphineupgrade" => Some(WindowClass::LaphineUpgrade),
        #[cfg(feature = "debug")]
        "maps" => Some(WindowClass::Maps),
        #[cfg(feature = "debug")]
        "clientstateinspector" => Some(WindowClass::ClientStateInspector),
        #[cfg(feature = "debug")]
        "packetinspector" => Some(WindowClass::PacketInspector),
        #[cfg(feature = "debug")]
        "renderoptions" => Some(WindowClass::RenderOptions),
        #[cfg(feature = "debug")]
        "commands" => Some(WindowClass::Commands),
        #[cfg(feature = "debug")]
        "themeinspector" => Some(WindowClass::ThemeInspector),
        #[cfg(feature = "debug")]
        "profiler" => Some(WindowClass::Profiler),
        #[cfg(feature = "debug")]
        "cachestatistics" => Some(WindowClass::CacheStatistics),
        _ => None,
    }
}

pub fn window_command(class: WindowClass, open: bool) -> Result<WindowCommand, String> {
    let event = match class {
        WindowClass::AudioSettings => InputEvent::ToggleAudioSettingsWindow,
        WindowClass::GameSettings => InputEvent::ToggleGameSettingsWindow,
        WindowClass::InterfaceSettings => InputEvent::ToggleInterfaceSettingsWindow,
        WindowClass::GraphicsSettings => InputEvent::ToggleGraphicsSettingsWindow,
        WindowClass::Inventory => InputEvent::ToggleInventoryWindow,
        WindowClass::Cart => InputEvent::ToggleCartWindow,
        WindowClass::Equipment => InputEvent::ToggleEquipmentWindow,
        WindowClass::SwitchEquip => InputEvent::ToggleSwitchEquipWindow,
        WindowClass::SkillTree => InputEvent::ToggleSkillTreeWindow,
        WindowClass::Stats => InputEvent::ToggleStatsWindow,
        WindowClass::FriendList => InputEvent::ToggleFriendListWindow,
        WindowClass::Guild => InputEvent::ToggleGuildWindow,
        WindowClass::GuildInvite => return Err("guild invite opens from the server".to_owned()),
        WindowClass::Clan => InputEvent::ToggleClanWindow,
        WindowClass::ItemFinder => InputEvent::ToggleItemFinderWindow,
        WindowClass::QuestJournal => InputEvent::ToggleQuestJournalWindow,
        WindowClass::Achievements => InputEvent::ToggleAchievementsWindow,
        WindowClass::Navigation => InputEvent::ToggleNavigationWindow,
        WindowClass::StatusEffects => InputEvent::ToggleStatusEffectsWindow,
        WindowClass::Menu => InputEvent::ToggleMenuWindow,
        WindowClass::WorldMap => InputEvent::ToggleWorldMap,
        WindowClass::Chat => InputEvent::ToggleChatWindow,
        WindowClass::Hotbar => InputEvent::ToggleHotbarWindow,
        WindowClass::Emotes => InputEvent::ToggleEmotesWindow,
        WindowClass::Keybinds => InputEvent::ToggleKeybindsWindow,
        WindowClass::CharacterOverview => InputEvent::ToggleCharacterOverviewWindow,
        WindowClass::MenuBar => return Err("the menu bar stays open".to_owned()),
        WindowClass::MvpMinimap => InputEvent::ToggleMvpMinimap,
        WindowClass::Bank => {
            if open {
                InputEvent::OpenBank
            } else {
                InputEvent::CloseBank
            }
        }
        WindowClass::Barter => {
            if open {
                return Err("barter opens from the server".to_owned());
            }
            InputEvent::CloseBarter
        }
        WindowClass::Roulette => {
            if open {
                InputEvent::OpenRoulette
            } else {
                InputEvent::CloseRoulette
            }
        }
        WindowClass::Mail => {
            if open {
                InputEvent::OpenMail
            } else {
                InputEvent::CloseMail
            }
        }
        WindowClass::Storage => {
            if open {
                return Err("storage opens from the server; close uses event CloseStorage".to_owned());
            }
            InputEvent::CloseStorage
        }
        WindowClass::Enchant => {
            if open {
                return Err("enchant opens from the server".to_owned());
            }
            InputEvent::CloseEnchant
        }
        WindowClass::Stable => {
            if !open {
                return Err("stable has no close event".to_owned());
            }
            InputEvent::OpenStable
        }
        WindowClass::Eden => {
            if !open {
                return Err("eden has no close event".to_owned());
            }
            InputEvent::OpenEden
        }
        WindowClass::Warpra => {
            if !open {
                return Err("warpra has no close event".to_owned());
            }
            InputEvent::OpenWarpra
        }
        WindowClass::Companions => {
            if !open {
                return Err("companions has no close event".to_owned());
            }
            InputEvent::OpenCompanions
        }
        WindowClass::Refine => {
            if open {
                return Err("refine opens from the server".to_owned());
            }
            InputEvent::RefineClose
        }
        WindowClass::ItemSkillSelection => {
            if open {
                return Err("item skill selection opens from the server".to_owned());
            }
            InputEvent::CancelItemSkillSelection
        }
        WindowClass::ItemReform => {
            if open {
                return Err("item reform opens from the server".to_owned());
            }
            InputEvent::ItemReformClose
        }
        WindowClass::EnchantGrade => {
            if open {
                return Err("enchant grade opens from the server".to_owned());
            }
            InputEvent::GradeEnchantClose
        }
        WindowClass::LaphineSynthesis => {
            if open {
                return Err("laphine synthesis opens from the server".to_owned());
            }
            InputEvent::LaphineSynthesisClose
        }
        WindowClass::LaphineUpgrade => {
            if open {
                return Err("laphine upgrade opens from the server".to_owned());
            }
            InputEvent::LaphineUpgradeClose
        }
        #[cfg(feature = "debug")]
        WindowClass::RenderOptions => InputEvent::ToggleRenderOptionsWindow,
        #[cfg(feature = "debug")]
        WindowClass::ClientStateInspector => InputEvent::ToggleClientStateInspectorWindow,
        #[cfg(feature = "debug")]
        WindowClass::PacketInspector => InputEvent::TogglePacketInspectorWindow,
        #[cfg(feature = "debug")]
        WindowClass::Commands => InputEvent::ToggleCommandsWindow,
        #[cfg(feature = "debug")]
        WindowClass::Profiler => InputEvent::ToggleProfilerWindow,
        #[cfg(feature = "debug")]
        WindowClass::Maps => InputEvent::ToggleMapsWindow,
        #[cfg(feature = "debug")]
        WindowClass::CacheStatistics => InputEvent::ToggleCacheStatisticsWindow,
        #[cfg(feature = "debug")]
        WindowClass::ThemeInspector => InputEvent::ToggleThemeInspectorWindow,
        other => return Err(format!("no open/close command for {other:?}")),
    };
    Ok(WindowCommand { class, open, event })
}
