#![allow(incomplete_features)]
#![feature(adt_const_params)]
#![feature(allocator_api)]
#![feature(generic_const_exprs)]
#![feature(iter_next_chunk)]
#![feature(negative_impls)]
#![feature(proc_macro_hygiene)]
#![feature(random)]
#![feature(type_changing_struct_update)]
#![feature(unsized_const_params)]
#![feature(variant_count)]
#![feature(anonymous_lifetime_in_impl_trait)]
#![feature(associated_type_defaults)]
#![feature(macro_metavar_expr)]
#![feature(unsafe_cell_access)]
#![feature(impl_trait_in_assoc_type)]
#![feature(thread_local)]

// Helper macro to time and print the startup time of Korangar
macro_rules! time_phase {
    ($message:expr, { $($statements:tt)* }) => {
        #[cfg(feature = "debug")]
        let _statement_timer = korangar_debug::logging::Timer::new($message);

        $($statements)*

        #[cfg(feature = "debug")]
        _statement_timer.stop();
    }
}

mod autopilot;
pub mod deployment;
mod graphics;
mod input;
mod state;
#[macro_use]
mod interface;
mod loaders;
#[cfg(feature = "debug")]
mod networking;
mod renderer;
mod settings;
mod system;
mod world;

use std::io::Cursor;
use std::net::{SocketAddr, ToSocketAddrs};
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Instant;

use cgmath::{Point3, Vector3};
use image::{EncodableLayout, ImageFormat, ImageReader};
use input::{MouseInputMode, MouseModeExt};
use korangar_audio::{AudioEngine, SoundEffectKey};
#[cfg(feature = "debug")]
use korangar_debug::logging::{Colorize, print_debug};
#[cfg(feature = "debug")]
use korangar_debug::profile_block;
#[cfg(feature = "debug")]
use korangar_debug::profiling::Profiler;
use korangar_interface::Interface;
use korangar_interface::layout::MouseButton;
use korangar_networking::{
    DialogHotkey, DisconnectReason, HotkeyState, InventoryItemDetails, ItemQuantity, LoginServerLoginData, MessageColor, NetworkEvent,
    NetworkEventBuffer, NetworkingSystem, PartyUpdate, SellItem, StatusChangeState, SupportedPacketVersion, cutin_anchor, dialog_hotkey,
    existing_cutin_texture,
};
#[cfg(feature = "debug")]
use networking::{PacketHistory, PacketHistoryCallback};
#[cfg(not(feature = "debug"))]
use ragnarok_packets::handler::NoPacketCallback;
use ragnarok_packets::{
    AccountId, AttackRange, BuyShopItemsResult, CharacterServerInformation, ClientTick, Direction, DisappearanceReason, EffectId, EntityId,
    EquipPosition, EquippableItemFlags, ExperienceSource, ExperienceType, GuildPositionEdit, HealType, HotbarSlot, InventoryIndex,
    SellItemsResult, SkillId, SkillLevel, SkillType, TilePosition, WorldPosition,
};
use renderer::InterfaceRenderer;
use rust_state::{ManuallyAssertExt, State};
#[cfg(feature = "debug")]
use rust_state::{VecIndexExt, VecLookupExt};
use settings::{
    AudioSettings, AudioSettingsPathExt, GraphicsSettingsCapabilities, GraphicsSettingsPathExt, InterfaceSettings,
    InterfaceSettingsPathExt, session_theme_name,
};
use state::character_slots::CharacterSlotsExt;
use state::equip_switch::{EquipSwitchPathExt, switch_delta};
use state::hotbar::HotbarPathExt;
use state::inventory::{GroundDrop, InventoryPathExt, ItemActivation, ThrowUpdate, ground_drop, item_activation, use_ack_targets_self};
use state::localization::Localization;
use state::skills::SkillTreePathExt;
use state::theme::{CursorThemePathExt, IndicatorThemePathExt, InterfaceThemePathExt, WorldThemePathExt};
use state::trade::{TradeInventoryOffer, TradeOfferLine};
use state::{ChatMessage, ClientState, ClientStatePathExt, client_state, this_entity, this_player};
#[cfg(feature = "debug")]
use wgpu::Device;
use wgpu::{DeviceDescriptor, ExperimentalFeatures, Instance, MemoryHints, Queue, Trace};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{ModifiersState, PhysicalKey};
use winit::window::{Icon, Window, WindowId};

use crate::graphics::*;
use crate::input::{InputEvent, InputReport, InputSystem};
use crate::interface::cursor::{MouseCursor, MouseCursorState};
use crate::interface::resource::{ItemSource, SkillSource};
use crate::interface::windows::*;
use crate::loaders::*;
#[cfg(feature = "debug")]
use crate::renderer::{AlignHorizontal, DebugMarkerRenderer};
use crate::renderer::{EffectRenderer, GameInterfaceRenderer};
use crate::settings::{
    GameSettingsPathExt, GraphicsSettings, IN_GAME_THEMES_PATH, LightingMode, MENU_THEMES_PATH, ServiceSettingsPathExt, WORLD_THEMES_PATH,
};
use crate::state::skills::{LearnedSkill, SkillTreeLayoutPathExt, bring_skill_to_level};
use crate::state::theme::{InterfaceTheme, InterfaceThemeType, WorldTheme};
use crate::state::{BufferedAction, SelectedServicePath};
use crate::system::{FrameTimers, GameTimer};
#[cfg(feature = "debug")]
use crate::world::MarkerIdentifier;
use crate::world::effects::{self, SkillField};
use crate::world::*;

const CLIENT_NAME: &str = "Korangar";
const ROLLING_CUTTER_ID: SkillId = SkillId(2036);
const DEFAULT_MAP: &str = "geffen";
/// Screenshot login. Set `KORANGAR_HEADLESS_AUTO=1` to log in, pick the first
/// character server, enter slot 0, and open Gear. It does not open Friends,
/// standalone Stats, Character Overview, Create Character, or the menu.
/// See `docs/headless-auto.md`.
static HEADLESS_AUTO_LOGIN_DONE: AtomicBool = AtomicBool::new(false);
static HEADLESS_AUTO_INWORLD_UI: AtomicBool = AtomicBool::new(false);

fn headless_auto() -> bool {
    std::env::var_os("KORANGAR_HEADLESS_AUTO").is_some()
}

fn finish_barter_purchase<Callback>(
    networking_system: &mut NetworkingSystem<Callback>,
    state: &mut State<ClientState>,
    interface: &mut Interface<'static, ClientState>,
    result: u8,
) where
    Callback: ragnarok_packets::handler::PacketCallback + Send,
{
    let expanded = state.follow(client_state().barter()).is_expanded();
    let color = match korangar_networking::barter_result_ok(result) {
        true => MessageColor::Information,
        false => MessageColor::Error,
    };
    state.follow_mut(client_state().chat_messages()).push(ChatMessage::new(
        korangar_networking::barter_result_text(result).to_owned(),
        color,
    ));
    let _ = networking_system.close_barter(expanded);
    state.follow_mut(client_state().barter()).mark_button_close();
    interface.close_window_with_class(WindowClass::Barter);
}

fn autopilot_println(line: &str) {
    println!("{line}");
    let _ = std::io::Write::flush(&mut std::io::stdout());
}
const START_CAMERA_FOCUS_POINT: Point3<f32> = Point3::new(600.0, 0.0, 240.0);
const DEFAULT_BACKGROUND_MUSIC: Option<&str> = Some("bgm\\01.mp3");
const MAIN_MENU_CLICK_SOUND_EFFECT: &str = "버튼소리.wav";
const ITEM_PICKUP_RANGE: AttackRange = AttackRange(1);
// TODO: The number of point lights that can cast shadows should be configurable
// through the graphics settings. For now I just chose an arbitrary smaller
// number that should be playable on most devices.
const NUMBER_OF_POINT_LIGHTS_WITH_SHADOWS: usize = 6;

enum LoadedInterfaceTexture {
    Image(Arc<Texture>),
    /// A local file was present but did not read. Do not record the path.
    Retry,
    /// Both local candidates missed and the remote read returned nothing.
    Missing,
}

const INITIAL_SCREEN_SIZE: ScreenSize = ScreenSize {
    width: 1280.0,
    height: 720.0,
};

const INITIAL_SCALING_FACTOR: Scaling = Scaling::new(1.0);
const FALLBACK_PACKET_VERSION: SupportedPacketVersion = SupportedPacketVersion::_20220406;

static ICON_DATA: &[u8] = include_bytes!("../archive/data/icon.png");

/// CTR+C was sent, and the client is supposed to close.
pub static SHUTDOWN_SIGNAL: LazyLock<AtomicBool> = LazyLock::new(|| AtomicBool::new(false));

#[cfg(feature = "debug")]
const DEBUG_WINDOWS: &[WindowClass] = &[
    WindowClass::CacheStatistics,
    WindowClass::ClientStateInspector,
    WindowClass::PacketInspector,
    WindowClass::Profiler,
    WindowClass::RenderOptions,
];

// Create the `threads` module.
#[cfg(feature = "debug")]
korangar_debug::create_profiler_threads!(threads, {
    Main,
    Loader,
});

pub fn init_tls_rand() {
    use std::random::*;
    let mut seed = [0; 32];
    DefaultRandomSource.fill_bytes(&mut seed);
    rand_aes::tls::rand_seed(seed.into());
}

fn cart_move_amount(item: &korangar_networking::InventoryItem<crate::world::ResourceMetadata>) -> i32 {
    match &item.details {
        korangar_networking::InventoryItemDetails::Regular { amount, .. } => i32::from(*amount),
        korangar_networking::InventoryItemDetails::Equippable { .. } => 1,
    }
}

fn initialize_shutdown_signal() {
    ctrlc::set_handler(|| {
        println!("CTRL-C received. Shutting down");
        SHUTDOWN_SIGNAL.store(true, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");
}

pub struct Client {
    event_loop: Option<EventLoop<()>>,
    startup_error: Option<String>,
    keyboard_modifiers: ModifiersState,
    game_file_loader: Arc<GameFileLoader>,
    action_loader: Arc<ActionLoader>,
    #[cfg(feature = "debug")]
    animation_loader: Arc<AnimationLoader>,
    async_loader: Arc<AsyncLoader>,
    effect_loader: Arc<EffectLoader>,
    font_loader: Arc<FontLoader>,
    #[cfg(feature = "debug")]
    map_loader: Arc<MapLoader>,
    sprite_loader: Arc<SpriteLoader>,
    texture_loader: Arc<TextureLoader>,
    library: Arc<Library>,

    interface_renderer: InterfaceRenderer,
    bottom_interface_renderer: GameInterfaceRenderer,
    middle_interface_renderer: GameInterfaceRenderer,
    top_interface_renderer: GameInterfaceRenderer,
    effect_renderer: EffectRenderer,
    #[cfg(feature = "debug")]
    debug_marker_renderer: DebugMarkerRenderer,
    #[cfg(feature = "debug")]
    aabb_instructions: Vec<DebugAabbInstruction>,
    #[cfg(feature = "debug")]
    circle_instructions: Vec<DebugCircleInstruction>,
    #[cfg(feature = "debug")]
    rectangle_instructions: Vec<DebugRectangleInstruction>,
    model_batches: Vec<ModelBatch>,
    model_instructions: Vec<ModelInstruction>,
    entity_instructions: Vec<EntityInstruction>,
    directional_shadow_model_batches: [Vec<ModelBatch>; PARTITION_COUNT],
    directional_shadow_model_instructions: Vec<ModelInstruction>,
    directional_shadow_entity_instructions: [Vec<EntityInstruction>; PARTITION_COUNT],
    point_shadow_model_batches: Vec<ModelBatch>,
    point_shadow_model_instructions: Vec<ModelInstruction>,
    point_shadow_entity_instructions: Vec<EntityInstruction>,
    point_light_with_shadow_instructions: Vec<PointLightWithShadowInstruction>,
    point_light_instructions: Vec<PointLightInstruction>,

    input_system: InputSystem,

    interface: Interface<'static, ClientState>,
    mouse_cursor: MouseCursor,
    show_interface: bool,
    game_timer: GameTimer,

    #[cfg(feature = "debug")]
    debug_camera: DebugCamera,
    start_camera: StartCamera,
    player_camera: PlayerCamera,
    directional_shadow_camera: DirectionalShadowCamera,
    directional_shadow_partitions: Arc<Mutex<[DirectionalShadowPartition; PARTITION_COUNT]>>,
    point_shadow_camera: PointShadowCamera,

    input_event_buffer: Vec<InputEvent>,
    autopilot: Option<autopilot::Autopilot>,
    network_event_buffer: NetworkEventBuffer,
    // TODO: Move or remove this.
    saved_login_data: Option<LoginServerLoginData>,
    // TODO: Move or remove this.
    saved_character_server: Option<CharacterServerInformation>,
    // TODO: Move or remove this.
    saved_login_server_address: Option<SocketAddr>,
    // TODO: Move or remove this.
    saved_password: String,
    // TODO: Move or remove this.
    saved_username: String,
    // TODO: Move or remove this.
    saved_packet_version: SupportedPacketVersion,

    particle_holder: ParticleHolder,
    point_light_manager: PointLightManager,
    effect_holder: EffectHolder,
    cast_bars: effects::CastBars,
    path_finder: PathFinder,

    point_light_set_buffer: ResourceSetBuffer<LightSourceKey>,
    directional_shadow_object_set_buffer: ResourceSetBuffer<ObjectKey>,
    point_shadow_object_set_buffer: ResourceSetBuffer<ObjectKey>,
    deferred_object_set_buffer: ResourceSetBuffer<ObjectKey>,
    #[cfg(feature = "debug")]
    bounding_box_object_set_buffer: ResourceSetBuffer<ObjectKey>,

    #[cfg(feature = "debug")]
    pathing_texture_set: Arc<TextureSet>,
    #[cfg(feature = "debug")]
    tile_texture_set: Arc<TextureSet>,

    main_menu_click_sound_effect: SoundEffectKey,

    #[cfg(feature = "debug")]
    networking_system: NetworkingSystem<PacketHistoryCallback>,
    #[cfg(not(feature = "debug"))]
    networking_system: NetworkingSystem<NoPacketCallback>,
    audio_engine: Arc<AudioEngine<GameFileLoader>>,
    active_interface_settings: InterfaceSettings,
    /// Last loaded theme names. Plain strings so a headless override is never
    /// stored on [`InterfaceSettings`], which saves itself on drop.
    loaded_in_game_theme: String,
    loaded_world_theme: String,
    loaded_in_game_theme_mtime: Option<std::time::SystemTime>,
    loaded_world_theme_mtime: Option<std::time::SystemTime>,
    force_reload_themes: bool,
    active_graphics_settings: GraphicsSettings,
    graphics_engine: GraphicsEngine,
    queue: Queue,
    #[cfg(feature = "debug")]
    device: Device,
    window: Option<Arc<Window>>,

    map: Option<Arc<Map>>,
    client_state: State<ClientState>,
}

fn str_effect_light(path: &str) -> (Color, f32) {
    if path.starts_with("firewall") {
        (Color::rgb_u8(255, 30, 0), 60.0)
    } else if path.starts_with("pneuma") {
        (Color::rgb_u8(83, 220, 108), 40.0)
    } else {
        (Color::WHITE, 50.0)
    }
}

fn load_item_finder(loader: &loaders::GameFileLoader, state: &mut rust_state::State<state::ClientState>) {
    let catalog = korangar_loaders::FileLoader::get(loader, state::item_finder::CATALOG_PATH).ok();
    let sources = korangar_loaders::FileLoader::get(loader, state::item_finder::SOURCES_PATH).ok();
    state
        .follow_mut(client_state().item_finder())
        .load_bytes(catalog.as_deref(), sources.as_deref());
}

impl Client {
    /// Check native configuration and verified asset-source initialization
    /// without creating a window/GPU or authenticating. This is not
    /// completeness/render validation.
    pub fn check_bootstrap() -> Result<(), String> {
        let loader = loaders::GameFileLoader::default();
        loader.load_archives_from_settings()?;
        let _ = loaders::try_load_client_info(&loader)?;
        let mut checked = 0;
        macro_rules! sample {
            ($path:literal, $kind:ty, $metadata:ty) => {
                if loader.file_exists($path) {
                    let bytes = korangar_loaders::FileLoader::get(&loader, $path)
                        .map_err(|error| format!("Representative asset unavailable: {error:?}"))?;
                    let mut reader = ragnarok_bytes::ByteReader::with_default_metadata::<$metadata>(&bytes);
                    <$kind as ragnarok_bytes::FromBytes>::from_bytes(&mut reader)
                        .map_err(|_| format!("Representative asset format is invalid or unsupported: {}", $path))?;
                    if !reader.is_empty() {
                        return Err(format!("Unparsed representative asset bytes: {}", $path));
                    }
                    checked += 1;
                }
            };
        }
        sample!(
            "data/izlude.rsw",
            ragnarok_formats::map::MapData,
            ragnarok_formats::version::MapFormatMetadata
        );
        sample!(
            "data/izlude.gnd",
            ragnarok_formats::map::GroundData,
            ragnarok_formats::version::MapFormatMetadata
        );
        sample!(
            "data/izlude.gat",
            ragnarok_formats::map::GatData,
            ragnarok_formats::version::MapFormatMetadata
        );
        sample!(
            "data/prontera.rsw",
            ragnarok_formats::map::MapData,
            ragnarok_formats::version::MapFormatMetadata
        );
        sample!(
            "data/prontera.gnd",
            ragnarok_formats::map::GroundData,
            ragnarok_formats::version::MapFormatMetadata
        );
        sample!(
            "data/prontera.gat",
            ragnarok_formats::map::GatData,
            ragnarok_formats::version::MapFormatMetadata
        );
        sample!(
            "data/sprite/npc/4_f_kafra1.spr",
            ragnarok_formats::sprite::SpriteData,
            ragnarok_formats::version::GenericFormatMetadata
        );
        sample!(
            "data/sprite/npc/4_f_kafra1.act",
            ragnarok_formats::action::ActionsData,
            ragnarok_formats::version::GenericFormatMetadata
        );
        sample!(
            "data/texture/effect/angel.str",
            ragnarok_formats::effect::EffectData,
            ragnarok_formats::version::GenericFormatMetadata
        );
        println!("Representative assets parsed: {checked} (not a complete dependency inventory)");
        Ok(())
    }

    pub fn init(sync_cache: bool) -> Option<Self> {
        // We start a frame so that functions trying to start a measurement don't panic.
        #[cfg(feature = "debug")]
        let _measurement = threads::Main::start_frame();

        initialize_shutdown_signal();

        time_phase!("create global thread pool", {
            rayon::ThreadPoolBuilder::new()
                .num_threads(4)
                .start_handler(|_| init_tls_rand())
                .build_global()
                .unwrap();
        });

        time_phase!("seed main random instance", {
            init_tls_rand();
        });

        // Check if korangar is in the correct working directory and if not, try to
        // correct it.
        // NOTE: This check might be temporary or feature gated in the future.
        time_phase!("adjust working directory", {
            if !std::fs::metadata("archive").is_ok_and(|metadata| metadata.is_dir()) {
                #[cfg(feature = "debug")]
                print_debug!(
                    "[{}] failed to find archive directory, attempting to change working directory {}",
                    "warning".yellow(),
                    "korangar".magenta()
                );

                if let Err(_error) = std::env::set_current_dir("korangar") {
                    #[cfg(feature = "debug")]
                    print_debug!("[{}] failed to change working directory: {:?}", "error".red(), _error);
                }
            }
        });

        time_phase!("load graphics settings", {
            let picker_value = Arc::new(AtomicU64::new(0));
            let directional_shadow_partitions = Arc::new(Mutex::new([DirectionalShadowPartition::default(); PARTITION_COUNT]));
            let input_system = InputSystem::new(picker_value.clone());
            let graphics_settings = GraphicsSettings::new();
        });

        // GLES must connect to the same platform display used by the eventual
        // window, not an implicit surfaceless EGL display. Keep the event loop
        // for run() and let the instance own a clone of its display connection.
        let event_loop = match EventLoop::new() {
            Ok(event_loop) => event_loop,
            Err(error) => {
                eprintln!("Cannot create native display event loop: {error}");
                return None;
            }
        };

        time_phase!("create adapter", {
            let instance = Instance::new(graphics::window_instance_descriptor(event_loop.owned_display_handle()));

            let adapter = match pollster::block_on(async { graphics::select_startup_adapter(&instance).await }) {
                Ok(adapter) => adapter,
                Err(error) => {
                    let summary = deployment::sanitize_graphics_diagnostic("adapter", &error);
                    eprintln!("{summary}");
                    deployment::report_failure(&summary, true);
                    return None;
                }
            };

            #[cfg(feature = "debug")]
            {
                let adapter_info = adapter.get_info();
                print_debug!("using adapter {} ({})", adapter_info.name, adapter_info.backend);
                print_debug!("using device {} ({})", adapter_info.device, adapter_info.vendor);
                print_debug!("using driver {} ({})", adapter_info.driver, adapter_info.driver_info);
                print_debug!("{}", graphics::adapter_diagnostic(&adapter));
            }
        });

        time_phase!("create device", {
            let capabilities = Capabilities::from_adapter(&adapter);

            let (device, queue) = match pollster::block_on(async {
                adapter
                    .request_device(&DeviceDescriptor {
                        label: None,
                        required_features: capabilities.get_required_features(),
                        required_limits: capabilities.get_required_limits(),
                        experimental_features: ExperimentalFeatures::disabled(),
                        memory_hints: MemoryHints::Performance,
                        trace: Trace::Off,
                    })
                    .await
            }) {
                Ok(pair) => pair,
                Err(error) => {
                    let detail = format!("{}; {}", graphics::adapter_diagnostic(&adapter), error);
                    let summary = deployment::sanitize_graphics_diagnostic("device", &detail);
                    eprintln!("{summary}");
                    deployment::report_failure(&summary, true);
                    return None;
                }
            };

            #[cfg(feature = "debug")]
            device.on_uncaptured_error(Arc::new(error_handler));

            #[cfg(feature = "debug")]
            print_debug!("received {} and {}", "queue".magenta(), "device".magenta());
        });

        time_phase!("create shader compiler", {
            let shader_compiler = ShaderCompiler::new(device.clone());
        });

        time_phase!("create game file loader", {
            let game_file_loader = Arc::new(GameFileLoader::default());

            if let Err(error) = game_file_loader.load_archives_from_settings() {
                eprintln!("Asset setup failed: {error}");
                return None;
            }
        });

        time_phase!("calculate game file hash", {
            let game_file_hash = game_file_loader.calculate_hash();
            #[cfg(feature = "debug")]
            print_debug!("game file hash: {}", game_file_hash);
        });

        time_phase!("create audio engine", {
            let audio_engine = Arc::new(AudioEngine::new(game_file_loader.clone()));
            audio_engine.set_background_music_volume(0.1);
        });

        time_phase!("create resource managers", {
            std::fs::create_dir_all(MENU_THEMES_PATH).unwrap();
            std::fs::create_dir_all(IN_GAME_THEMES_PATH).unwrap();
            std::fs::create_dir_all(WORLD_THEMES_PATH).unwrap();

            let model_loader = Arc::new(ModelLoader::new(game_file_loader.clone(), capabilities.bindless_support()));
            let texture_loader = Arc::new(TextureLoader::new(
                device.clone(),
                queue.clone(),
                &shader_compiler,
                &capabilities,
                game_file_loader.clone(),
            ));
            let video_loader = Arc::new(VideoLoader::new(game_file_loader.clone(), texture_loader.clone()));
            let font_loader = Arc::new(FontLoader::new(
                &["NotoSans".to_owned(), "NotoSansKR".to_owned(), "NotoSansJP".to_owned()],
                &game_file_loader,
                &device,
                &queue,
            ));
            let map_loader = Arc::new(MapLoader::new(
                device.clone(),
                queue.clone(),
                game_file_loader.clone(),
                audio_engine.clone(),
                capabilities.bindless_support(),
            ));
            let sprite_loader = Arc::new(SpriteLoader::new(game_file_loader.clone(), texture_loader.clone()));
            let action_loader = Arc::new(ActionLoader::new(game_file_loader.clone(), audio_engine.clone()));
            let effect_loader = Arc::new(EffectLoader::new(game_file_loader.clone()));
            let animation_loader = Arc::new(AnimationLoader::new());

            // Load from the selected sources, never from a stale rewritten Lua overlay.
            // Bytecode ABI adaptation happens in memory; translations stay untouched.
            let library = match Library::new(&game_file_loader) {
                Ok(library) => Arc::new(library),
                Err(error) => {
                    eprintln!("Lua library initialization failed: {error}");
                    return None;
                }
            };

            if sync_cache {
                sync_cache_archive(&game_file_loader, texture_loader, game_file_hash);
                return None;
            }

            game_file_loader.load_cache_archive(game_file_hash);

            let async_loader = Arc::new(AsyncLoader::new(
                action_loader.clone(),
                animation_loader.clone(),
                map_loader.clone(),
                model_loader.clone(),
                sprite_loader.clone(),
                texture_loader.clone(),
                video_loader.clone(),
                library.clone(),
            ));

            let interface_renderer = InterfaceRenderer::new(
                INITIAL_SCREEN_SIZE,
                font_loader.clone(),
                &texture_loader,
                graphics_settings.high_quality_interface,
            );
            let bottom_interface_renderer = GameInterfaceRenderer::new(
                INITIAL_SCREEN_SIZE,
                INITIAL_SCALING_FACTOR,
                font_loader.clone(),
                #[cfg(feature = "debug")]
                &texture_loader,
            );
            let middle_interface_renderer = GameInterfaceRenderer::from_renderer(&bottom_interface_renderer);
            let top_interface_renderer = GameInterfaceRenderer::from_renderer(&bottom_interface_renderer);
            let effect_renderer = EffectRenderer::new(INITIAL_SCREEN_SIZE);
            #[cfg(feature = "debug")]
            let debug_marker_renderer = DebugMarkerRenderer::new();

            #[cfg(feature = "debug")]
            let aabb_instructions = Vec::default();
            #[cfg(feature = "debug")]
            let circle_instructions = Vec::default();
            #[cfg(feature = "debug")]
            let rectangle_instructions = Vec::default();
            let model_batches = Vec::default();
            let model_instructions = Vec::default();
            let entity_instructions = Vec::default();
            let directional_shadow_model_batches = Default::default();
            let directional_shadow_model_instructions = Vec::default();
            let directional_shadow_entity_instructions = Default::default();
            let point_shadow_model_batches = Vec::default();
            let point_shadow_model_instructions = Vec::default();
            let point_shadow_entity_instructions = Vec::default();
            let point_light_with_shadow_instructions = Vec::default();
            let point_light_instructions = Vec::default();
        });

        time_phase!("create graphics engine", {
            let graphics_engine = GraphicsEngine::initialize(GraphicsEngineDescriptor {
                capabilities,
                adapter,
                instance,
                device: device.clone(),
                queue: queue.clone(),
                shader_compiler,
                texture_loader: texture_loader.clone(),
                picker_value,
                directional_shadow_partitions: directional_shadow_partitions.clone(),
            });
        });

        time_phase!("initialize interface", {
            let mut interface = Interface::new(font_loader.clone(), INITIAL_SCREEN_SIZE);
            let mouse_cursor = MouseCursor::new(&sprite_loader, &action_loader);
            let show_interface = true;
        });

        time_phase!("initialize timer", {
            let game_timer = GameTimer::new();
        });

        time_phase!("initialize camera", {
            #[cfg(feature = "debug")]
            let debug_camera = DebugCamera::new();
            let mut start_camera = StartCamera::new();
            let player_camera = PlayerCamera::new();
            let mut directional_shadow_camera = DirectionalShadowCamera::new();
            let point_shadow_camera = PointShadowCamera::new();
            start_camera.set_focus_point(START_CAMERA_FOCUS_POINT);
        });

        // TODO: Move all of these to the ClientState
        let saved_login_data: Option<LoginServerLoginData> = None;
        let saved_character_server: Option<CharacterServerInformation> = None;
        let saved_login_server_address = None;
        let saved_password = String::new();
        let saved_username = String::new();
        let saved_packet_version = FALLBACK_PACKET_VERSION;

        time_phase!("initialize networking", {
            #[cfg(not(feature = "debug"))]
            let (networking_system, network_event_buffer) = NetworkingSystem::spawn();

            #[cfg(feature = "debug")]
            let (packet_history, packet_history_callback) = PacketHistory::new();
            #[cfg(feature = "debug")]
            let (networking_system, network_event_buffer) = NetworkingSystem::spawn_with_callback(packet_history_callback);
        });

        time_phase!("create resources", {
            let input_event_buffer = Vec::new();
            let autopilot = autopilot::Autopilot::from_env();

            let particle_holder = ParticleHolder::default();
            let point_light_manager = PointLightManager::new();
            let effect_holder = EffectHolder::default();
            let cast_bars = effects::CastBars::default();
            let path_finder = PathFinder::default();

            let point_light_set_buffer = ResourceSetBuffer::default();
            let directional_shadow_object_set_buffer = ResourceSetBuffer::default();
            let point_shadow_object_set_buffer = ResourceSetBuffer::default();
            let deferred_object_set_buffer = ResourceSetBuffer::default();
            #[cfg(feature = "debug")]
            let bounding_box_object_set_buffer = ResourceSetBuffer::default();

            #[cfg(feature = "debug")]
            let pathing_texture_set = TextureSetBuilder::build_from_group(texture_loader.clone(), video_loader.clone(), "pathing", &[
                "pathing_goal.png",
                "pathing_straight.png",
                "pathing_diagonal.png",
            ]);
            #[cfg(feature = "debug")]
            let pathing_texture_set = Arc::new(pathing_texture_set);

            #[cfg(feature = "debug")]
            let tile_texture_set = TextureSetBuilder::build_from_group(texture_loader.clone(), video_loader.clone(), "tile", &[
                "tile_0.png",
                "tile_1.png",
                "tile_2.png",
                "tile_3.png",
                "tile_4.png",
                "tile_5.png",
                "tile_6.png",
            ]);
            #[cfg(feature = "debug")]
            let tile_texture_set = Arc::new(tile_texture_set);

            let main_menu_click_sound_effect = audio_engine.load(MAIN_MENU_CLICK_SOUND_EFFECT);
        });

        time_phase!("load default map", {
            let map = map_loader
                .load(
                    DEFAULT_MAP.to_string(),
                    &model_loader,
                    texture_loader.clone(),
                    video_loader,
                    &library,
                )
                .expect("failed to load initial map");

            directional_shadow_camera.set_level_bound(map.get_level_bound());

            audio_engine.play_background_music_track(DEFAULT_BACKGROUND_MUSIC);
            map.set_ambient_sound_sources(&audio_engine);
        });

        time_phase!("create client state", {
            let client_state = State::new(ClientState::new(
                &game_file_loader,
                graphics_settings.clone(),
                #[cfg(feature = "debug")]
                packet_history,
            ));
        });

        let active_interface_settings = client_state.follow(crate::client_state().interface_settings()).clone();
        let loaded_in_game_theme = session_theme_name(&active_interface_settings.in_game_theme).to_string();
        let loaded_world_theme = session_theme_name(&active_interface_settings.world_theme).to_string();

        interface.open_window(LoginWindow::new(
            crate::client_state().login_window(),
            crate::client_state().login_settings(),
            crate::client_state().client_info(),
        ));

        Some(Self {
            event_loop: Some(event_loop),
            startup_error: None,
            keyboard_modifiers: ModifiersState::empty(),
            game_file_loader,
            action_loader,
            #[cfg(feature = "debug")]
            animation_loader,
            async_loader,
            effect_loader,
            font_loader,
            #[cfg(feature = "debug")]
            map_loader,
            sprite_loader,
            texture_loader,
            library,
            interface_renderer,
            bottom_interface_renderer,
            middle_interface_renderer,
            top_interface_renderer,
            effect_renderer,
            #[cfg(feature = "debug")]
            debug_marker_renderer,
            #[cfg(feature = "debug")]
            aabb_instructions,
            #[cfg(feature = "debug")]
            circle_instructions,
            #[cfg(feature = "debug")]
            rectangle_instructions,
            model_batches,
            model_instructions,
            entity_instructions,
            directional_shadow_model_batches,
            directional_shadow_model_instructions,
            directional_shadow_entity_instructions,
            point_shadow_model_batches,
            point_shadow_model_instructions,
            point_shadow_entity_instructions,
            point_light_with_shadow_instructions,
            point_light_instructions,
            input_system,
            interface,
            mouse_cursor,
            show_interface,
            game_timer,
            #[cfg(feature = "debug")]
            debug_camera,
            start_camera,
            player_camera,
            directional_shadow_camera,
            directional_shadow_partitions,
            point_shadow_camera,
            input_event_buffer,
            autopilot,
            network_event_buffer,
            saved_login_data,
            saved_character_server,
            saved_login_server_address,
            saved_password,
            saved_username,
            saved_packet_version,
            particle_holder,
            point_light_manager,
            effect_holder,
            cast_bars,
            path_finder,
            point_light_set_buffer,
            directional_shadow_object_set_buffer,
            point_shadow_object_set_buffer,
            deferred_object_set_buffer,
            #[cfg(feature = "debug")]
            bounding_box_object_set_buffer,
            #[cfg(feature = "debug")]
            pathing_texture_set,
            #[cfg(feature = "debug")]
            tile_texture_set,
            main_menu_click_sound_effect,
            networking_system,
            audio_engine,
            active_interface_settings,
            loaded_in_game_theme_mtime: crate::state::theme::theme_file_mtime(&loaded_in_game_theme),
            loaded_world_theme_mtime: crate::state::theme::theme_file_mtime(&loaded_world_theme),
            loaded_in_game_theme,
            loaded_world_theme,
            force_reload_themes: false,
            active_graphics_settings: graphics_settings,
            graphics_engine,
            queue,
            #[cfg(feature = "debug")]
            device,
            window: None,

            map: Some(map),
            client_state,
        })
    }

    pub fn run(&mut self) -> Result<(), String> {
        let event_loop = self.event_loop.take().ok_or("Client event loop has already been consumed")?;
        event_loop.set_control_flow(ControlFlow::Poll);
        event_loop
            .run_app(self)
            .map_err(|error| format!("Native event loop failed: {error}"))?;
        self.startup_error.take().map_or(Ok(()), Err)
    }

    #[inline(always)]
    #[cfg_attr(feature = "debug", korangar_debug::profile)]
    fn clear_render_instructions(&mut self) {
        self.interface_renderer.clear();
        self.bottom_interface_renderer.clear();
        self.middle_interface_renderer.clear();
        self.top_interface_renderer.clear();
        self.effect_renderer.clear();
        #[cfg(feature = "debug")]
        self.debug_marker_renderer.clear();

        #[cfg(feature = "debug")]
        self.aabb_instructions.clear();
        #[cfg(feature = "debug")]
        self.circle_instructions.clear();
        #[cfg(feature = "debug")]
        self.rectangle_instructions.clear();
        self.model_batches.clear();
        self.model_instructions.clear();
        self.entity_instructions.clear();
        self.directional_shadow_model_batches.iter_mut().for_each(|batch| batch.clear());
        self.directional_shadow_model_instructions.clear();
        self.directional_shadow_entity_instructions
            .iter_mut()
            .for_each(|instructions| instructions.clear());
        self.point_shadow_model_batches.clear();
        self.point_shadow_model_instructions.clear();
        self.point_shadow_entity_instructions.clear();
        self.point_light_with_shadow_instructions.clear();
        self.point_light_instructions.clear();
    }

    #[inline(always)]
    #[cfg_attr(feature = "debug", korangar_debug::profile)]
    fn update_client_state(&mut self) {
        // Unset the highlighted skill just before applying. That way, if the skill is
        // still hovered the value will be the same as before, if not it will clear the
        // highlighted.
        *self.client_state.follow_mut(client_state().skill_tree_window().highlighted_skill()) = None;

        // Apply the game state after all the UI work + rendering is done.
        if let Err(_errors) = self.client_state.apply() {
            #[cfg(feature = "debug")]
            {
                print_debug!("[{}] failed to apply {} updates: ", "error".red(), _errors.len());
                _errors.into_iter().for_each(|error| print_debug!("path: {}", error.type_name));
            }
        }
    }

    /// Apply any graphics or interface setting changes that the user
    /// dispatched during the previous frame.
    ///
    /// May reconfigure the GPU surface (MSAA, SSAA, present mode, shadow
    /// resolution, etc.). Surface reconfiguration is only safe between
    /// presenting the previous frame and acquiring the next swapchain image,
    /// so this *must* be called after [`Self::update_client_state`] (so the
    /// user's dispatched changes are visible) and *before*
    /// `graphics_engine.wait_for_next_frame()`. Calling it after
    /// `wait_for_next_frame` has been observed to cause surface configuration
    /// errors under DX12.
    #[inline(always)]
    #[cfg_attr(feature = "debug", korangar_debug::profile)]
    fn update_settings(&mut self) {
        let graphics_settings = self.client_state.follow(client_state().graphics_settings());

        if self.active_graphics_settings.vsync != graphics_settings.vsync {
            self.graphics_engine.set_vsync(graphics_settings.vsync);
            self.active_graphics_settings.vsync = graphics_settings.vsync;
        }

        if self.active_graphics_settings.limit_framerate != graphics_settings.limit_framerate {
            self.graphics_engine.set_limit_framerate(graphics_settings.limit_framerate);
            self.active_graphics_settings.limit_framerate = graphics_settings.limit_framerate;
        }

        if self.active_graphics_settings.triple_buffering != graphics_settings.triple_buffering {
            self.graphics_engine.set_triple_buffering(graphics_settings.triple_buffering);
            self.active_graphics_settings.triple_buffering = graphics_settings.triple_buffering;
        }

        if self.active_graphics_settings.texture_filtering != graphics_settings.texture_filtering {
            self.graphics_engine.set_texture_sampler_type(graphics_settings.texture_filtering);
            self.active_graphics_settings.texture_filtering = graphics_settings.texture_filtering;
        }

        if self.active_graphics_settings.msaa != graphics_settings.msaa {
            self.graphics_engine.set_msaa(graphics_settings.msaa);
            self.active_graphics_settings.msaa = graphics_settings.msaa;
        }

        if self.active_graphics_settings.ssaa != graphics_settings.ssaa {
            self.graphics_engine.set_ssaa(graphics_settings.ssaa);
            self.active_graphics_settings.ssaa = graphics_settings.ssaa;
        }

        if self.active_graphics_settings.screen_space_anti_aliasing != graphics_settings.screen_space_anti_aliasing {
            self.graphics_engine
                .set_screen_space_anti_aliasing(graphics_settings.screen_space_anti_aliasing);
            self.active_graphics_settings.screen_space_anti_aliasing = graphics_settings.screen_space_anti_aliasing;
        }

        if self.active_graphics_settings.shadow_resolution != graphics_settings.shadow_resolution {
            self.graphics_engine.set_shadow_resolution(graphics_settings.shadow_resolution);
            self.active_graphics_settings.shadow_resolution = graphics_settings.shadow_resolution;
        }

        if self.active_graphics_settings.high_quality_interface != graphics_settings.high_quality_interface {
            self.interface_renderer
                .update_high_quality_interface(graphics_settings.high_quality_interface);
            self.graphics_engine
                .set_high_quality_interface(graphics_settings.high_quality_interface);
            self.active_graphics_settings.high_quality_interface = graphics_settings.high_quality_interface;
        }

        let language = *self.client_state.follow(client_state().interface_settings().language());

        if self.active_interface_settings.language != language {
            *self.client_state.follow_mut(client_state().localization()) = Localization::load_language(&self.game_file_loader, language);
            self.active_interface_settings.language = language;
        }

        let interface_settings = self.client_state.follow_mut(client_state().interface_settings());

        if self.active_interface_settings.menu_theme != interface_settings.menu_theme {
            let menu_theme = interface_settings.menu_theme.clone();
            let theme = InterfaceTheme::load(state::theme::InterfaceThemeType::Menu, &menu_theme);
            *self.client_state.follow_mut(client_state().menu_theme()) = theme;
            self.active_interface_settings.menu_theme = menu_theme;
        }

        let interface_settings = self.client_state.follow(client_state().interface_settings());

        let saved_in_game_theme = interface_settings.in_game_theme.clone();
        let in_game_theme = session_theme_name(&saved_in_game_theme).to_string();
        let in_game_mtime = crate::state::theme::theme_file_mtime(&in_game_theme);
        if self.force_reload_themes || self.loaded_in_game_theme != in_game_theme || self.loaded_in_game_theme_mtime != in_game_mtime {
            let theme = InterfaceTheme::load(InterfaceThemeType::InGame, &in_game_theme);
            *self.client_state.follow_mut(client_state().in_game_theme()) = theme;
            self.loaded_in_game_theme = in_game_theme;
            self.loaded_in_game_theme_mtime = in_game_mtime;
        }
        if self.active_interface_settings.in_game_theme != saved_in_game_theme {
            self.active_interface_settings.in_game_theme = saved_in_game_theme;
        }

        let interface_settings = self.client_state.follow(client_state().interface_settings());

        let saved_world_theme = interface_settings.world_theme.clone();
        let world_theme = session_theme_name(&saved_world_theme).to_string();
        let world_mtime = crate::state::theme::theme_file_mtime(&world_theme);
        if self.force_reload_themes || self.loaded_world_theme != world_theme || self.loaded_world_theme_mtime != world_mtime {
            let theme = WorldTheme::load(&world_theme);
            *self.client_state.follow_mut(client_state().world_theme()) = theme;
            self.loaded_world_theme = world_theme;
            self.loaded_world_theme_mtime = world_mtime;
        }
        if self.active_interface_settings.world_theme != saved_world_theme {
            self.active_interface_settings.world_theme = saved_world_theme;
        }
        if self.force_reload_themes {
            *self.client_state.follow_mut(client_state().interface_settings_capabilities()) =
                crate::settings::InterfaceSettingsCapabilities::default();
            self.force_reload_themes = false;
        }
    }

    #[inline(always)]
    #[cfg_attr(feature = "debug", korangar_debug::profile)]
    fn update_interface_scaling(&mut self, scaling: Scaling) {
        self.bottom_interface_renderer.update_scaling(scaling);
        self.middle_interface_renderer.update_scaling(scaling);
        self.top_interface_renderer.update_scaling(scaling);
    }

    #[inline(always)]
    #[cfg_attr(feature = "debug", korangar_debug::profile)]
    fn request_entity_details(&mut self, input_report: &InputReport) {
        if let PickerTarget::Entity(entity_id) = input_report.mouse_target
            && let Some(entity) = self
                .client_state
                .follow_mut(client_state().entities())
                .iter_mut()
                .find(|entity| entity.get_entity_id() == entity_id)
            && entity.are_details_unavailable()
            && self.networking_system.entity_details(entity_id).is_ok()
        {
            entity.set_details_requested();
        }
    }

    #[inline(always)]
    #[cfg_attr(feature = "debug", korangar_debug::profile)]
    fn spawn_str_effects(&mut self, paths: &[&str], entity_id: EntityId, position: Option<Point3<f32>>, repeating: bool) {
        for path in paths {
            let Ok(effect) = self.effect_loader.get_or_load(path, &self.texture_loader) else {
                #[cfg(feature = "debug")]
                print_debug!("[{}] failed to load effect {}", "effect".red(), path.magenta());
                continue;
            };
            let (color, range) = str_effect_light(path);
            let frame_timer = effect.new_frame_timer();
            let (center, offset, light_offset) = if let Some(position) = position {
                (
                    EffectCenter::Position(position),
                    Vector3::new(0.0, 0.0, 0.0),
                    Vector3::new(0.0, 6.0, 0.0),
                )
            } else {
                (
                    EffectCenter::Entity(entity_id, Point3::new(0.0, 0.0, 0.0)),
                    Vector3::new(0.0, 9.0, 0.0),
                    Vector3::new(0.0, 12.0, 0.0),
                )
            };
            let built = Box::new(EffectWithLight::new(
                effect,
                frame_timer,
                center,
                offset,
                PointLightId::new(entity_id.0),
                light_offset,
                color,
                range,
                repeating,
            ));
            // Repeating ground units are removed by `RemoveSkillUnit`. One-shot
            // effects, even when placed on a cell, must not be keyed by the caster.
            if position.is_some() && repeating {
                self.effect_holder.add_unit(built, entity_id);
            } else {
                self.effect_holder.add_effect(built);
            }
        }
    }

    fn play_skill_field(&mut self, skill_id: SkillId, field: SkillField, entity_id: EntityId, position: Option<Point3<f32>>, repeating: bool) {
        let paths = effects::collect_str_paths(effects::skill_effect_refs(skill_id.0, field));
        self.spawn_str_effects(&paths, entity_id, position, repeating);
    }

    fn handle_network_events(&mut self, client_tick: ClientTick) {
        *self.client_state.follow_mut(client_state().client_tick()) = client_tick;
        self.cast_bars.expire(client_tick.0);
        self.client_state.follow_mut(client_state().navigation()).expire(client_tick.0);
        self.client_state.follow_mut(client_state()).expire_chat_overlays(client_tick.0);
        self.client_state
            .follow_mut(client_state().roulette_window())
            .tick_spin(client_tick.0);
        if self.headless_auto_login() && !HEADLESS_AUTO_LOGIN_DONE.swap(true, Ordering::SeqCst) {
            let login_settings = self.client_state.follow(client_state().login_settings());
            if let Some(service_id) = login_settings.recent_service_id
                && let Some(service) = login_settings.service_settings.get(&service_id)
                && !service.username.is_empty()
                && !service.password.is_empty()
            {
                self.input_event_buffer.push(InputEvent::LogIn {
                    service_id,
                    username: service.username.clone(),
                    password: service.password.clone(),
                });
            }
        }
        self.networking_system.get_events(&mut self.network_event_buffer);

        let mut defer_headless_character = false;
        let mut autopilot_slot_occupied: Option<bool> = None;
        let mut pincode_effects = Vec::new();
        let mut autopilot_effects = Vec::new();
        // Own the events before matching. Shop handlers and effect playback borrow
        // `self`, which cannot overlap the buffer drain.
        let network_events: Vec<_> = self.network_event_buffer.drain().collect();
        for event in network_events {
            match event {
                NetworkEvent::LoginServerConnected {
                    character_servers,
                    login_data,
                } => {
                    self.audio_engine.play_sound_effect(self.main_menu_click_sound_effect);

                    // Remove `_m`/`_f` suffix from the username. The suffix is only for *creating*
                    // an account and thus can (and needs to) be removed after the first successful
                    // login.
                    {
                        let selected_service_path =
                            SelectedServicePath::new(client_state().login_window(), client_state().login_settings());
                        let username_path = selected_service_path.username();

                        let username = self.client_state.follow_mut(username_path);

                        if let Some(stripped) = username.strip_suffix("_m") {
                            *username = stripped.to_owned();
                        } else if let Some(stripped) = username.strip_suffix("_f") {
                            *username = stripped.to_owned();
                        }
                    }

                    self.saved_login_data = Some(login_data);

                    // `SelectServer` must not alias the vec moved into client state.
                    let server_names: Vec<String> = character_servers.iter().map(|server| server.server_name.clone()).collect();
                    let auto_server = character_servers.first().cloned();
                    *self.client_state.follow_mut(client_state().character_servers()) = character_servers;

                    #[cfg(not(feature = "debug"))]
                    self.interface.close_all_windows();

                    #[cfg(feature = "debug")]
                    self.interface.close_all_windows_except(DEBUG_WINDOWS);

                    self.interface
                        .open_window(ServerSelectionWindow::new(client_state().character_servers()));
                    if headless_auto()
                        && self.autopilot.is_none()
                        && let Some(character_server_information) = auto_server
                    {
                        self.input_event_buffer.push(InputEvent::SelectServer {
                            character_server_information,
                        });
                    }
                    if let Some(auto) = self.autopilot.as_mut() {
                        autopilot_effects.extend(auto.on_login_ok(Instant::now(), &server_names));
                    }
                }
                NetworkEvent::LoginServerConnectionFailed { message, .. } => {
                    if let Some(auto) = self.autopilot.as_mut() {
                        autopilot_effects.extend(auto.on_login_failed(&message));
                    }
                    self.networking_system.disconnect_from_login_server();

                    self.interface.open_window(ErrorWindow::new(message.to_owned()));
                }
                NetworkEvent::LoginServerDisconnected { reason } => {
                    if reason != DisconnectReason::ClosedByClient {
                        // TODO: Make this an on-screen popup.
                        #[cfg(feature = "debug")]
                        print_debug!("Disconnection from the character server with error");

                        let socket_address = self.saved_login_server_address.unwrap();
                        self.networking_system.connect_to_login_server(
                            self.saved_packet_version,
                            socket_address,
                            &self.saved_username,
                            &self.saved_password,
                        );
                    }
                }
                NetworkEvent::CharacterServerConnected { normal_slot_count } => {
                    self.client_state
                        .follow_mut(client_state().character_slots())
                        .set_slot_count(normal_slot_count);

                    let _ = self.networking_system.request_character_list();
                }
                NetworkEvent::CharacterServerConnectionFailed { message, .. } => {
                    self.networking_system.disconnect_from_character_server();
                    self.interface.open_window(ErrorWindow::new(message.to_owned()));
                }
                NetworkEvent::CharacterServerDisconnected { reason } => {
                    if reason != DisconnectReason::ClosedByClient {
                        // TODO: Make this an on-screen popup.
                        #[cfg(feature = "debug")]
                        print_debug!("Disconnection from the character server with error");

                        let login_data = self.saved_login_data.as_ref().unwrap();
                        let server = self.saved_character_server.clone().unwrap();
                        self.networking_system
                            .connect_to_character_server(self.saved_packet_version, login_data, server);
                    } else if !self.networking_system.is_map_server_connected() {
                        #[cfg(not(feature = "debug"))]
                        self.interface.close_all_windows();

                        #[cfg(feature = "debug")]
                        self.interface.close_all_windows_except(DEBUG_WINDOWS);

                        self.interface.open_window(LoginWindow::new(
                            client_state().login_window(),
                            client_state().login_settings(),
                            client_state().client_info(),
                        ));
                    }
                }
                NetworkEvent::MapServerDisconnected { reason } => {
                    if reason != DisconnectReason::ClosedByClient {
                        // TODO: Make this an on-screen popup.
                        #[cfg(feature = "debug")]
                        print_debug!("Disconnection from the map server with error");
                    }

                    let login_data = self.saved_login_data.as_ref().unwrap();
                    let server = self.saved_character_server.clone().unwrap();
                    self.networking_system
                        .connect_to_character_server(self.saved_packet_version, login_data, server);

                    self.map = None;

                    self.particle_holder.clear();
                    self.effect_holder.clear();
                    self.point_light_manager.clear();
                    self.audio_engine.clear_ambient_sound();

                    self.client_state.follow_mut(client_state().entities()).clear();
                    self.client_state.follow_mut(client_state().dead_entities()).clear();
                    self.client_state.follow_mut(client_state().ground_items()).clear();
                    self.client_state.follow_mut(client_state().equip_switch()).clear();
                    *self.client_state.follow_mut(client_state().buffered_action()) = None;
                    self.client_state.follow_mut(client_state().status_effects()).clear();
                    self.client_state.follow_mut(client_state().skill_cooldowns()).clear();
                    self.client_state.follow_mut(client_state().quest_log()).clear();
                    self.client_state.follow_mut(client_state().achievements()).clear();
                    self.client_state.follow_mut(client_state().navigation()).clear();
                    self.client_state.follow_mut(client_state().guild()).clear();
                    self.client_state.follow_mut(client_state().guild_window()).clear();
                    self.client_state.follow_mut(client_state().party()).clear();
                    self.client_state.follow_mut(client_state().clan()).clear();
                    self.client_state.follow_mut(client_state().warpra_window()).clear_catalog();
                    self.client_state.follow_mut(client_state().world_map_window()).on_catalog_cleared();
                    self.client_state.follow_mut(client_state().companion_window()).clear();
                    self.client_state.follow_mut(client_state().mvp_minimap()).clear_markers();
                    self.client_state.follow_mut(client_state().mvp_minimap()).set_map_size(0, 0);
                    self.interface.close_window_with_class(WindowClass::MvpMinimap);
                    if self.interface.is_window_with_class_open(WindowClass::Mail) {
                        let _ = self.networking_system.close_rodex();
                    }
                    self.client_state.follow_mut(client_state().mail_window()).clear();
                    self.interface.close_window_with_class(WindowClass::Mail);

                    self.audio_engine.play_background_music_track(None);

                    #[cfg(not(feature = "debug"))]
                    self.interface.close_all_windows();

                    #[cfg(feature = "debug")]
                    self.interface.close_all_windows_except(DEBUG_WINDOWS);

                    self.async_loader
                        .request_map_load(DEFAULT_MAP.to_string(), Some(TilePosition::new(0, 0)));
                }
                NetworkEvent::InitialStats {
                    strength_stat_points_cost,
                    agility_stat_points_cost,
                    vitality_stat_points_cost,
                    intelligence_stat_points_cost,
                    dexterity_stat_points_cost,
                    luck_stat_points_cost,
                } => {
                    if let Some(player) = self.client_state.try_follow_mut(this_player()) {
                        player.strength_stat_points_cost = strength_stat_points_cost;
                        player.agility_stat_points_cost = agility_stat_points_cost;
                        player.vitality_stat_points_cost = vitality_stat_points_cost;
                        player.intelligence_stat_points_cost = intelligence_stat_points_cost;
                        player.dexterity_stat_points_cost = dexterity_stat_points_cost;
                        player.luck_stat_points_cost = luck_stat_points_cost;
                    }
                }
                NetworkEvent::ResurrectPlayer { entity_id } => {
                    // If the resurrected player is us, close the resurrect window.
                    if self
                        .client_state
                        .try_follow(this_entity())
                        .is_some_and(|player| player.get_entity_id() == entity_id)
                    {
                        self.interface.close_window_with_class(WindowClass::Respawn);
                    }
                }
                NetworkEvent::PlayerStandUp { entity_id } => {
                    if let Some(entity) = self
                        .client_state
                        .follow_mut(client_state().entities())
                        .iter_mut()
                        .find(|entity| entity.get_entity_id() == entity_id)
                    {
                        entity.set_idle(client_tick);
                    }
                }
                NetworkEvent::DisplayEmotion { entity_id, emotion } => {
                    if let Some(entity) = self
                        .client_state
                        .follow(client_state().entities())
                        .iter()
                        .find(|entity| entity.get_entity_id() == entity_id)
                        && let Ok(sprite) = self.sprite_loader.get_or_load("이팩트\\emotion.spr")
                        && let Ok(actions) = self.action_loader.get_or_load("이팩트\\emotion.act")
                    {
                        self.particle_holder
                            .spawn_particle(Box::new(Emote::new(entity.get_position(), sprite, actions, emotion as usize)));
                    }
                }
                NetworkEvent::AccountId { .. } => {}
                NetworkEvent::CharacterList { characters } => {
                    self.audio_engine.play_sound_effect(self.main_menu_click_sound_effect);

                    self.client_state
                        .follow_mut(client_state().character_slots())
                        .set_characters(characters);

                    if !self.interface.is_window_with_class_open(WindowClass::CharacterSelection) {
                        // TODO: this will do one unnecessary restore_focus. check
                        // if that will be problematic

                        #[cfg(not(feature = "debug"))]
                        self.interface.close_all_windows();

                        #[cfg(feature = "debug")]
                        self.interface.close_all_windows_except(DEBUG_WINDOWS);

                        self.client_state.follow_mut(client_state().status_effects()).clear();
                        self.client_state.follow_mut(client_state().skill_cooldowns()).clear();
                        self.client_state.follow_mut(client_state().quest_log()).clear();
                        self.client_state.follow_mut(client_state().achievements()).clear();
                        self.client_state.follow_mut(client_state().navigation()).clear();
                        self.client_state.follow_mut(client_state().guild()).clear();
                        self.client_state.follow_mut(client_state().guild_window()).clear();
                        self.client_state.follow_mut(client_state().warpra_window()).clear_catalog();
                        self.client_state.follow_mut(client_state().world_map_window()).on_catalog_cleared();
                        self.client_state.follow_mut(client_state().companion_window()).clear();
                        self.interface.open_window(CharacterSelectionWindow::new(
                            client_state().character_slots(),
                            client_state().switch_request(),
                        ));
                    }
                    // The menu map opens this window before the list arrives. Selecting only
                    // when the window was closed skips the character on a slow first frame.
                    if self.autopilot.is_some() {
                        let slot = self.autopilot.as_ref().map(|auto| auto.slot()).unwrap_or(0);
                        autopilot_slot_occupied = Some(
                            self.client_state
                                .try_follow(client_state().character_slots().in_slot(slot))
                                .is_some(),
                        );
                    } else if headless_auto() && self.interface.is_window_with_class_open(WindowClass::CharacterSelection) {
                        defer_headless_character = true;
                    }
                }
                NetworkEvent::Pincode { notice } => {
                    let autopilot = crate::state::pincode::PincodeWindowState::autopilot_pin();
                    let effect = self
                        .client_state
                        .follow_mut(client_state().pincode_window())
                        .apply_notice(notice, autopilot.as_deref());
                    pincode_effects.push(effect);
                    if !self.client_state.follow(client_state().pincode_window()).allows_select() {
                        defer_headless_character = false;
                    } else if self.interface.is_window_with_class_open(WindowClass::CharacterSelection) {
                        if self.autopilot.is_some() {
                            let slot = self.autopilot.as_ref().map(|auto| auto.slot()).unwrap_or(0);
                            autopilot_slot_occupied = Some(
                                self.client_state
                                    .try_follow(client_state().character_slots().in_slot(slot))
                                    .is_some(),
                            );
                        } else if headless_auto() {
                            defer_headless_character = true;
                        }
                    }
                }
                NetworkEvent::CharacterSelectionFailed { message, .. } => self.interface.open_window(ErrorWindow::new(message.to_owned())),
                NetworkEvent::CharacterDeleted => {
                    if let Some(character_id) = self.client_state.follow_mut(client_state().currently_deleting()).take() {
                        self.client_state
                            .follow_mut(client_state().character_slots())
                            .remove_with_id(character_id);
                    }
                }
                NetworkEvent::CharacterDeletionFailed { message, .. } => {
                    *self.client_state.follow_mut(client_state().currently_deleting()) = None;
                    self.interface.open_window(ErrorWindow::new(message.to_owned()))
                }
                NetworkEvent::CharacterSelected { login_data, .. } => {
                    self.audio_engine.play_sound_effect(self.main_menu_click_sound_effect);

                    let saved_login_data = self.saved_login_data.as_ref().unwrap();
                    self.networking_system.disconnect_from_character_server();
                    self.networking_system
                        .connect_to_map_server(self.saved_packet_version, saved_login_data, login_data);
                    // NOTE: Nothing else must be sent to the map server until it responds:
                    // rAthena requires the first read of a session to contain exactly the
                    // login packet and disconnects otherwise. The client tick is part of
                    // the map server login success packet, so there is no need to request
                    // it here.

                    let character_information = self
                        .client_state
                        .follow(client_state().character_slots())
                        .with_id(login_data.character_id)
                        .cloned()
                        .unwrap();

                    let mut player = Entity::Player(Player::new(
                        &self.library,
                        saved_login_data.account_id,
                        &character_information,
                        client_tick,
                    ));

                    *self.client_state.follow_mut(client_state().player_name()) = character_information.name;
                    *self.client_state.follow_mut(client_state().player_character_id()) = login_data.character_id;

                    let entity_id = player.get_entity_id();
                    let entity_type = player.get_entity_type();
                    let entity_part_files = player.get_entity_part_files(&self.library);

                    if let Some(animation_data) = self
                        .async_loader
                        .request_animation_data_load(entity_id, entity_type, entity_part_files)
                    {
                        player.set_animation_data(animation_data);
                    }

                    let layout = self.async_loader.request_skill_tree_layout_load(player.get_job_id(), client_tick);
                    *self.client_state.follow_mut(client_state().skill_tree_window().selected_tab()) = layout.tabs.len().saturating_sub(1);
                    *self.client_state.follow_mut(client_state().skill_tree().layout()) = layout;
                    self.client_state
                        .follow_mut(client_state().skill_tree_window().chosen_skill_level())
                        .clear();

                    self.client_state.follow_mut(client_state().entities()).push(player);

                    self.interface.close_window_with_class(WindowClass::CharacterSelection);
                    self.interface.close_window_with_class(WindowClass::CharacterCreation);
                    self.interface.close_window_with_class(WindowClass::Pincode);
                    if headless_auto() || self.autopilot.is_some() {
                        self.interface.close_window_with_class(WindowClass::CharacterOverview);
                        self.interface.close_window_with_class(WindowClass::Menu);
                    } else {
                        self.interface.open_window(CharacterOverviewWindow::new(
                            client_state().player_name(),
                            // TODO: Check that manually asserting is fine. Technically this window should only
                            // be open while the player is selected.
                            this_player().manually_asserted().base_level(),
                            // TODO: Check that manually asserting is fine. Technically this window should only
                            // be open while the player is selected.
                            this_player().manually_asserted().job_level(),
                        ));
                    }
                    self.interface
                        .open_window(ChatWindow::new(client_state().chat_window(), client_state().chat_messages()));
                    self.interface.open_window(StatusPanelWindow::new(
                        client_state().player_name(),
                        this_player().manually_asserted(),
                        client_state().status_effects(),
                    ));
                    self.interface.open_window(HotbarWindow::new(
                        client_state().hotbar().skills(),
                        client_state().skill_tree().skills(),
                    ));

                    // Put the dialog system in a well-defined state.
                    crate::interface::npc_flow::dismiss_npc_dialog_ui(&mut self.client_state, &mut self.interface);

                    self.map = None;

                    self.particle_holder.clear();
                    self.effect_holder.clear();
                    self.point_light_manager.clear();
                    self.audio_engine.clear_ambient_sound();
                }
                NetworkEvent::CharacterCreated { character_information } => {
                    self.client_state
                        .follow_mut(client_state().character_slots())
                        .add_character(character_information);

                    self.interface.close_window_with_class(WindowClass::CharacterCreation);
                    if let Some(auto) = self.autopilot.as_mut() {
                        autopilot_effects.extend(auto.on_character_created(Instant::now()));
                    }
                }
                NetworkEvent::CharacterCreationFailed { message, .. } => {
                    if let Some(auto) = self.autopilot.as_mut() {
                        autopilot_effects.extend(auto.on_character_creation_failed(message));
                    }
                    self.interface.open_window(ErrorWindow::new(message.to_owned()));
                }
                NetworkEvent::CharacterSlotSwitched => {
                    *self.client_state.follow_mut(client_state().switch_request()) = None;
                }
                NetworkEvent::CharacterSlotSwitchFailed => {
                    self.interface
                        .open_window(ErrorWindow::new("Failed to switch character slots".to_owned()));
                }
                NetworkEvent::AddEntity { entity_data } => {
                    let appeared_guild_id = entity_data.guild_id;
                    let appeared_emblem_version = entity_data.emblem_version;
                    let mut spawned = false;
                    if let Some(map) = &self.map
                        && let Some(npc) = Npc::new(&self.library, map, &mut self.path_finder, entity_data, client_tick)
                    {
                        spawned = true;
                        let mut npc = Entity::Npc(npc);

                        let entity_id = npc.get_entity_id();
                        let entity_type = npc.get_entity_type();
                        let entity_part_files = npc.get_entity_part_files(&self.library);

                        let entities = self.client_state.follow_mut(client_state().entities());

                        // If the entity was already visible, we use it's old alpha value.
                        if let Some(entity) = entities.iter().find(|entity| entity.get_entity_id() == entity_id) {
                            npc.inherit_fade_state(entity, client_tick);
                        };

                        // Sometimes (like after a job change) the server will tell the client
                        // that a new entity appeared, even though it was already on screen. So
                        // to prevent the entity existing twice, we remove the old one.
                        entities.retain(|entity| entity.get_entity_id() != entity_id);

                        if let Some(animation_data) =
                            self.async_loader
                                .request_animation_data_load(entity_id, entity_type, entity_part_files)
                        {
                            npc.set_animation_data(animation_data);
                        }

                        #[cfg(feature = "debug")]
                        npc.generate_pathing_mesh(&self.device, &self.queue, self.graphics_engine.bindless_support(), map);

                        entities.push(npc);
                    }
                    if spawned {
                        request_emblem_if_needed(
                            &mut self.client_state,
                            &mut self.networking_system,
                            appeared_guild_id,
                            appeared_emblem_version,
                        );
                    }
                }
                NetworkEvent::RemoveEntity { entity_id, reason } => {
                    // If the motive is dead, you need to set the player to dead.
                    if reason == DisappearanceReason::Died {
                        if let Some(entity) = self
                            .client_state
                            .follow_mut(client_state().entities())
                            .iter_mut()
                            .find(|entity| entity.get_entity_id() == entity_id)
                        {
                            let entity_type = entity.get_entity_type();

                            if entity_type == EntityType::Monster {
                                let mut entity = entity.clone();
                                entity.set_dead(client_tick);
                                entity.stop_movement();

                                // Remove the entity from the list of alive entities.
                                self.client_state
                                    .follow_mut(client_state().entities())
                                    .retain(|entity| entity.get_entity_id() != entity_id);

                                // Add the entity to the list of dead entities.
                                self.client_state.follow_mut(client_state().dead_entities()).push(entity);
                            } else if entity_type == EntityType::Player {
                                entity.set_dead(client_tick);

                                // If the player is us, we need to open the respawn window.
                                if entity_id == self.client_state.follow(client_state().entities())[0].get_entity_id() {
                                    self.interface.open_window(RespawnWindow);
                                }
                            }
                        }
                    } else {
                        // For non-death disappearances, start fading out the entity.
                        if let Some(entity) = self
                            .client_state
                            .follow_mut(client_state().entities())
                            .iter_mut()
                            .find(|entity| entity.get_entity_id() == entity_id)
                        {
                            entity.fade_out(reason, client_tick);
                        }
                    }

                    // If the entity that was removed had an attack buffered we remove the entity
                    // from the buffer.
                    let buffered_action = self.client_state.follow_mut(client_state().buffered_action());
                    if buffered_action.is_some_and(|buffered_action| buffered_action.is_attack_entity(entity_id)) {
                        *buffered_action = None;
                    }
                }
                NetworkEvent::AddGroundItem {
                    entity_id,
                    item_id,
                    is_identified,
                    quantity,
                    position,
                    x_offset,
                    y_offset,
                } => {
                    if let Some(map) = self.map.as_ref()
                        && let Some(mut ground_item) = GroundItem::new(
                            map,
                            item_id,
                            entity_id,
                            is_identified,
                            quantity,
                            position,
                            x_offset,
                            y_offset,
                            client_tick,
                        )
                    {
                        let ground_items = self.client_state.follow_mut(client_state().ground_items());
                        let entity_part_files = ground_item.get_entity_part_files(&self.library);

                        if let Some(animation_data) = self
                            .async_loader
                            // TODO: Technically Npc is not correct here. We could add an item
                            // variant or refactor this fuction to take an optional entity
                            // type.
                            .request_animation_data_load(entity_id, EntityType::Npc, entity_part_files)
                        {
                            ground_item.set_animation_data(animation_data);
                        }

                        ground_items.push(ground_item);
                    } else {
                        #[cfg(feature = "debug")]
                        print_debug!("[{}] failed to spawn item", "error".red());
                    }
                }
                NetworkEvent::RemoveGroundItem { entity_id } => {
                    if let Some(item) = self
                        .client_state
                        .follow_mut(client_state().ground_items())
                        .iter_mut()
                        .find(|item| item.get_entity_id() == entity_id)
                    {
                        item.fade_out(client_tick);
                    }

                    let buffered_action = self.client_state.follow_mut(client_state().buffered_action());
                    if buffered_action.is_some_and(|buffered_action| buffered_action.is_pick_up_item(entity_id)) {
                        *buffered_action = None;
                    }
                }
                NetworkEvent::EntityMove {
                    entity_id,
                    origin,
                    destination,
                    starting_timestamp,
                } => {
                    let entities = self.client_state.follow_mut(client_state().entities());
                    let entity = entities.iter_mut().find(|entity| entity.get_entity_id() == entity_id);

                    if let Some(entity) = entity
                        && let Some(map) = &self.map
                    {
                        entity.move_from_to(
                            map,
                            &mut self.path_finder,
                            origin.tile_position(),
                            destination.tile_position(),
                            starting_timestamp,
                        );
                        #[cfg(feature = "debug")]
                        entity.generate_pathing_mesh(&self.device, &self.queue, self.graphics_engine.bindless_support(), map);
                    }
                }
                NetworkEvent::PlayerMove {
                    origin,
                    destination,
                    starting_timestamp,
                } => {
                    if let Some(map) = &self.map
                        && let Some(player) = self.client_state.try_follow_mut(this_entity())
                    {
                        player.move_from_to(
                            map,
                            &mut self.path_finder,
                            origin.tile_position(),
                            destination.tile_position(),
                            starting_timestamp,
                        );
                        #[cfg(feature = "debug")]
                        player.generate_pathing_mesh(&self.device, &self.queue, self.graphics_engine.bindless_support(), map);
                    }
                    // The pin tracks the cell the walk starts from. Later frames
                    // sample the entity tile, and a rejected step arrives as
                    // `EntityStopMove` rather than the requested destination.
                    let origin = origin.tile_position();
                    self.client_state
                        .follow_mut(client_state().navigation())
                        .set_player_cell(origin.x, origin.y);
                }
                NetworkEvent::EntityStopMove { entity_id, position } => {
                    let is_local_player = self
                        .client_state
                        .try_follow(this_entity())
                        .is_some_and(|player| player.get_entity_id() == entity_id);
                    if is_local_player {
                        if let Some(map) = &self.map
                            && let Some(player) = self.client_state.try_follow_mut(this_entity())
                        {
                            player.set_position(map, position, client_tick);
                        }
                        self.client_state
                            .follow_mut(client_state().navigation())
                            .set_player_cell(position.x, position.y);
                    }
                }
                NetworkEvent::ChangeMap { map_name, position } => {
                    self.map = None;
                    self.particle_holder.clear();
                    self.effect_holder.clear();
                    self.point_light_manager.clear();
                    self.audio_engine.clear_ambient_sound();

                    // Only the player must stay alive between map changes.
                    self.client_state.follow_mut(client_state().entities()).truncate(1);
                    self.client_state.follow_mut(client_state().dead_entities()).clear();
                    self.client_state.follow_mut(client_state().ground_items()).clear();
                    self.client_state.follow_mut(client_state().player_shops()).clear();
                    self.client_state.follow_mut(client_state().mvp_minimap()).clear_markers();
                    self.client_state.follow_mut(client_state().mvp_minimap()).set_map_size(0, 0);
                    *self.client_state.follow_mut(client_state().buffered_action()) = None;

                    // Close any remaining dialogs. The new map restores its own BGM.
                    crate::interface::npc_flow::dismiss_npc_dialog_ui(&mut self.client_state, &mut self.interface);
                    self.client_state.follow_mut(client_state().npc_presentation()).clear_map();
                    if self.interface.is_window_with_class_open(WindowClass::Storage) {
                        let _ = self.networking_system.close_storage();
                        self.interface.close_window_with_class(WindowClass::Storage);
                    }
                    if self.interface.is_window_with_class_open(WindowClass::Enchant) {
                        let _ = self.networking_system.close_ui_enchant();
                        self.interface.close_window_with_class(WindowClass::Enchant);
                    }
                    self.interface.close_window_with_class(WindowClass::Stable);
                    self.interface.close_window_with_class(WindowClass::Eden);
                    self.interface.close_window_with_class(WindowClass::Warpra);
                    self.interface.close_window_with_class(WindowClass::WorldMap);
                    self.client_state.follow_mut(client_state().warpra_window()).clear_catalog();
                    self.client_state.follow_mut(client_state().world_map_window()).on_catalog_cleared();
                    self.interface.close_window_with_class(WindowClass::Companions);
                    self.client_state.follow_mut(client_state().companion_window()).clear();
                    if self.interface.is_window_with_class_open(WindowClass::Refine) {
                        let _ = self.networking_system.close_refine();
                        self.client_state.follow_mut(client_state().refine()).close();
                        self.interface.close_window_with_class(WindowClass::Refine);
                    }
                    crate::interface::windows::item_skill_selection::close_for_map_change(
                        &mut self.client_state,
                        &mut self.networking_system,
                        &mut self.interface,
                    );
                    if self.interface.is_window_with_class_open(WindowClass::ItemReform) {
                        let _ = self.networking_system.close_item_reform();
                        self.interface.close_window_with_class(WindowClass::ItemReform);
                    }
                    if self.interface.is_window_with_class_open(WindowClass::EnchantGrade) {
                        let _ = self.networking_system.close_grade_enchant();
                        self.interface.close_window_with_class(WindowClass::EnchantGrade);
                    }
                    if self.interface.is_window_with_class_open(WindowClass::LaphineSynthesis) {
                        let _ = self.networking_system.close_laphine_synthesis();
                        self.interface.close_window_with_class(WindowClass::LaphineSynthesis);
                    }
                    if self.interface.is_window_with_class_open(WindowClass::LaphineUpgrade) {
                        let _ = self.networking_system.close_laphine_upgrade();
                        self.interface.close_window_with_class(WindowClass::LaphineUpgrade);
                    }
                    if self.interface.is_window_with_class_open(WindowClass::Mail) {
                        let _ = self.networking_system.close_rodex();
                        self.interface.close_window_with_class(WindowClass::Mail);
                    }
                    if self.client_state.follow(client_state().barter()).is_open()
                        || self.interface.is_window_with_class_open(WindowClass::Barter)
                    {
                        let expanded = self.client_state.follow(client_state().barter()).is_expanded();
                        let _ = self.networking_system.close_barter(expanded);
                        self.client_state.follow_mut(client_state().barter()).mark_button_close();
                        self.interface.close_window_with_class(WindowClass::Barter);
                    }

                    self.client_state
                        .follow_mut(client_state().navigation())
                        .on_map_change(&map_name, position.x, position.y);
                    {
                        let display = self.library.map_display_name(&map_name);
                        let board = self.client_state.follow_mut(client_state().mvp_minimap());
                        board.set_display_name(display);
                        board.set_player_cell(position.x, position.y);
                    }
                    self.async_loader.request_map_load(map_name, Some(position));
                }
                NetworkEvent::UpdateClientTick { client_tick, received_at } => {
                    self.game_timer.set_client_tick(client_tick, received_at);
                }
                NetworkEvent::ChatMessage { text, color } => {
                    self.client_state
                        .follow_mut(client_state().chat_messages())
                        .push(ChatMessage::new(text, color));
                }
                NetworkEvent::EntitySpeech { entity_id, text, color } => {
                    self.client_state
                        .follow_mut(client_state())
                        .record_entity_speech(entity_id, text, color, client_tick.0);
                }
                NetworkEvent::BroadcastMessage { text, color } => {
                    self.client_state
                        .follow_mut(client_state())
                        .record_broadcast(text, color, client_tick.0);
                }
                NetworkEvent::WhisperReceived { sender, text, is_admin: _ } => {
                    self.client_state.follow_mut(client_state()).record_whisper(sender, text);
                }
                NetworkEvent::WhisperAck { result } => {
                    self.client_state.follow_mut(client_state()).record_whisper_ack(result);
                }
                NetworkEvent::UpdateEntityDetails { entity_id, name } => {
                    let entity = self
                        .client_state
                        .follow_mut(client_state().entities())
                        .iter_mut()
                        .find(|entity| entity.get_entity_id() == entity_id);

                    if let Some(entity) = entity {
                        entity.set_details(name);
                    }
                }
                NetworkEvent::DamageEffect {
                    source_entity_id,
                    destination_entity_id,
                    damage_amount,
                    attack_duration,
                    is_critical,
                } => {
                    let target_position = self
                        .client_state
                        .follow(client_state().entities())
                        .iter()
                        .find(|entity| entity.get_entity_id() == destination_entity_id)
                        .map(|entity| entity.get_tile_position());

                    // Auto attack logic.
                    if self
                        .client_state
                        .try_follow(this_entity())
                        .is_some_and(|player| player.get_entity_id() == source_entity_id)
                    {
                        let auto_attack = *self.client_state.follow(client_state().game_settings().auto_attack());
                        let buffered_action = self.client_state.follow_mut(client_state().buffered_action());

                        if let Some(BufferedAction::AttackEntity { entity_id }) = *buffered_action {
                            let _ = self.networking_system.player_attack(entity_id);

                            if !auto_attack {
                                *buffered_action = None;
                            }
                        }
                    }

                    if let Some(entity) = self
                        .client_state
                        .follow_mut(client_state().entities())
                        .iter_mut()
                        .find(|entity| entity.get_entity_id() == source_entity_id)
                    // TODO: Maybe also or_else this_entity?
                    {
                        if let Some(target_position) = target_position {
                            entity.rotate_towards(target_position);
                        }

                        entity.set_attack(attack_duration, is_critical, client_tick);
                    }

                    if let Some(entity) = self
                        .client_state
                        .follow(client_state().entities())
                        .iter()
                        .find(|entity| entity.get_entity_id() == destination_entity_id)
                        .or_else(|| self.client_state.try_follow(this_entity()))
                    {
                        let particle: Box<dyn Particle + Send + Sync> = match damage_amount {
                            Some(amount) => Box::new(DamageNumber::new(entity.get_position(), amount.to_string(), is_critical)),
                            None => Box::new(Miss::new(entity.get_position())),
                        };

                        self.particle_holder.spawn_particle(particle);
                    }
                }
                NetworkEvent::EntityPickUpItem { entity_id, item_entity_id } => {
                    let item_position = self
                        .client_state
                        .follow(client_state().ground_items())
                        .iter()
                        .find(|item| item.get_entity_id() == item_entity_id)
                        .map(|item| item.get_tile_position());

                    if let Some(entity) = self
                        .client_state
                        .follow_mut(client_state().entities())
                        .iter_mut()
                        .find(|entity| entity.get_entity_id() == entity_id)
                    {
                        if let Some(item_position) = item_position {
                            entity.rotate_towards(item_position);
                        }

                        if matches!(entity.get_entity_type(), EntityType::Player | EntityType::Hidden) {
                            entity.set_pickup(client_tick);
                        }
                    }
                }
                NetworkEvent::HealEffect {
                    entity_id,
                    source_entity_id,
                    skill_id,
                    heal_amount,
                    result,
                } => {
                    self.play_skill_field(skill_id, SkillField::Effect, entity_id, None, false);
                    self.play_skill_field(skill_id, SkillField::EffectOnCaster, source_entity_id, None, false);
                    if result == 1 {
                        self.play_skill_field(skill_id, SkillField::Success, entity_id, None, false);
                        self.play_skill_field(skill_id, SkillField::SuccessOnCaster, source_entity_id, None, false);
                    }
                    if let Some(entity) = self
                        .client_state
                        .follow(client_state().entities())
                        .iter()
                        .find(|entity| entity.get_entity_id() == entity_id)
                        .or_else(|| self.client_state.try_follow(this_entity()))
                    {
                        self.particle_holder
                            .spawn_particle(Box::new(HealNumber::new(entity.get_position(), heal_amount.to_string())));
                    }
                }
                NetworkEvent::PlayerHealEffect { heal_type, heal_amount } => {
                    let kind = match heal_type {
                        HealType::Health => "HP",
                        HealType::SpellPoints => "SP",
                    };
                    self.client_state.follow_mut(client_state().chat_messages()).push(ChatMessage::new(
                        format!("Recovered {heal_amount} {kind}."),
                        MessageColor::Information,
                    ));

                    if let Some(entity) = self.client_state.try_follow(this_entity()) {
                        self.particle_holder
                            .spawn_particle(Box::new(HealNumber::new(entity.get_position(), heal_amount.to_string())));
                    }
                }
                NetworkEvent::BankOpened { .. } => {
                    self.client_state.follow_mut(client_state().bank_window()).note_open();
                    self.interface.open_window(BankWindow::new());
                }
                NetworkEvent::BankBalance { money, reason } => {
                    self.client_state
                        .follow_mut(client_state().bank_window())
                        .note_balance(money, reason);
                    self.interface.open_window(BankWindow::new());
                }
                NetworkEvent::BankDeposit { reason, money, zeny } => {
                    self.client_state
                        .follow_mut(client_state().bank_window())
                        .note_deposit(reason, money, zeny);
                }
                NetworkEvent::BankWithdraw { reason, money, zeny } => {
                    self.client_state
                        .follow_mut(client_state().bank_window())
                        .note_withdraw(reason, money, zeny);
                }
                NetworkEvent::BankClosed { .. } => {
                    self.client_state.follow_mut(client_state().bank_window()).note_closed();
                    self.interface.close_window_with_class(WindowClass::Bank);
                }
                NetworkEvent::RouletteOpened {
                    result,
                    step,
                    index,
                    bonus_item_id,
                    gold,
                    silver,
                    bronze,
                    ..
                } => {
                    self.client_state.follow_mut(client_state().roulette_window()).note_open(
                        &crate::state::bank_roulette::RouletteSpinView {
                            result: i32::from(result),
                            step: i32::from(step),
                            index: i32::from(index),
                            bonus_item_id,
                            gold,
                            silver,
                            bronze,
                        },
                    );
                    self.interface.open_window(RouletteWindow::new());
                }
                NetworkEvent::RoulettePrizes { prizes, .. } => {
                    self.client_state.follow_mut(client_state().roulette_window()).note_prizes(&prizes);
                }
                NetworkEvent::RouletteGenerated {
                    result,
                    step,
                    index,
                    bonus_item_id,
                    gold,
                    silver,
                    bronze,
                } => {
                    self.client_state.follow_mut(client_state().roulette_window()).note_spin(
                        &crate::state::bank_roulette::RouletteSpinView {
                            result: i32::from(result),
                            step: i32::from(step),
                            index: i32::from(index),
                            bonus_item_id,
                            gold,
                            silver,
                            bronze,
                        },
                        client_tick.0,
                    );
                }
                NetworkEvent::RouletteClaimed { result, item_id } => {
                    self.client_state
                        .follow_mut(client_state().roulette_window())
                        .note_claim(result, item_id);
                }
                NetworkEvent::RouletteClosed { result } => {
                    self.client_state.follow_mut(client_state().roulette_window()).note_closed(result);
                    self.interface.close_window_with_class(WindowClass::Roulette);
                }
                NetworkEvent::GainedExperience {
                    amount,
                    experience_type,
                    experience_source,
                    ..
                } => {
                    let kind = match experience_type {
                        ExperienceType::BaseExperience => "Base",
                        ExperienceType::JobExperience => "Job",
                    };
                    let source = match experience_source {
                        ExperienceSource::Quest => " (quest)",
                        ExperienceSource::Regular => "",
                    };
                    self.client_state.follow_mut(client_state().chat_messages()).push(ChatMessage::new(
                        format!("Gained {amount} {kind} EXP{source}."),
                        MessageColor::Information,
                    ));
                }
                NetworkEvent::SpecialEffect { entity_id, effect_id } => {
                    let effect_value = effects::effect_id_value(effect_id);
                    // Unmapped effects are skipped. `chat_line_for_special_effect` is the
                    // single place that could ask for a chat line, and it never does.
                    if let Some(line) = effects::chat_line_for_special_effect(effect_value) {
                        self.client_state
                            .follow_mut(client_state().chat_messages())
                            .push(ChatMessage::new(line.to_string(), MessageColor::Information));
                    }
                    let paths = effects::collect_str_paths(&[effects::EffectRef::Id(effect_value)]);
                    self.spawn_str_effects(&paths, entity_id, None, false);
                    if matches!(effect_id, EffectId::Heal | EffectId::Heal2 | EffectId::Heal3 | EffectId::Heal4)
                        && let Some(entity) = self
                            .client_state
                            .follow(client_state().entities())
                            .iter()
                            .find(|entity| entity.get_entity_id() == entity_id)
                            .or_else(|| self.client_state.try_follow(this_entity()))
                    {
                        self.particle_holder
                            .spawn_particle(Box::new(HealNumber::new(entity.get_position(), "+".to_owned())));
                    }
                }
                NetworkEvent::SkillEffectAndDamage {
                    skill_id,
                    source_entity_id,
                    destination_entity_id,
                    damage,
                    destination_delay,
                    source_delay,
                    ..
                } => {
                    let target_position = self
                        .client_state
                        .follow(client_state().entities())
                        .iter()
                        .find(|entity| entity.get_entity_id() == destination_entity_id)
                        .map(|entity| entity.get_tile_position());

                    let attack_duration = if destination_delay != 0 { destination_delay } else { source_delay };

                    if let Some(entity) = self
                        .client_state
                        .follow_mut(client_state().entities())
                        .iter_mut()
                        .find(|entity| entity.get_entity_id() == source_entity_id)
                    {
                        if let Some(target_position) = target_position {
                            entity.rotate_towards(target_position);
                        }

                        entity.set_attack(attack_duration, false, client_tick);
                    }

                    if let Some(entity) = self
                        .client_state
                        .follow(client_state().entities())
                        .iter()
                        .find(|entity| entity.get_entity_id() == destination_entity_id)
                        .or_else(|| {
                            self.client_state
                                .try_follow(this_entity())
                                .filter(|entity| entity.get_entity_id() == destination_entity_id)
                        })
                    {
                        self.particle_holder
                            .spawn_particle(Box::new(DamageNumber::new(entity.get_position(), damage.to_string(), false)));
                    }

                    self.play_skill_field(skill_id, SkillField::Effect, destination_entity_id, None, false);
                    self.play_skill_field(skill_id, SkillField::EffectOnCaster, source_entity_id, None, false);
                    self.play_skill_field(skill_id, SkillField::BeforeHit, destination_entity_id, None, false);
                    self.play_skill_field(skill_id, SkillField::BeforeHitOnCaster, source_entity_id, None, false);
                    if damage > 0 {
                        self.play_skill_field(skill_id, SkillField::Hit, destination_entity_id, None, false);
                        self.play_skill_field(skill_id, SkillField::HitOnCaster, source_entity_id, None, false);
                    }
                }
                NetworkEvent::UpdateEntityHealth {
                    entity_id,
                    health_points,
                    maximum_health_points,
                } => {
                    let entity = self
                        .client_state
                        .follow_mut(client_state().entities())
                        .iter_mut()
                        .find(|entity| entity.get_entity_id() == entity_id);

                    if let Some(entity) = entity {
                        entity.update_health(health_points, maximum_health_points);
                    }
                }
                NetworkEvent::UpdateStat { stat_type } => {
                    if let Some(player) = self.client_state.try_follow_mut(this_player()) {
                        player.update_stat(stat_type);
                    }
                }
                NetworkEvent::StatUpResult { success, .. } => {
                    if let Some(notice) = crate::world::stat_up_feedback(success) {
                        self.client_state
                            .follow_mut(client_state().chat_messages())
                            .push(ChatMessage::new(notice.to_owned(), MessageColor::Error));
                    }
                }
                NetworkEvent::OpenEnchant { enchant_group } => {
                    self.client_state.follow_mut(client_state().enchant_window()).open(enchant_group);
                    self.interface.open_window(EnchantWindow::new());
                }
                NetworkEvent::EnchantResult { message_id, item_id } => {
                    self.client_state
                        .follow_mut(client_state().enchant_window())
                        .show_result(message_id, item_id);
                }
                NetworkEvent::StableHomunculusList { active_id, homunculi, .. } => {
                    self.client_state
                        .follow_mut(client_state().stable_window())
                        .show_homunculi(active_id, &homunculi);
                    self.interface.open_window(StableWindow::new());
                }
                NetworkEvent::StablePetList { active_id, pets, .. } => {
                    let faces = crate::interface::windows::stable::pet_face_list(&self.async_loader, &pets);
                    self.client_state
                        .follow_mut(client_state().stable_window())
                        .show_pets(active_id, &pets, &faces);
                    self.interface.open_window(StableWindow::new());
                }
                NetworkEvent::StableOwnedList { items, .. } => {
                    let ids: Vec<i32> = items.iter().map(|item| item.item_id).collect();
                    let faces = crate::interface::windows::stable::item_faces(&self.async_loader, &ids);
                    self.client_state
                        .follow_mut(client_state().stable_window())
                        .show_owned(&items, &faces);
                    self.interface.open_window(StableWindow::new());
                }
                NetworkEvent::StableResult { result, id, .. } => {
                    self.client_state.follow_mut(client_state().stable_window()).show_result(result, id);
                    self.interface.open_window(StableWindow::new());
                }
                NetworkEvent::EdenQuestList { summary, .. } => {
                    self.client_state.follow_mut(client_state().eden_window()).show_quests(summary);
                    self.interface.open_window(EdenWindow::new());
                }
                NetworkEvent::EdenShop { summary, .. } => {
                    self.client_state.follow_mut(client_state().eden_window()).show_shop(summary);
                    self.interface.open_window(EdenWindow::new());
                }
                NetworkEvent::EdenResult { summary, .. } => {
                    self.client_state.follow_mut(client_state().eden_window()).show_result(summary);
                    self.interface.open_window(EdenWindow::new());
                }
                NetworkEvent::WarpraList {
                    zeny,
                    price,
                    flags,
                    total,
                    destinations,
                } => {
                    self.client_state
                        .follow_mut(client_state().warpra_window())
                        .apply_list(zeny, price, flags, total, destinations);
                    let reveal = self.client_state.follow_mut(client_state().warpra_window()).take_text_surface();
                    if reveal {
                        self.interface.open_window(WarpraWindow::new());
                    }
                    if self.interface.is_window_with_class_open(WindowClass::WorldMap) {
                        let status = self.client_state.follow(client_state().warpra_window()).status().to_owned();
                        self.client_state.follow_mut(client_state().world_map_window()).set_status(status);
                    }
                }
                NetworkEvent::WarpraResult {
                    result, kind, location_id, ..
                } => {
                    let message = self
                        .client_state
                        .follow_mut(client_state().warpra_window())
                        .show_result(result, kind, location_id);
                    let action = self.client_state.follow(client_state().warpra_window()).last_action();
                    let reveal = self.client_state.follow_mut(client_state().warpra_window()).take_text_surface();
                    if result != 0 {
                        self.client_state
                            .follow_mut(client_state().chat_messages())
                            .push(ChatMessage::new(message.clone(), MessageColor::Error));
                        if self.interface.is_window_with_class_open(WindowClass::WorldMap) {
                            self.client_state.follow_mut(client_state().world_map_window()).set_status(message);
                        }
                    }
                    if should_close_after_result(result, action) {
                        self.interface.close_window_with_class(WindowClass::WorldMap);
                        self.client_state.follow_mut(client_state().world_map_window()).close_floors();
                    }
                    if reveal || self.interface.is_window_with_class_open(WindowClass::Warpra) {
                        self.interface.open_window(WarpraWindow::new());
                    }
                }
                NetworkEvent::PetInfo {
                    summary,
                    name,
                    renamed,
                    level,
                    hunger,
                    intimacy,
                    accessory_id,
                    class_id,
                } => {
                    self.client_state.follow_mut(client_state().companion_window()).show_pet(
                        crate::interface::windows::PetPanel {
                            name,
                            renamed,
                            level,
                            hunger,
                            intimacy,
                            accessory: accessory_id,
                            class_id,
                        },
                        summary,
                    );
                    self.interface.open_window(CompanionWindow::new());
                }
                NetworkEvent::PetState { kind, value, .. } => {
                    self.client_state
                        .follow_mut(client_state().companion_window())
                        .apply_pet_state(kind, value);
                    self.interface.open_window(CompanionWindow::new());
                }
                NetworkEvent::HomunculusInfo { summary, .. } => {
                    self.client_state
                        .follow_mut(client_state().companion_window())
                        .show_homunculus(summary);
                    self.interface.open_window(CompanionWindow::new());
                }
                NetworkEvent::MercenaryInfo {
                    summary,
                    name,
                    level,
                    attack,
                    magic_attack,
                    hit,
                    critical,
                    defense,
                    magic_defense,
                    flee,
                    attack_speed,
                    health_points,
                    maximum_health_points,
                    spell_points,
                    maximum_spell_points,
                    expire_date,
                    faith,
                    summons,
                    kills,
                    attack_range,
                    account_id,
                } => {
                    self.client_state.follow_mut(client_state().companion_window()).show_mercenary(
                        crate::interface::windows::MercenaryPanel {
                            name,
                            level,
                            attack,
                            magic_attack,
                            hit,
                            critical,
                            defense,
                            magic_defense,
                            flee,
                            attack_speed,
                            health_points,
                            maximum_health_points,
                            spell_points,
                            maximum_spell_points,
                            expire_date,
                            faith,
                            summons,
                            kills,
                            attack_range,
                        },
                        account_id == 0,
                        summary,
                    );
                    self.interface.open_window(CompanionWindow::new());
                }
                NetworkEvent::MercenaryParameter { variable, value } => {
                    self.client_state
                        .follow_mut(client_state().companion_window())
                        .apply_mercenary_parameter(variable, value);
                    self.interface.open_window(CompanionWindow::new());
                }
                NetworkEvent::ElementalInfo {
                    summary,
                    class_id,
                    health_points,
                    maximum_health_points,
                    spell_points,
                    maximum_spell_points,
                } => {
                    self.client_state.follow_mut(client_state().companion_window()).show_elemental(
                        class_id,
                        health_points,
                        maximum_health_points,
                        spell_points,
                        maximum_spell_points,
                        summary,
                    );
                    self.interface.open_window(CompanionWindow::new());
                }
                NetworkEvent::ElementalParameter { variable, value } => {
                    self.client_state
                        .follow_mut(client_state().companion_window())
                        .apply_elemental_parameter(variable, value);
                    self.interface.open_window(CompanionWindow::new());
                }
                NetworkEvent::MvpMinimap { flags, markers } => {
                    self.client_state.follow_mut(client_state().mvp_minimap()).replace(flags, markers);
                }
                NetworkEvent::NewMailStatus { available } => {
                    self.client_state.follow_mut(client_state().mail_window()).set_new_mail(available);
                    if available {
                        self.interface.open_window(MailWindow::new());
                    }
                }
                NetworkEvent::RodexMailList {
                    summary,
                    is_end,
                    next_mail_id,
                } => {
                    self.client_state
                        .follow_mut(client_state().mail_window())
                        .set_list(summary, is_end, next_mail_id);
                    self.interface.open_window(MailWindow::new());
                }
                NetworkEvent::RodexWriteResult { summary, .. } => {
                    self.client_state.follow_mut(client_state().mail_window()).set_status(summary);
                    self.interface.open_window(MailWindow::new());
                }
                NetworkEvent::RodexDelete { mail_id, .. } => {
                    self.client_state
                        .follow_mut(client_state().mail_window())
                        .set_status(format!("Deleted mail #{mail_id}."));
                }
                NetworkEvent::RodexZeny { mail_id, result, summary } => {
                    self.client_state
                        .follow_mut(client_state().mail_window())
                        .note_zeny(mail_id, result, summary);
                }
                NetworkEvent::RodexItems { mail_id, result, summary } => {
                    self.client_state
                        .follow_mut(client_state().mail_window())
                        .note_items(mail_id, result, summary);
                }
                NetworkEvent::RodexRead {
                    mail_id,
                    zeny,
                    item_count,
                    summary,
                } => {
                    self.client_state
                        .follow_mut(client_state().mail_window())
                        .set_read(mail_id, zeny, item_count, summary);
                    self.interface.open_window(MailWindow::new());
                }
                NetworkEvent::OpenDialog { text, npc_id } => {
                    self.client_state
                        .follow_mut(client_state().dialog_window())
                        .initialize(npc_id)
                        .add_text(text);

                    self.interface.open_window(DialogWindow::new(client_state().dialog_window()));
                }
                NetworkEvent::AddNextButton { npc_id } => {
                    self.client_state
                        .follow_mut(client_state().dialog_window())
                        // An NPCs could start the dialog with this packet so we want to make sure it's initialized.
                        .initialize(npc_id)
                        .add_next_button();

                    self.interface.open_window(DialogWindow::new(client_state().dialog_window()));
                }
                NetworkEvent::AddCloseButton { npc_id } => {
                    // Some NPCs send the `CloseButtonPacket` after the dialog
                    // has been closed. We want to filter these out because otherwise we get a
                    // close button at the start of the next dialog.
                    if self.interface.is_window_with_class_open(WindowClass::Dialog) {
                        self.client_state
                            .follow_mut(client_state().dialog_window())
                            // Technically this call is redundant since the window is already open
                            // but we keep it for consistency.
                            .initialize(npc_id)
                            .add_close_button();
                    }
                }
                NetworkEvent::AddChoiceButtons { choices, npc_id } => {
                    self.client_state
                        .follow_mut(client_state().dialog_window())
                        // Some NPCs start the dialog with this packet so we need to make sure it's initialized.
                        .initialize(npc_id)
                        .add_choice_buttons(choices);

                    self.interface.open_window(DialogWindow::new(client_state().dialog_window()));
                }
                NetworkEvent::NpcNumberInput { npc_id } => {
                    self.client_state
                        .follow_mut(client_state().dialog_window())
                        .initialize(npc_id)
                        .open_number_input();
                    self.interface.open_window(NpcInputWindow::new());
                }
                NetworkEvent::NpcTextInput { npc_id } => {
                    self.client_state
                        .follow_mut(client_state().dialog_window())
                        .initialize(npc_id)
                        .open_text_input();
                    self.interface.open_window(NpcInputWindow::new());
                }
                NetworkEvent::NpcCutin { image_name, location } => {
                    let anchor = cutin_anchor(location);
                    let path = existing_cutin_texture(&image_name, |full| self.game_file_loader.file_exists(full));
                    let presentation = self.client_state.follow_mut(client_state().npc_presentation());
                    match (anchor, path) {
                        (Some(anchor), Some(path)) => presentation.show_cutin(anchor, path),
                        _ => presentation.clear_cutin(),
                    }
                }
                NetworkEvent::NpcProgress { color, seconds } => {
                    self.client_state
                        .follow_mut(client_state().npc_presentation())
                        .start_progress(color, seconds, client_tick.0);
                }
                NetworkEvent::NpcProgressCancel => {
                    if self.client_state.follow_mut(client_state().npc_presentation()).cancel_from_server() {
                        let _ = self.networking_system.npc_progress_complete();
                    }
                }
                NetworkEvent::NpcSound {
                    file_name, action, npc_id, ..
                } => crate::interface::npc_flow::play_npc_sound(&self.audio_engine, &self.client_state, &file_name, action, npc_id),
                NetworkEvent::NpcBgm { track, play_type } => {
                    crate::interface::npc_flow::play_npc_bgm(
                        &self.audio_engine,
                        &mut self.client_state,
                        self.map.as_ref(),
                        &track,
                        play_type,
                    );
                }
                NetworkEvent::AddQuestEffect { quest_effect } => {
                    if let Some(map) = &self.map {
                        self.particle_holder.add_quest_icon(&self.texture_loader, map, quest_effect)
                    }
                }
                NetworkEvent::RemoveQuestEffect { entity_id } => self.particle_holder.remove_quest_icon(entity_id),
                NetworkEvent::SetInventory { items } => {
                    self.client_state
                        .follow_mut(client_state().inventory())
                        .fill(&self.async_loader, items);
                }
                NetworkEvent::SetStorage { items } => {
                    self.client_state
                        .follow_mut(client_state().storage())
                        .fill(&self.async_loader, items);
                    self.interface.open_window(StorageWindow::new(client_state().storage().items()));
                }
                NetworkEvent::SetCart { items } => {
                    let open_cart = !items.is_empty();
                    self.client_state.follow_mut(client_state().cart()).fill(&self.async_loader, items);

                    if open_cart
                        && self.client_state.try_follow(this_entity()).is_some()
                        && !self.interface.is_window_with_class_open(WindowClass::Cart)
                    {
                        self.interface.open_window(CartWindow::new(client_state().cart().items()));
                    }
                }
                NetworkEvent::CartItemAdded { item } => {
                    self.client_state
                        .follow_mut(client_state().cart())
                        .add_item(&self.async_loader, item);
                }
                NetworkEvent::CartItemRemoved { index, amount } => {
                    self.client_state.follow_mut(client_state().cart()).remove_item(index, amount);
                }
                NetworkEvent::IventoryItemAdded { item } => {
                    self.client_state
                        .follow_mut(client_state().inventory())
                        .add_item(&self.async_loader, item);

                    // TODO: Update the selling items. If you pick up an item
                    // that you already have the sell window
                    // should allow you to sell the new
                    // amount of items.
                }
                NetworkEvent::ItemObtained {
                    item_id,
                    quantity,
                    is_identified,
                } => {
                    let name = self.library.get::<ItemName>(ItemNameKey { item_id, is_identified }).to_string();
                    let message = format!("You got {name} ({quantity}).");
                    self.client_state
                        .follow_mut(client_state().chat_messages())
                        .push(ChatMessage::new(message, MessageColor::Information));
                }
                NetworkEvent::InventoryItemRemoved { index, amount, .. } => {
                    self.client_state.follow_mut(client_state().inventory()).remove_item(index, amount);
                }
                NetworkEvent::ItemUseAck {
                    index,
                    account_id,
                    amount,
                    success,
                    ..
                } => {
                    let ours = self.saved_login_data.as_ref().map(|login| login.account_id);
                    if use_ack_targets_self(ours, account_id) {
                        if success {
                            self.client_state
                                .follow_mut(client_state().inventory())
                                .apply_use_ack(index, amount);
                        } else {
                            self.client_state
                                .follow_mut(client_state().chat_messages())
                                .push(ChatMessage::new("Could not use that item.".to_owned(), MessageColor::Error));
                        }
                    }
                }
                NetworkEvent::ItemThrowAck { index, amount } => match self
                    .client_state
                    .follow_mut(client_state().inventory())
                    .apply_throw_ack(index, amount)
                {
                    ThrowUpdate::Failed => {
                        self.client_state
                            .follow_mut(client_state().chat_messages())
                            .push(ChatMessage::new("Could not drop that item.".to_owned(), MessageColor::Error));
                    }
                    ThrowUpdate::Missing | ThrowUpdate::Updated => {}
                },
                NetworkEvent::SkillTree { skill_information } => {
                    *self.client_state.follow_mut(client_state().skill_tree().skills()) =
                        skill_information.into_iter().map(LearnedSkill::new).collect();
                }
                NetworkEvent::UpdateEquippedPosition { index, equipped_position } => {
                    self.client_state
                        .follow_mut(client_state().inventory())
                        .update_equipped_position(index, equipped_position);
                }
                NetworkEvent::SetEquipSwitch { entries } => {
                    let items = self.client_state.follow(client_state().inventory().items()).clone();
                    self.client_state.follow_mut(client_state().equip_switch()).replace(entries, &items);
                }
                NetworkEvent::UpdateEquipSwitch { index, position } => {
                    let items = self.client_state.follow(client_state().inventory().items()).clone();
                    self.client_state
                        .follow_mut(client_state().equip_switch())
                        .assign(index, position, &items);
                }
                NetworkEvent::EquipSwitchReply { failed: true } => {
                    self.client_state
                        .follow_mut(client_state().chat_messages())
                        .push(ChatMessage::new("Equipment switch failed.".to_owned(), MessageColor::Error));
                }
                NetworkEvent::EquipSwitchReply { failed: false } => {}
                NetworkEvent::ChangeJob { account_id, job_id } => {
                    let layout = self.async_loader.request_skill_tree_layout_load(job_id, client_tick);
                    *self.client_state.follow_mut(client_state().skill_tree_window().selected_tab()) = layout.tabs.len().saturating_sub(1);
                    *self.client_state.follow_mut(client_state().skill_tree().layout()) = layout;
                    self.client_state
                        .follow_mut(client_state().skill_tree_window().chosen_skill_level())
                        .clear();

                    let entity = self
                        .client_state
                        .follow_mut(client_state().entities())
                        .iter_mut()
                        .find(|entity| entity.get_entity_id().0 == account_id.0)
                        .unwrap();

                    // FIX: A job change does not automatically send packets for the
                    // inventory and for unequipping items. We should probably manually
                    // request a full list of items and the hotbar.

                    entity.set_job(&self.library, job_id);

                    if let Some(animation_data) = self.async_loader.request_animation_data_load(
                        entity.get_entity_id(),
                        entity.get_entity_type(),
                        entity.get_entity_part_files(&self.library),
                    ) {
                        entity.set_animation_data(animation_data);
                    }
                }
                NetworkEvent::ChangeHair { account_id, hair_id } => {
                    Self::apply_account_look(
                        &mut self.client_state,
                        &self.async_loader,
                        &self.library,
                        account_id,
                        ragnarok_packets::SpriteChangeType::Hair,
                        hair_id,
                    );
                }
                NetworkEvent::ChangeHairColor { account_id, color } => {
                    Self::apply_account_look(
                        &mut self.client_state,
                        &self.async_loader,
                        &self.library,
                        account_id,
                        ragnarok_packets::SpriteChangeType::HairCollor,
                        color,
                    );
                }
                NetworkEvent::ChangeClothColor { account_id, color } => {
                    Self::apply_account_look(
                        &mut self.client_state,
                        &self.async_loader,
                        &self.library,
                        account_id,
                        ragnarok_packets::SpriteChangeType::ClothesColor,
                        color,
                    );
                }
                NetworkEvent::ChangeHeadTop { account_id, view_id } => {
                    Self::apply_account_look(
                        &mut self.client_state,
                        &self.async_loader,
                        &self.library,
                        account_id,
                        ragnarok_packets::SpriteChangeType::HeadTop,
                        view_id,
                    );
                }
                NetworkEvent::ChangeHeadMid { account_id, view_id } => {
                    Self::apply_account_look(
                        &mut self.client_state,
                        &self.async_loader,
                        &self.library,
                        account_id,
                        ragnarok_packets::SpriteChangeType::HeadMiddle,
                        view_id,
                    );
                }
                NetworkEvent::ChangeHeadBottom { account_id, view_id } => {
                    Self::apply_account_look(
                        &mut self.client_state,
                        &self.async_loader,
                        &self.library,
                        account_id,
                        ragnarok_packets::SpriteChangeType::HeadBottom,
                        view_id,
                    );
                }
                NetworkEvent::LoggedOut => {
                    self.client_state.follow_mut(client_state().party()).clear();
                    self.networking_system.disconnect_from_map_server();
                }
                NetworkEvent::FriendRequest { requestee } => {
                    self.interface.open_window(FriendRequestWindow::new(requestee));
                }
                NetworkEvent::FriendRemoved { account_id, character_id } => {
                    self.client_state
                        .follow_mut(client_state().friend_list())
                        .retain(|friend| !(friend.account_id == account_id && friend.character_id == character_id));
                }
                NetworkEvent::FriendOnlineStatus {
                    account_id,
                    character_id,
                    online,
                    ..
                } => {
                    let friends = self.client_state.follow_mut(client_state().friend_list());
                    if let Some(entry) = friends
                        .iter_mut()
                        .find(|entry| entry.account_id == account_id && entry.character_id == character_id)
                    {
                        entry.set_online(online);
                    }
                }
                NetworkEvent::PartyInvite { party_id, party_name } => {
                    self.client_state.follow_mut(client_state().party()).set_pending_invite(party_id);
                    self.interface.open_window(PartyInviteWindow::new(party_id, party_name));
                }
                NetworkEvent::PartyInvitationState { invitations_allowed } => {
                    self.client_state
                        .follow_mut(client_state().party())
                        .set_invitations_allowed(invitations_allowed);
                }
                NetworkEvent::Party(update) => {
                    if let PartyUpdate::Health {
                        account_id,
                        health,
                        maximum,
                    } = &update
                        && *maximum > 0
                    {
                        let entity_id = EntityId(account_id.0);
                        let health = (*health).max(0) as usize;
                        let maximum = *maximum as usize;
                        if let Some(entity) = self
                            .client_state
                            .follow_mut(client_state().entities())
                            .iter_mut()
                            .find(|entity| entity.get_entity_id() == entity_id)
                        {
                            entity.update_health(health, maximum);
                        }
                    }
                    let account_id = self.saved_login_data.as_ref().map(|login| login.account_id).unwrap_or(AccountId(0));
                    if let Some(notice) = self.client_state.follow_mut(client_state().party()).apply(update, account_id) {
                        let color = match notice.error {
                            true => MessageColor::Error,
                            false => MessageColor::Information,
                        };
                        self.client_state
                            .follow_mut(client_state().chat_messages())
                            .push(ChatMessage::new(notice.text, color));
                    }
                }
                NetworkEvent::GuildAffiliation {
                    guild_id,
                    name,
                    is_master,
                    master_character_id,
                    emblem_version,
                } => {
                    let previous_id = self.client_state.follow(client_state().guild()).guild_id;
                    let joined = {
                        let guild = self.client_state.follow_mut(client_state().guild());
                        guild.apply_affiliation(guild_id, name, is_master, master_character_id, emblem_version);
                        guild.joined
                    };
                    if !joined || previous_id != guild_id {
                        self.client_state.follow_mut(client_state().guild_window()).clear();
                    }
                    let version = u32::try_from(emblem_version).unwrap_or(0);
                    apply_local_guild_mark(&mut self.client_state, &mut self.networking_system, guild_id, version);
                }
                NetworkEvent::GuildInfo {
                    guild_id,
                    level,
                    online_members,
                    max_members,
                    average_level,
                    experience,
                    next_experience,
                    tax_points,
                    honor,
                    virtue,
                    emblem_version,
                    name,
                    master_name,
                    territory,
                    master_character_id,
                } => {
                    let previous_id = self.client_state.follow(client_state().guild()).guild_id;
                    self.client_state
                        .follow_mut(client_state().guild())
                        .apply_info(crate::state::guild::GuildInfoUpdate {
                            guild_id,
                            level,
                            online_members,
                            max_members,
                            average_level,
                            experience,
                            next_experience,
                            tax_points,
                            honor,
                            virtue,
                            emblem_version,
                            name,
                            master_name,
                            territory,
                            master_character_id,
                        });
                    if previous_id != guild_id {
                        self.client_state.follow_mut(client_state().guild_window()).clear();
                    }
                    let version = u32::try_from(emblem_version).unwrap_or(0);
                    apply_local_guild_mark(&mut self.client_state, &mut self.networking_system, guild_id, version);
                }
                NetworkEvent::GuildMembers { members } => {
                    self.client_state.follow_mut(client_state().guild()).set_members(members);
                }
                NetworkEvent::GuildPositions { positions } => {
                    self.client_state.follow_mut(client_state().guild()).set_positions(positions);
                }
                NetworkEvent::GuildNotice { subject, notice } => {
                    self.client_state
                        .follow_mut(client_state().guild())
                        .set_notice(subject.clone(), notice.clone());
                    let drafts = self.client_state.follow_mut(client_state().guild_window());
                    drafts.subject = subject;
                    drafts.notice = notice;
                }
                NetworkEvent::GuildMemberOnline {
                    account_id,
                    character_id,
                    online,
                } => {
                    self.client_state
                        .follow_mut(client_state().guild())
                        .set_member_online(account_id, character_id, online);
                }
                NetworkEvent::ClanInfo {
                    clan_id,
                    name,
                    master_name,
                    map_name,
                    alliances,
                    antagonists,
                } => {
                    self.client_state
                        .follow_mut(client_state().clan())
                        .apply_info(crate::state::clan::ClanInfoUpdate {
                            clan_id,
                            name,
                            master_name,
                            map_name,
                            alliances,
                            antagonists,
                        });
                }
                NetworkEvent::ClanOnlineCount {
                    online_members,
                    maximum_members,
                } => {
                    self.client_state
                        .follow_mut(client_state().clan())
                        .set_online_count(online_members, maximum_members);
                }
                NetworkEvent::ClanLeft => {
                    self.client_state.follow_mut(client_state().clan()).clear();
                }
                NetworkEvent::GuildMemberRemoved { character_id } => {
                    let self_id = *self.client_state.follow(client_state().player_character_id());
                    let left = self
                        .client_state
                        .follow_mut(client_state().guild())
                        .apply_member_removed(character_id, self_id);
                    if left {
                        self.client_state.follow_mut(client_state().guild_window()).clear();
                        apply_local_guild_mark(&mut self.client_state, &mut self.networking_system, 0, 0);
                    }
                }
                NetworkEvent::GuildMakeResult { result } => {
                    let text = crate::state::guild::make_guild_result_text(result);
                    self.client_state.follow_mut(client_state().guild()).set_status(text.clone());
                    self.client_state
                        .follow_mut(client_state().chat_messages())
                        .push(ChatMessage::new(text, MessageColor::Information));
                    if result == 0 {
                        let _ = self.networking_system.request_guild_window();
                    }
                }
                NetworkEvent::GuildInviteRequest { guild_id, name } => {
                    self.interface.open_window(GuildInviteWindow::new(guild_id, name));
                }
                NetworkEvent::GuildInviteResult { result } => {
                    let text = crate::state::guild::invite_result_text(result);
                    self.client_state.follow_mut(client_state().guild()).set_status(text.clone());
                    self.client_state
                        .follow_mut(client_state().chat_messages())
                        .push(ChatMessage::new(text, MessageColor::Information));
                }
                NetworkEvent::GuildRelations { relations } => {
                    self.client_state.follow_mut(client_state().guild()).set_relations(relations);
                }
                NetworkEvent::GuildRelationRemoved { guild_id, opposition } => {
                    self.client_state
                        .follow_mut(client_state().guild())
                        .remove_relation(guild_id, opposition);
                }
                NetworkEvent::GuildRelationAdded {
                    opposition,
                    guild_id,
                    name,
                } => {
                    self.client_state
                        .follow_mut(client_state().guild())
                        .add_relation(opposition, guild_id, name);
                }
                NetworkEvent::GuildPositionRights { positions } => {
                    self.client_state
                        .follow_mut(client_state().guild())
                        .set_position_rights(positions);
                }
                NetworkEvent::GuildPositionsUpdated { positions } => {
                    let guild = self.client_state.follow_mut(client_state().guild());
                    for (id, mode, ranking, pay_rate, name) in positions {
                        guild.apply_position_changed(id, mode, ranking, pay_rate, name);
                    }
                }
                NetworkEvent::GuildMemberPositions { entries } => {
                    let guild = self.client_state.follow_mut(client_state().guild());
                    for (account_id, character_id, position_id) in entries {
                        guild.set_member_position(account_id, character_id, position_id);
                    }
                }
                NetworkEvent::GuildSkills { skill_points, skills } => {
                    let skill_ids: Vec<u16> = skills.iter().map(|skill| skill.skill_id).collect();
                    self.client_state
                        .follow_mut(client_state().guild())
                        .set_skills(skill_points, skills);
                    load_guild_skill_icons(&mut self.client_state, &self.async_loader, &self.library, &skill_ids);
                }
                NetworkEvent::GuildChatMessage { text } => {
                    self.client_state
                        .follow_mut(client_state().chat_messages())
                        .push(ChatMessage::new(text, MessageColor::Guild));
                }
                NetworkEvent::GuildEmblemImage {
                    result,
                    guild_id,
                    emblem_version,
                    bytes,
                } => {
                    self.client_state.follow_mut(client_state().guild()).push_emblem_chunk(
                        result,
                        guild_id,
                        emblem_version,
                        &bytes,
                    );
                }
                NetworkEvent::GuildEmblemOnEntity {
                    account_id,
                    guild_id,
                    emblem_version,
                } => {
                    if let Some(entity) = self
                        .client_state
                        .follow_mut(client_state().entities())
                        .iter_mut()
                        .find(|entity| entity.get_entity_id().0 == account_id.0)
                    {
                        entity.set_guild(guild_id, emblem_version);
                    }
                    request_emblem_if_needed(&mut self.client_state, &mut self.networking_system, guild_id, emblem_version);
                }
                NetworkEvent::GuildMemberAdded { member } => {
                    self.client_state.follow_mut(client_state().guild()).add_member(member);
                }
                NetworkEvent::FriendAdded { friend } => {
                    self.client_state
                        .follow_mut(client_state().friend_list())
                        .push(crate::state::friends::FriendListEntry::from_friend(friend, false));
                }
                NetworkEvent::VisualEffect { effect_path, entity_id } => {
                    let effect = self.effect_loader.get_or_load(effect_path, &self.texture_loader).unwrap();
                    let frame_timer = effect.new_frame_timer();

                    self.effect_holder.add_effect(Box::new(EffectWithLight::new(
                        effect,
                        frame_timer,
                        EffectCenter::Entity(entity_id, Point3::new(0.0, 0.0, 0.0)),
                        Vector3::new(0.0, 9.0, 0.0),
                        // FIX: The point light id needs to be unique.
                        // The point light manager uses the id to decide which point light
                        // renders with a shadow. Having duplicate ids might cause some
                        // visual artifacts, such as flickering, as the point lights switch
                        // between shadows and no shadows.
                        PointLightId::new(entity_id.0),
                        Vector3::new(0.0, 12.0, 0.0),
                        Color::WHITE,
                        50.0,
                        false,
                    )));
                }
                NetworkEvent::AddSkillUnit {
                    entity_id,
                    unit_id,
                    position,
                } => {
                    let Some(map) = &self.map else {
                        continue;
                    };
                    let Some(position) = map.get_world_position(position) else {
                        #[cfg(feature = "debug")]
                        print_debug!("[{}] entity with id {:?} is out of map bounds", "error".red(), entity_id);
                        continue;
                    };
                    let Some(effect_ref) = effects::unit_effect(effects::unit_id_value(unit_id)) else {
                        #[cfg(feature = "debug")]
                        print_debug!("[{}] unmapped skill unit {:?}", "effect".red(), unit_id);
                        continue;
                    };
                    let paths = effects::collect_str_paths(&[effect_ref]);
                    self.spawn_str_effects(&paths, entity_id, Some(position), true);
                }
                NetworkEvent::RemoveSkillUnit { entity_id } => {
                    self.effect_holder.remove_unit(entity_id);
                }
                NetworkEvent::SetFriendList { friend_list } => {
                    *self.client_state.follow_mut(client_state().friend_list()) = friend_list
                        .into_iter()
                        .map(|friend| crate::state::friends::FriendListEntry::from_friend(friend, false))
                        .collect();
                }
                NetworkEvent::SetHotkeyData { tab, hotkeys } => {
                    // FIX: Since we only have one hotbar at the moment, we ignore
                    // everything but 0.
                    if tab.0 != 0 {
                        continue;
                    }

                    if let Some(job_id) = self.client_state.try_follow(this_entity()).map(Entity::get_job_id) {
                        for (index, hotkey) in hotkeys.into_iter().take(10).enumerate() {
                            match hotkey {
                                HotkeyState::Bound(hotkey) => {
                                    // TODO: Properly distinguish between skill and item.
                                    let skill_id = SkillId(hotkey.item_or_skill_id as u16);

                                    let mut skill = self.async_loader.request_learnable_skill_load(job_id, skill_id, client_tick);
                                    skill.maximum_level.0 = hotkey.quantity_or_skill_level;

                                    self.client_state
                                        .follow_mut(client_state().hotbar())
                                        .set_slot(HotbarSlot(index as u16), skill);
                                }
                                HotkeyState::Unbound => self
                                    .client_state
                                    .follow_mut(client_state().hotbar())
                                    .unset_slot(HotbarSlot(index as u16)),
                            }
                        }
                    }
                }
                NetworkEvent::PlayerShopSign {
                    account_id,
                    buying_store,
                    shop_name,
                } => self.on_player_shop_sign(account_id, shop_name, buying_store),
                NetworkEvent::PlayerShopClosed { account_id, buying_store } => {
                    self.client_state
                        .follow_mut(client_state().player_shops())
                        .retain(|shop| shop.account_id != account_id || shop.buying_store != buying_store);
                }
                NetworkEvent::PlayerShopOpened {
                    account_id,
                    store_id,
                    buying_store,
                    items,
                    indexes,
                } => {
                    let mut item_ids = Vec::with_capacity(items.len());
                    let rows = items
                        .into_iter()
                        .map(|item| {
                            item_ids.push(item.item_id);
                            let amount = match item.quantity {
                                ItemQuantity::Fixed(amount) => amount,
                                ItemQuantity::Infinite => 0,
                            };
                            let price = item.price.0;
                            let named = self.async_loader.request_shop_item_metadata_load(item);
                            let verb = if buying_store { "Wants" } else { "Sells" };
                            crate::state::VendorRow {
                                label: format!("{verb} {}  x{amount}  {price}z", named.metadata.name),
                                name: named.metadata.name,
                                price,
                                amount,
                            }
                        })
                        .collect();

                    *self.client_state.follow_mut(client_state().vendor_account_id()) = account_id;
                    *self.client_state.follow_mut(client_state().vendor_shop_id()) = store_id;
                    *self.client_state.follow_mut(client_state().vendor_buying()) = buying_store;
                    *self.client_state.follow_mut(client_state().vendor_indexes()) = indexes;
                    *self.client_state.follow_mut(client_state().vendor_item_ids()) = item_ids;
                    *self.client_state.follow_mut(client_state().vendor_rows()) = rows;
                    self.interface.open_window(VendorWindow::new(client_state().vendor_rows()));
                }
                NetworkEvent::VendingPrepare { item_count } => self.on_vending_prepare(item_count),
                NetworkEvent::BuyingStorePrepare { item_count } => self.on_buying_prepare(item_count),
                NetworkEvent::VendingOpenResult { result } => self.on_vending_open_result(result),
                NetworkEvent::OwnPlayerShopOpened { buying_store, lines } => self.on_own_shop_opened(buying_store, lines),
                NetworkEvent::VendingSold { index, amount, zeny, date } => self.on_vending_sold(index, amount, zeny, date),
                NetworkEvent::VendingPurchaseResult { index, amount, result } => {
                    if result == 0 {
                        let position = self
                            .client_state
                            .follow(client_state().vendor_indexes())
                            .iter()
                            .position(|slot| *slot == index);
                        if let Some(position) = position {
                            let rows = self.client_state.follow_mut(client_state().vendor_rows());
                            if let Some(row) = rows.get_mut(position) {
                                row.amount = row.amount.saturating_sub(u32::from(amount));
                                row.label = format!("Sells {}  x{}  {}z", row.name, row.amount, row.price);
                            }
                        }
                    } else {
                        self.client_state.follow_mut(client_state().chat_messages()).push(ChatMessage::new(
                            format!("Could not buy from the vending shop (code {result})."),
                            MessageColor::Error,
                        ));
                    }
                }
                NetworkEvent::BuyingStoreOpenFailed { result, .. } => {
                    self.client_state.follow_mut(client_state().chat_messages()).push(ChatMessage::new(
                        format!("Could not open a buying store (code {result})."),
                        MessageColor::Error,
                    ));
                }
                NetworkEvent::BuyingStoreTradeFailed { result } => {
                    self.client_state.follow_mut(client_state().chat_messages()).push(ChatMessage::new(
                        format!("Could not sell to the buying store (code {result})."),
                        MessageColor::Error,
                    ));
                }
                NetworkEvent::SearchStoreOpen { remaining_uses, .. } => self.on_search_store_open(remaining_uses),
                NetworkEvent::SearchStoreFailed { reason } => {
                    self.client_state.follow_mut(client_state().chat_messages()).push(ChatMessage::new(
                        format!("Shop search failed (code {reason})."),
                        MessageColor::Error,
                    ));
                }
                NetworkEvent::SearchStoreResults {
                    first_page,
                    next_page,
                    uses_remaining,
                    hits,
                } => self.on_search_store_results(first_page, next_page, uses_remaining, hits),
                NetworkEvent::SearchStorePosition { x, y } => {
                    self.client_state.follow_mut(client_state().chat_messages()).push(ChatMessage::new(
                        format!("Searched shop is at {x}, {y}."),
                        MessageColor::Information,
                    ));
                }
                NetworkEvent::BarterShopOpened { expanded, offers } => {
                    self.client_state.follow_mut(client_state().dialog_window()).end();
                    self.interface.close_window_with_class(WindowClass::Dialog);
                    let offers = crate::state::barter::offers_from_catalog(&self.async_loader, offers);
                    self.client_state.follow_mut(client_state().barter()).open_shop(expanded, offers);
                    self.interface.open_window(BarterWindow::new());
                }
                NetworkEvent::OpenShop { items } => {
                    // Close the dialog. Some NPCs don't use the `BuyOrSellPacket` and instead use
                    // the regular `DialogMenuPacket`. When opening the shop that dialog should be
                    // closed.
                    crate::interface::npc_flow::dismiss_npc_dialog_ui(&mut self.client_state, &mut self.interface);

                    *self.client_state.follow_mut(client_state().shop_items()) = items
                        .into_iter()
                        .map(|item| self.async_loader.request_shop_item_metadata_load(item))
                        .collect();

                    self.interface
                        .open_window(BuyWindow::new(client_state().shop_items(), client_state().buy_cart()));
                    self.interface.open_window(BuyCartWindow::new(client_state().buy_cart()));
                }
                NetworkEvent::AskBuyOrSell { shop_id } => {
                    self.interface.open_window(BuyOrSellWindow::new(shop_id));
                }
                NetworkEvent::BuyingCompleted { result } => match result {
                    BuyShopItemsResult::Success => {
                        let _ = self.networking_system.close_shop();

                        // Clear the cart.
                        self.client_state.follow_mut(client_state().buy_cart()).clear();

                        self.interface.close_window_with_class(WindowClass::Buy);
                        self.interface.close_window_with_class(WindowClass::BuyCart);
                    }
                    BuyShopItemsResult::Error => {
                        self.client_state
                            .follow_mut(client_state().chat_messages())
                            .push(ChatMessage::new("Failed to buy items".to_owned(), MessageColor::Error));
                    }
                },
                NetworkEvent::NpcBuyResult { result } => {
                    // Regular NPC buy also uses 0x00CA and previously ignored it. Only barter
                    // treats this byte as the exchange result and answers with a barter close.
                    if self.client_state.follow(client_state().barter()).is_open() {
                        finish_barter_purchase(&mut self.networking_system, &mut self.client_state, &mut self.interface, result);
                    }
                }
                NetworkEvent::SellItemList { items } => {
                    // Close the dialog. Some NPCs don't use the `BuyOrSellPacket` and instead use
                    // the regular `DialogMenuPacket`. When opening the shop that dialog should be
                    // closed.
                    crate::interface::npc_flow::dismiss_npc_dialog_ui(&mut self.client_state, &mut self.interface);

                    let inventory_items = self.client_state.follow(client_state().inventory().items());
                    let sell_items = items
                        .into_iter()
                        .map(|item| {
                            let inventory_item = inventory_items
                                .iter()
                                .find(|inventory_item| inventory_item.index == item.inventory_index)
                                .expect("item not in inventory");

                            let name = inventory_item.metadata.name.clone();
                            let texture = inventory_item.metadata.texture.clone();
                            let description = inventory_item.metadata.description.clone();
                            let quantity = match &inventory_item.details {
                                korangar_networking::InventoryItemDetails::Regular { amount, .. } => *amount,
                                korangar_networking::InventoryItemDetails::Equippable { .. } => 1,
                            };

                            SellItem {
                                metadata: (
                                    ResourceMetadata {
                                        name,
                                        texture,
                                        description,
                                    },
                                    quantity,
                                ),
                                inventory_index: item.inventory_index,
                                price: item.price,
                                overcharge_price: item.overcharge_price,
                            }
                        })
                        .collect();

                    *self.client_state.follow_mut(client_state().sell_items()) = sell_items;

                    self.interface
                        .open_window(SellWindow::new(client_state().sell_items(), client_state().sell_cart()));
                    self.interface.open_window(SellCartWindow::new(client_state().sell_cart()));
                }
                NetworkEvent::RefineOpened => {
                    let equips = self
                        .client_state
                        .follow(client_state().inventory().items())
                        .iter()
                        .filter_map(|item| {
                            let InventoryItemDetails::Equippable {
                                refinement_level, flags, ..
                            } = &item.details
                            else {
                                return None;
                            };
                            if !flags.contains(EquippableItemFlags::IDENTIFIED) {
                                return None;
                            }
                            Some(crate::state::refine::RefineEquip {
                                index: item.index.0,
                                item_id: item.item_id.0,
                                refine: *refinement_level,
                            })
                        })
                        .collect::<Vec<_>>();
                    self.client_state.follow_mut(client_state().refine()).open_with(&equips);
                    self.interface.open_window(RefineWindow::new(client_state().refine()));
                }
                NetworkEvent::RefineMaterials {
                    index,
                    blacksmith_blessing,
                    materials,
                } => {
                    let rows = materials
                        .into_iter()
                        .map(|material| crate::state::refine::RefineMaterialRow {
                            item_id: material.item_id.0,
                            label: crate::state::refine::material_label(material.item_id.0, material.chance_percent, material.zeny),
                        })
                        .collect();
                    self.client_state
                        .follow_mut(client_state().refine())
                        .set_materials(index.0, blacksmith_blessing, rows);
                }
                NetworkEvent::RefineResult {
                    result,
                    index,
                    refine_level,
                } => {
                    let level = u8::try_from(refine_level).unwrap_or(u8::MAX);
                    self.client_state
                        .follow_mut(client_state().inventory())
                        .set_refinement(index, level);
                    let rearm = self
                        .client_state
                        .follow_mut(client_state().refine())
                        .note_result(index.0, result, level);
                    if let Some(stored_index) = rearm {
                        let _ = self.networking_system.refine_select_item(InventoryIndex(stored_index));
                    }
                    let status = self.client_state.follow(client_state().refine()).status.clone();
                    self.client_state
                        .follow_mut(client_state().chat_messages())
                        .push(ChatMessage::new(status, MessageColor::Information));
                }
                NetworkEvent::ItemIdentifyList { indexes } => {
                    crate::interface::windows::item_skill_selection::open_identify_list(&mut self.client_state, &mut self.interface, indexes);
                }
                NetworkEvent::ItemIdentifyResult { index, result } => {
                    crate::interface::windows::item_skill_selection::on_identify_result(
                        &mut self.client_state,
                        &mut self.interface,
                        &self.library,
                        &self.async_loader,
                        index,
                        result,
                    );
                }
                NetworkEvent::ItemCompositionList { indexes } => {
                    crate::interface::windows::item_skill_selection::open_composition_list(
                        &mut self.client_state,
                        &mut self.interface,
                        indexes,
                    );
                }
                NetworkEvent::ItemCompositionResult {
                    equip_index, result, ..
                } => {
                    crate::interface::windows::item_skill_selection::on_composition_result(
                        &mut self.client_state,
                        &mut self.interface,
                        equip_index,
                        result,
                    );
                }
                NetworkEvent::WarpList { skill_id, maps } => {
                    crate::interface::windows::item_skill_selection::open_warp_list(
                        &mut self.client_state,
                        &mut self.interface,
                        &self.library,
                        skill_id,
                        maps,
                    );
                }
                NetworkEvent::AutoSpellList { skill_ids } => {
                    crate::interface::windows::item_skill_selection::open_auto_spell_list(
                        &mut self.client_state,
                        &mut self.interface,
                        &self.library,
                        &self.async_loader,
                        skill_ids,
                    );
                }
                NetworkEvent::SkillSelectList { flag, skill_ids } => {
                    crate::interface::windows::item_skill_selection::open_skill_select_list(
                        &mut self.client_state,
                        &mut self.interface,
                        &self.library,
                        &self.async_loader,
                        flag,
                        skill_ids,
                    );
                }
                NetworkEvent::MakingArrowList { item_ids } => {
                    crate::interface::windows::item_skill_selection::open_arrow_list(
                        &mut self.client_state,
                        &mut self.interface,
                        &self.library,
                        &self.async_loader,
                        item_ids,
                    );
                }
                NetworkEvent::MakableItemList { items } => {
                    crate::interface::windows::item_skill_selection::open_makable_list(
                        &mut self.client_state,
                        &mut self.interface,
                        &self.library,
                        &self.async_loader,
                        items,
                    );
                }
                NetworkEvent::ProduceResult { result, item_id } => {
                    crate::interface::windows::item_skill_selection::on_produce_result(
                        &mut self.client_state,
                        &self.library,
                        &self.async_loader,
                        result,
                        item_id,
                    );
                }
                NetworkEvent::MakingItemList { make_type, item_ids } => {
                    crate::interface::windows::item_skill_selection::open_making_list(
                        &mut self.client_state,
                        &mut self.interface,
                        &self.library,
                        &self.async_loader,
                        make_type,
                        item_ids,
                    );
                }
                NetworkEvent::WeaponRefineList { items } => {
                    crate::interface::windows::item_skill_selection::open_weapon_refine_list(
                        &mut self.client_state,
                        &mut self.interface,
                        &self.library,
                        &self.async_loader,
                        items,
                    );
                }
                NetworkEvent::WeaponRefineResult { result, item_id } => {
                    crate::interface::windows::item_skill_selection::on_weapon_refine_result(
                        &mut self.client_state,
                        &self.library,
                        &self.async_loader,
                        result,
                        item_id,
                    );
                }
                NetworkEvent::EnchantGradeOpened { data } => {
                    self.client_state.follow_mut(client_state().enchant_grade()).open(data);
                    self.interface.open_window(EnchantGradeWindow::new());
                }
                NetworkEvent::EnchantGradeMaterials {
                    index,
                    success_chance,
                    blessing,
                    materials,
                    ..
                } => {
                    let rows = materials
                        .into_iter()
                        .enumerate()
                        .map(|(row, material)| {
                            let flags = match (material.downgrade, material.breakable) {
                                (true, true) => "downgrade, break",
                                (true, false) => "downgrade",
                                (false, true) => "break",
                                (false, false) => "safe",
                            };
                            crate::state::item_workshop::GradeMaterialRow {
                                material_index: row as i32,
                                label: format!(
                                    "Option {row}: item {} x{}  {}z  {flags}",
                                    material.item_id.0, material.amount, material.price
                                ),
                            }
                        })
                        .collect();
                    self.client_state.follow_mut(client_state().enchant_grade()).set_materials(
                        index,
                        success_chance,
                        blessing.item_id,
                        rows,
                    );
                }
                NetworkEvent::EnchantGradeResult { index, grade, result } => {
                    self.client_state
                        .follow_mut(client_state().inventory())
                        .apply_enchant_grade(index, grade, result);
                    self.client_state
                        .follow_mut(client_state().enchant_grade())
                        .note_result(index.0, grade, result);
                    let status = self.client_state.follow(client_state().enchant_grade()).status.clone();
                    self.client_state
                        .follow_mut(client_state().chat_messages())
                        .push(ChatMessage::new(status, MessageColor::Information));
                }
                NetworkEvent::EnchantGradeBroadcast {
                    name,
                    item_id,
                    grade,
                    success,
                } => {
                    let outcome = if success { "succeeded" } else { "failed" };
                    self.client_state.follow_mut(client_state().chat_messages()).push(ChatMessage::new(
                        format!("{name} grade enchant {outcome} on item {} (grade {grade})", item_id.0),
                        MessageColor::Information,
                    ));
                }
                NetworkEvent::ItemReformOpened { item_id } => {
                    self.client_state.follow_mut(client_state().item_reform()).open(item_id.0);
                    self.interface.open_window(ItemReformWindow::new());
                }
                NetworkEvent::ItemReformResult { index, result } => {
                    let close = self
                        .client_state
                        .follow_mut(client_state().item_reform())
                        .note_result(index.0, result);
                    let status = self.client_state.follow(client_state().item_reform()).status.clone();
                    self.client_state
                        .follow_mut(client_state().chat_messages())
                        .push(ChatMessage::new(status, MessageColor::Information));
                    if close {
                        let _ = self.networking_system.close_item_reform();
                        self.interface.close_window_with_class(WindowClass::ItemReform);
                    }
                }
                NetworkEvent::LaphineSynthesisOpened { item_id } => {
                    self.client_state.follow_mut(client_state().laphine_synthesis()).open(item_id.0);
                    self.interface.open_window(LaphineSynthesisWindow::new());
                }
                NetworkEvent::LaphineSynthesisResult { result } => {
                    self.client_state.follow_mut(client_state().laphine_synthesis()).note_result(result);
                    let status = self.client_state.follow(client_state().laphine_synthesis()).status.clone();
                    self.client_state
                        .follow_mut(client_state().chat_messages())
                        .push(ChatMessage::new(status, MessageColor::Information));
                }
                NetworkEvent::LaphineUpgradeOpened { item_id } => {
                    self.client_state.follow_mut(client_state().laphine_upgrade()).open(item_id.0);
                    self.interface.open_window(LaphineUpgradeWindow::new());
                }
                NetworkEvent::LaphineUpgradeResult { result } => {
                    self.client_state.follow_mut(client_state().laphine_upgrade()).note_result(result);
                    let status = self.client_state.follow(client_state().laphine_upgrade()).status.clone();
                    self.client_state
                        .follow_mut(client_state().chat_messages())
                        .push(ChatMessage::new(status, MessageColor::Information));
                }
                NetworkEvent::OpenStylingShop => {
                    let library = Arc::clone(&self.library);
                    let doram = Self::styling_is_doram(&self.client_state);
                    self.client_state
                        .follow_mut(client_state().styling_shop())
                        .open_shop(doram, Some(library.as_ref()));
                    self.interface.open_window(StylingShopWindow::new());
                    Self::request_styling_preview(&mut self.client_state, &self.async_loader, &self.library);
                }
                NetworkEvent::StylingShopResult { success } => {
                    let library = Arc::clone(&self.library);
                    let doram = Self::styling_is_doram(&self.client_state);
                    self.client_state
                        .follow_mut(client_state().styling_shop())
                        .note_result(success, doram, Some(library.as_ref()));
                    let status = self.client_state.follow(client_state().styling_shop()).status.clone();
                    self.client_state
                        .follow_mut(client_state().chat_messages())
                        .push(ChatMessage::new(status, MessageColor::Information));
                    Self::request_styling_preview(&mut self.client_state, &self.async_loader, &self.library);
                }
                NetworkEvent::TradeRequested { partner_name, level, .. } => {
                    let trade = self.client_state.follow_mut(client_state().trade());
                    trade.partner_name = partner_name.clone();
                    trade.partner_level = level;
                    self.interface.open_window(TradeRequestWindow::new(partner_name, level));
                }
                NetworkEvent::TradeOpened {
                    result,
                    partner_character_id,
                    level,
                } => {
                    self.interface.close_window_with_class(WindowClass::TradeRequest);
                    if result != 3 {
                        self.client_state.follow_mut(client_state().trade()).clear();
                        let message = match result {
                            0 => "That character is too far away to trade.",
                            1 => "That character is not available to trade.",
                            4 => "Trade declined.",
                            5 => "That character is busy.",
                            _ => "Trade failed.",
                        };
                        self.client_state
                            .follow_mut(client_state().chat_messages())
                            .push(ChatMessage::new(message.to_owned(), MessageColor::Error));
                    } else {
                        let offers = self
                            .client_state
                            .follow(client_state().inventory().items())
                            .iter()
                            .filter_map(|item| {
                                let (amount, equipped) = match &item.details {
                                    korangar_networking::InventoryItemDetails::Regular {
                                        amount, equipped_position, ..
                                    } => (*amount, *equipped_position != EquipPosition::NONE),
                                    korangar_networking::InventoryItemDetails::Equippable { equipped_position, .. } => {
                                        (1, *equipped_position != EquipPosition::NONE)
                                    }
                                };
                                if equipped || amount == 0 {
                                    None
                                } else {
                                    Some(TradeInventoryOffer {
                                        index: item.index.0,
                                        label: format!("{} x{amount}", item.metadata.name),
                                        amount,
                                    })
                                }
                            })
                            .collect();

                        let trade = self.client_state.follow_mut(client_state().trade());
                        let name = std::mem::take(&mut trade.partner_name);
                        trade.clear();
                        trade.partner_name = if name.is_empty() {
                            format!("character {}", partner_character_id.0)
                        } else {
                            name
                        };
                        trade.partner_level = level;
                        trade.inventory_offers = offers;
                        trade.refresh_summaries();

                        if !self.interface.is_window_with_class_open(WindowClass::Trade) {
                            self.interface.open_window(TradeWindow::new(client_state().trade()));
                        }
                    }
                }
                NetworkEvent::TradeItemAdded {
                    item_id,
                    amount,
                    identified,
                    refinement_level,
                    ..
                } => {
                    let name = self
                        .library
                        .get::<ItemName>(ItemNameKey {
                            item_id,
                            is_identified: identified,
                        })
                        .to_string();
                    let prefix = match refinement_level {
                        0 => String::new(),
                        level => format!("+{level} "),
                    };
                    let trade = self.client_state.follow_mut(client_state().trade());
                    trade.partner_lines.push(TradeOfferLine {
                        label: format!("{prefix}{name} x{amount}"),
                    });
                    trade.refresh_summaries();
                }
                NetworkEvent::TradeZenyAdded { amount } => {
                    if amount >= 0 {
                        self.client_state.follow_mut(client_state().trade()).set_partner_zeny(amount as u32);
                    }
                }
                NetworkEvent::TradeAddResult { index, result } => {
                    if let Some(message) = self.client_state.follow_mut(client_state().trade()).apply_add_result(index, result) {
                        self.client_state
                            .follow_mut(client_state().chat_messages())
                            .push(ChatMessage::new(message.to_owned(), MessageColor::Error));
                    }
                }
                NetworkEvent::TradeConfirmed { partner } => {
                    let trade = self.client_state.follow_mut(client_state().trade());
                    if partner {
                        trade.partner_locked = true;
                    } else {
                        trade.self_locked = true;
                    }
                    trade.refresh_summaries();
                }
                NetworkEvent::TradeLockRemoved => {
                    let trade = self.client_state.follow_mut(client_state().trade());
                    trade.self_locked = false;
                    trade.partner_locked = false;
                    trade.self_committed = false;
                    trade.refresh_summaries();
                }
                NetworkEvent::TradeCancelled => {
                    self.client_state.follow_mut(client_state().trade()).clear();
                    self.interface.close_window_with_class(WindowClass::Trade);
                    self.interface.close_window_with_class(WindowClass::TradeRequest);
                    self.client_state
                        .follow_mut(client_state().chat_messages())
                        .push(ChatMessage::new("Trade cancelled.".to_owned(), MessageColor::Information));
                }
                NetworkEvent::TradeCompleted { result } => {
                    self.client_state.follow_mut(client_state().trade()).clear();
                    self.interface.close_window_with_class(WindowClass::Trade);
                    self.interface.close_window_with_class(WindowClass::TradeRequest);
                    let message = match result {
                        0 => "Trade complete.",
                        _ => "Trade failed.",
                    };
                    let color = match result {
                        0 => MessageColor::Information,
                        _ => MessageColor::Error,
                    };
                    self.client_state
                        .follow_mut(client_state().chat_messages())
                        .push(ChatMessage::new(message.to_owned(), color));
                }
                NetworkEvent::SellingCompleted { result } => match result {
                    SellItemsResult::Success => {
                        // Clear the cart.
                        self.client_state.follow_mut(client_state().buy_cart()).clear();

                        self.interface.close_window_with_class(WindowClass::Sell);
                        self.interface.close_window_with_class(WindowClass::SellCart);
                    }
                    SellItemsResult::Error => {
                        self.client_state
                            .follow_mut(client_state().chat_messages())
                            .push(ChatMessage::new("Failed to sell items".to_owned(), MessageColor::Error));
                    }
                },
                NetworkEvent::AttackFailed {
                    target_entity_id,
                    target_position,
                    player_position,
                    attack_range,
                } => {
                    if let Some(map) = &self.map
                        && self.client_state.try_follow_mut(this_entity()).is_some()
                        // Make sure that the entity is on screen.
                        && self
                            .client_state
                            .follow(client_state().entities())
                            .iter()
                            .any(|entity| entity.get_entity_id() == target_entity_id)
                        && let Some(path) =
                            self.path_finder
                                .find_walkable_path_in_range(&**map, player_position, target_position, attack_range)
                    {
                        let nearest_tile = path.last().unwrap();

                        crate::interface::npc_flow::abort_npc_progress_for_move(&mut self.client_state, &mut self.networking_system);
                        let _ = self.networking_system.player_move(WorldPosition {
                            x: nearest_tile.x,
                            y: nearest_tile.y,
                            direction: Direction::North,
                        });

                        *self.client_state.follow_mut(client_state().buffered_action()) = Some(BufferedAction::AttackEntity {
                            entity_id: target_entity_id,
                        });
                    }
                }
                NetworkEvent::UpdateSkill {
                    skill_id,
                    skill_level,
                    spell_point_cost,
                    attack_range,
                    upgradable,
                } => {
                    self.client_state.follow_mut(client_state().skill_tree()).update_skill(
                        skill_id,
                        skill_level,
                        spell_point_cost,
                        attack_range,
                        upgradable,
                    );
                    if skill_id.0 >= 10_000 {
                        self.client_state.follow_mut(client_state().guild()).apply_skill_update(
                            skill_id.0,
                            skill_level.0,
                            spell_point_cost,
                            upgradable,
                        );
                    }
                }
                NetworkEvent::RemoveSkill { skill_id } => {
                    self.client_state.follow_mut(client_state().skill_tree()).remove_skill(skill_id);
                    self.client_state
                        .follow_mut(client_state().skill_tree_window().chosen_skill_level())
                        .remove(&skill_id);
                }
                NetworkEvent::SkillCooldown { skill_id, until } => {
                    self.client_state.follow_mut(client_state().skill_cooldowns()).set(skill_id, until);
                }
                NetworkEvent::SkillCastCancelled { entity_id } => {
                    self.cast_bars.cancel(entity_id.0);
                }
                NetworkEvent::UseSkillSuccess {
                    source_entity_id,
                    skill_id,
                    delay_time,
                    ..
                } => {
                    self.cast_bars
                        .begin(source_entity_id.0, skill_id.0, delay_time, client_tick.0);
                    self.play_skill_field(skill_id, SkillField::BeginCast, source_entity_id, None, false);
                }
                NetworkEvent::ToUseSkillSuccess { flag, .. } => {
                    // `0x0110` is the failure packet. Hercules always sends `flag == 0`.
                    // Success STR is `HealEffect` when `result == 1` (`0x09CB`).
                    if flag != 0 {
                        continue;
                    }
                    let Some(entity_id) = self
                        .client_state
                        .try_follow(this_entity())
                        .map(|player| player.get_entity_id())
                    else {
                        continue;
                    };
                    self.cast_bars.cancel(entity_id.0);
                }
                NetworkEvent::NotifyGroundSkill {
                    skill_id,
                    entity_id,
                    position,
                    ..
                } => {
                    // `0x0117` is the caster's cell, not a skill unit. Play the cast
                    // STR once there. Persistent zones arrive as `AddSkillUnit`.
                    let Some(position) = self.map.as_ref().and_then(|map| map.get_world_position(position)) else {
                        continue;
                    };
                    self.play_skill_field(skill_id, SkillField::Effect, entity_id, Some(position), false);
                    self.play_skill_field(skill_id, SkillField::EffectOnCaster, entity_id, Some(position), false);
                }
                NetworkEvent::StatusChange {
                    status_index,
                    entity_id,
                    state,
                    total_duration_milliseconds,
                    remaining_milliseconds,
                    ..
                } => {
                    let is_local = self
                        .client_state
                        .try_follow(this_entity())
                        .is_some_and(|entity| entity.get_entity_id() == entity_id);
                    if is_local {
                        let statuses = self.client_state.follow_mut(client_state().status_effects());
                        match state {
                            StatusChangeState::Ended => statuses.end(status_index),
                            StatusChangeState::Active | StatusChangeState::StillActive | StatusChangeState::Other(_) => {
                                statuses.apply(status_index, remaining_milliseconds, total_duration_milliseconds, client_tick);
                            }
                        }
                    }
                }
                NetworkEvent::StatusChangeSequence {
                    status_index,
                    entity_id,
                    state,
                } => {
                    let is_local = self
                        .client_state
                        .try_follow(this_entity())
                        .is_some_and(|entity| entity.get_entity_id() == entity_id);
                    if is_local {
                        let statuses = self.client_state.follow_mut(client_state().status_effects());
                        match state {
                            StatusChangeState::Ended => statuses.end(status_index),
                            StatusChangeState::Active | StatusChangeState::StillActive | StatusChangeState::Other(_) => {
                                statuses.apply(status_index, 0, 0, client_tick);
                            }
                        }
                    }
                }
                NetworkEvent::QuestList { quests } => {
                    let library = std::sync::Arc::clone(&self.library);
                    self.client_state
                        .follow_mut(client_state().quest_log())
                        .replace_all(quests, library.as_ref());
                }
                NetworkEvent::QuestNotification {
                    quest_id,
                    active,
                    start_time,
                    expire_time,
                    objectives,
                } => {
                    let library = std::sync::Arc::clone(&self.library);
                    self.client_state.follow_mut(client_state().quest_log()).upsert_notification(
                        quest_id,
                        active,
                        start_time,
                        expire_time,
                        objectives,
                        library.as_ref(),
                    );
                }
                NetworkEvent::HuntingQuestNotification { objectives } | NetworkEvent::HuntingQuestObjectiveUpdate { objectives } => {
                    let library = std::sync::Arc::clone(&self.library);
                    self.client_state
                        .follow_mut(client_state().quest_log())
                        .apply_hunting_objectives(&objectives, library.as_ref());
                }
                NetworkEvent::HuntingQuestIndexUpdate { updates } => {
                    let quest_log = self.client_state.follow_mut(client_state().quest_log());
                    for update in updates {
                        quest_log.apply_hunt_progress_at_index(
                            update.quest_id,
                            update.objective_index,
                            update.current_count,
                            update.total_count,
                        );
                    }
                }
                NetworkEvent::QuestRemoved { quest_id } => {
                    self.client_state.follow_mut(client_state().quest_log()).remove(quest_id);
                }
                NetworkEvent::AchievementList {
                    total_score,
                    level,
                    experience,
                    experience_to_next_level,
                    entries,
                } => {
                    self.client_state.follow_mut(client_state().achievements()).replace_list(
                        total_score,
                        level,
                        experience,
                        experience_to_next_level,
                        entries,
                    );
                }
                NetworkEvent::AchievementUpdate {
                    total_score,
                    level,
                    experience,
                    experience_to_next_level,
                    entry,
                } => {
                    self.client_state.follow_mut(client_state().achievements()).upsert(
                        total_score,
                        level,
                        experience,
                        experience_to_next_level,
                        entry,
                    );
                }
                NetworkEvent::AchievementReward { failed, achievement_id } => {
                    self.client_state
                        .follow_mut(client_state().achievements())
                        .apply_reward(achievement_id, failed);
                }
                NetworkEvent::Attendance {
                    start_time,
                    end_time,
                    count,
                    rewards,
                } => {
                    self.client_state
                        .follow_mut(client_state().achievements())
                        .set_attendance(start_time, end_time, count, rewards);
                }
                NetworkEvent::AttendanceClaim { result } => {
                    self.client_state
                        .follow_mut(client_state().achievements())
                        .apply_attendance_claim(result);
                }
                NetworkEvent::NavigateTo {
                    target_type,
                    service_flags,
                    show_window,
                    map_name,
                    x,
                    y,
                    monster_id,
                } => {
                    self.client_state.follow_mut(client_state().navigation()).set_navigation(
                        map_name,
                        x,
                        y,
                        monster_id,
                        target_type,
                        service_flags,
                    );
                    if show_window
                        && self.client_state.try_follow(this_entity()).is_some()
                        && !self.interface.is_window_with_class_open(WindowClass::Navigation)
                    {
                        self.interface.open_window(NavigationWindow::new(client_state().navigation()));
                    }
                }
                NetworkEvent::MinimapMark {
                    npc_id,
                    kind,
                    x,
                    y,
                    slot,
                    red,
                    green,
                    blue,
                } => {
                    self.client_state
                        .follow_mut(client_state().navigation())
                        .apply_mark(crate::state::navigation::MinimapMarkUpdate {
                            npc_id: npc_id.0,
                            kind,
                            x,
                            y,
                            slot,
                            red,
                            green,
                            blue,
                            now: client_tick.0,
                        });
                }
            }
        }

        for effect in pincode_effects {
            self.apply_pincode_effect(effect);
        }

        if defer_headless_character
            && self.client_state.follow(client_state().pincode_window()).allows_select()
            && self.interface.is_window_with_class_open(WindowClass::CharacterSelection)
        {
            self.input_event_buffer.push(InputEvent::SelectCharacter { slot: 0 });
        }
        if let Some(occupied) = autopilot_slot_occupied
            && self.client_state.follow(client_state().pincode_window()).allows_select()
            && let Some(auto) = self.autopilot.as_mut()
        {
            autopilot_effects.extend(auto.on_characters(Instant::now(), occupied));
        }
        self.apply_autopilot(autopilot_effects);
    }

    fn apply_pincode_effect(&mut self, effect: crate::state::pincode::PincodeEffect) {
        use crate::state::pincode::PincodeOutbound;

        if effect.close_window {
            self.interface.close_window_with_class(WindowClass::Pincode);
        }
        if effect.open_window {
            self.interface.open_window(PincodeWindow);
        }
        if let Some(outbound) = effect.outbound {
            let _ = match outbound {
                PincodeOutbound::Check { account_id, pin } => self.networking_system.send_pincode_check(account_id, pin),
                PincodeOutbound::Make { account_id, pin } => self.networking_system.send_pincode_make(account_id, pin),
                PincodeOutbound::Edit {
                    account_id,
                    old_pin,
                    new_pin,
                } => self.networking_system.send_pincode_edit(account_id, old_pin, new_pin),
                PincodeOutbound::Available { account_id } => self.networking_system.send_pincode_available(account_id),
            };
        }
        if effect.disconnect {
            self.interface.close_window_with_class(WindowClass::Pincode);
            self.networking_system.disconnect_from_character_server();
        }
    }

    /// Load world-map images while the window is open.
    ///
    /// The logical path is the Hangul interface directory. A folder extract
    /// that still uses the CP949 Latin-1 directory is the local fallback.
    /// Local archives are probed first; a remote manifest entry is not a
    /// local hit. The asset host is read only after both local candidates
    /// miss. A failed read does not install the placeholder texture and
    /// does not record the path, so the next frame can retry. A confirmed
    /// miss stays empty and the window paints an opaque theme panel
    /// underneath.
    fn sync_world_map_textures(&mut self) {
        if !self.interface.is_window_with_class_open(WindowClass::WorldMap) {
            if self.client_state.follow(client_state().world_map_window()).holds_textures() {
                self.client_state.follow_mut(client_state().world_map_window()).drop_textures();
            }
            return;
        }

        let (region_path, hover_path, region_file, hover_file, loaded_region, loaded_hover) = {
            let window = self.client_state.follow(client_state().world_map_window());
            (
                window.desired_region_texture(),
                window.desired_hover_texture(),
                window.desired_region_file(),
                window.desired_hover_file(),
                window.loaded_region().to_owned(),
                window.loaded_hover().to_owned(),
            )
        };

        if region_path != loaded_region {
            let loaded = self.load_interface_texture(&region_file);
            let window = self.client_state.follow_mut(client_state().world_map_window());
            match loaded {
                LoadedInterfaceTexture::Image(texture) => window.install_region_texture(region_path, Some(texture)),
                LoadedInterfaceTexture::Retry => window.release_region_texture(),
                LoadedInterfaceTexture::Missing => window.mark_region_texture_absent(region_path),
            }
        }

        if hover_path != loaded_hover {
            if hover_file.is_empty() {
                self.client_state
                    .follow_mut(client_state().world_map_window())
                    .mark_hover_texture_absent(hover_path);
            } else {
                let loaded = self.load_interface_texture(&hover_file);
                let window = self.client_state.follow_mut(client_state().world_map_window());
                match loaded {
                    LoadedInterfaceTexture::Image(texture) => window.install_hover_texture(hover_path, Some(texture)),
                    LoadedInterfaceTexture::Retry => window.release_hover_texture(),
                    LoadedInterfaceTexture::Missing => window.mark_hover_texture_absent(hover_path),
                }
            }
        }
    }

    /// Resolve one interface texture. Local hits win, including the CP949
    /// directory, before any asset-host fetch. The image is never the
    /// placeholder texture.
    fn load_interface_texture(&self, file: &str) -> LoadedInterfaceTexture {
        match select_interface_texture(
            file,
            |archive| self.game_file_loader.local_file_exists(archive),
            |archive| self.interface_texture_readable(archive),
        ) {
            InterfaceTextureAttempt::Ready(relative) => match self.texture_loader.load_user_color(&relative) {
                Some(texture) => LoadedInterfaceTexture::Image(texture),
                None => LoadedInterfaceTexture::Retry,
            },
            InterfaceTextureAttempt::Retry => LoadedInterfaceTexture::Retry,
            InterfaceTextureAttempt::Missing => LoadedInterfaceTexture::Missing,
        }
    }

    /// `true` when the archive path reads and decodes. A manifest entry whose
    /// bytes are missing is `false` and does not install the placeholder.
    fn interface_texture_readable(&self, archive: &str) -> bool {
        let Some(relative) = archive.strip_prefix(r"data\texture\") else {
            return false;
        };
        let Ok(bytes) = korangar_loaders::FileLoader::get(self.game_file_loader.as_ref(), archive) else {
            return false;
        };
        crate::loaders::color_image_decodes(relative, &bytes)
    }

    /// A hover on the previous frame asked to show a skill. Run this after
    /// input so a right-click in the same frame toggles the skill the window
    /// was actually showing. Opens the window only when it is closed.
    fn open_pending_skill_description(&mut self) {
        let window_open = self.interface.is_window_with_class_open(WindowClass::SkillDescription);
        if !window_open {
            // Right-click and the close button both remove the window before this runs.
            // Clearing the committed id lets a later hover open it again. The slot
            // still suppresses that open while the pointer has not left the slot.
            self.client_state
                .follow_mut(client_state().skill_tree_window())
                .note_description_window_closed();
        }
        let should_open = self
            .client_state
            .follow_mut(client_state().skill_tree_window())
            .take_hover_description();
        if should_open && self.client_state.try_follow(this_entity()).is_some() && !window_open {
            self.interface
                .open_window(SkillDescriptionWindow::new(client_state().skill_tree_window()));
        }
    }

    /// Returns whether or not the interface is focused.
    #[inline(always)]
    #[cfg_attr(feature = "debug", korangar_debug::profile)]
    fn process_user_events(
        &mut self,
        input_report: &InputReport,
        client_tick: ClientTick,
        #[cfg(feature = "debug")] delta_time: f32,
    ) -> bool {
        let shop_setup_open = self.interface.is_window_with_class_open(WindowClass::VendingSetup);
        let shop_search_open = self.interface.is_window_with_class_open(WindowClass::SearchStore);
        let shop_report_open = self.interface.is_window_with_class_open(WindowClass::VendingReport);
        let selection_open = self.interface.is_window_with_class_open(WindowClass::ItemSkillSelection);
        self.interface.process_events(&mut self.input_event_buffer);
        self.on_shop_windows_dismissed(shop_setup_open, shop_search_open, shop_report_open);
        crate::interface::windows::item_skill_selection::on_window_dismissed(
            selection_open,
            &mut self.client_state,
            &mut self.networking_system,
            &mut self.interface,
        );

        let interface_has_focus = self.interface.has_focus();

        if self.interface.get_mouse_mode().is_rotating_camera() {
            // TODO: Does this really need to be a InputEvent?
            let rotation = input_report.mouse_delta.width;
            self.input_event_buffer.push(InputEvent::RotateCamera { rotation });
        }

        if !interface_has_focus {
            self.input_system.handle_keyboard_input(
                &mut self.input_event_buffer,
                #[cfg(feature = "debug")]
                self.interface.get_mouse_mode().is_default(),
                #[cfg(feature = "debug")]
                *self.client_state.follow(client_state().render_options().use_debug_camera()),
            );
        }

        let mut pincode_effects = Vec::new();
        // Own the events before matching. Shop handlers borrow `self`, which cannot
        // overlap the buffer drain.
        let input_events: Vec<_> = self.input_event_buffer.drain(..).collect();
        for event in input_events {
            match event {
                InputEvent::LogIn {
                    service_id,
                    username,
                    password,
                } => {
                    let service = self
                        .client_state
                        .follow(client_state().client_info().services())
                        .iter()
                        .find(|service| service.service_id() == service_id)
                        .unwrap();
                    let address = format!("{}:{}", service.address, service.port);
                    let socket_address = address
                        .to_socket_addrs()
                        .expect("Failed to resolve IP")
                        .next()
                        .expect("ill formatted service IP");

                    let packet_version = match service.packet_version {
                        Some(packet_version) => match packet_version {
                            PacketVersion::_20220406 => SupportedPacketVersion::_20220406,
                            PacketVersion::_20250716 => SupportedPacketVersion::_20250716,
                            PacketVersion::_20190530 => SupportedPacketVersion::_20190530,
                            PacketVersion::Unsupported(packet_version) => {
                                self.interface.open_window(ErrorWindow::new(format!(
                                    "Selected server has an unsupported package version: {packet_version}"
                                )));
                                continue;
                            }
                        },
                        None => FALLBACK_PACKET_VERSION,
                    };

                    self.saved_login_server_address = Some(socket_address);
                    self.saved_username = username.clone();
                    self.saved_password = password.clone();
                    self.saved_packet_version = packet_version;

                    self.networking_system
                        .connect_to_login_server(packet_version, socket_address, username, password);
                }
                InputEvent::SelectServer {
                    character_server_information,
                } => {
                    self.saved_character_server = Some(character_server_information.clone());

                    self.networking_system.disconnect_from_login_server();

                    // Korangar should never attempt to connect to the character
                    // server before it logged in to the login server, so it's fine to
                    // unwrap here.
                    let login_data = self.saved_login_data.as_ref().unwrap();
                    self.client_state.follow_mut(client_state().pincode_window()).reset();
                    self.networking_system
                        .connect_to_character_server(self.saved_packet_version, login_data, character_server_information);
                }
                InputEvent::Respawn => {
                    let _ = self.networking_system.respawn();
                    self.interface.close_window_with_class(WindowClass::Respawn);
                }
                InputEvent::LogOut => {
                    let _ = self.networking_system.log_out();
                }
                InputEvent::LogOutCharacter => {
                    self.client_state.follow_mut(client_state().pincode_window()).reset();
                    self.interface.close_window_with_class(WindowClass::Pincode);
                    self.networking_system.disconnect_from_character_server();
                }
                InputEvent::PincodeDigit { index } => {
                    self.client_state.follow_mut(client_state().pincode_window()).press_digit(index);
                }
                InputEvent::PincodeReset => {
                    self.client_state.follow_mut(client_state().pincode_window()).reset_entry();
                }
                InputEvent::PincodeSubmit => {
                    pincode_effects.push(self.client_state.follow_mut(client_state().pincode_window()).submit());
                }
                InputEvent::PincodeChange => {
                    pincode_effects.push(self.client_state.follow_mut(client_state().pincode_window()).start_change());
                }
                InputEvent::PincodeRequest => {
                    pincode_effects.push(self.client_state.follow_mut(client_state().pincode_window()).request_or_reopen());
                }
                InputEvent::PincodeCancel => {
                    pincode_effects.push(self.client_state.follow_mut(client_state().pincode_window()).cancel());
                }
                InputEvent::Exit => SHUTDOWN_SIGNAL.store(true, Ordering::SeqCst),
                InputEvent::ZoomCamera { zoom_factor } => self.player_camera.soft_zoom(zoom_factor),
                InputEvent::RotateCamera { rotation } => self.player_camera.soft_rotate(rotation),
                InputEvent::ResetCameraRotation => self.player_camera.reset_rotation(),
                InputEvent::ToggleMenuWindow => {
                    if self.client_state.try_follow(this_entity()).is_some() {
                        match self.interface.is_window_with_class_open(WindowClass::Menu) {
                            true => self.interface.close_window_with_class(WindowClass::Menu),
                            false => self.interface.open_window(MenuWindow),
                        }
                    }
                }
                InputEvent::ToggleWorldMap => {
                    if self.map.is_some() && self.client_state.try_follow(this_entity()).is_some() {
                        if self.interface.is_window_with_class_open(WindowClass::WorldMap) {
                            self.interface.close_window_with_class(WindowClass::WorldMap);
                            self.client_state.follow_mut(client_state().world_map_window()).close_floors();
                        } else {
                            self.client_state.follow_mut(client_state().warpra_window()).begin_list_request();
                            let _ = self.networking_system.request_warpra(WarpraWindowState::list_action(), 0, 0);
                            if !self.interface.is_window_with_class_open(WindowClass::Warpra) {
                                self.client_state.follow_mut(client_state().warpra_window()).clear_text_surface();
                            }
                            self.client_state.follow_mut(client_state().world_map_window()).prepare_open();
                            self.interface.open_window(WorldMapWindow::new());
                        }
                    }
                }
                InputEvent::ToggleInventoryWindow => {
                    if self.client_state.try_follow(this_entity()).is_some() {
                        match self.interface.is_window_with_class_open(WindowClass::Inventory) {
                            true => self.interface.close_window_with_class(WindowClass::Inventory),
                            false => self.interface.open_window(InventoryWindow::new(client_state().inventory().items())),
                        }
                    }
                }
                InputEvent::ToggleEquipmentWindow => {
                    if self.client_state.try_follow(this_entity()).is_some() {
                        match self.interface.is_window_with_class_open(WindowClass::Equipment) {
                            true => self.interface.close_window_with_class(WindowClass::Equipment),
                            false => self.interface.open_window(EquipmentWindow::new(client_state().inventory().items())),
                        }
                    }
                }
                InputEvent::ToggleCartWindow => {
                    if self.client_state.try_follow(this_entity()).is_some() {
                        match self.interface.is_window_with_class_open(WindowClass::Cart) {
                            true => self.interface.close_window_with_class(WindowClass::Cart),
                            false => self.interface.open_window(CartWindow::new(client_state().cart().items())),
                        }
                    }
                }
                InputEvent::ToggleSwitchEquipWindow => {
                    if self.client_state.try_follow(this_entity()).is_some() {
                        match self.interface.is_window_with_class_open(WindowClass::SwitchEquip) {
                            true => self.interface.close_window_with_class(WindowClass::SwitchEquip),
                            false => self
                                .interface
                                .open_window(SwitchEquipWindow::new(client_state().equip_switch().slots())),
                        }
                    }
                }
                InputEvent::ActivateEquipSwitch => {
                    let _ = self.networking_system.request_equip_switch();
                }
                InputEvent::RemoveEquipSwitchItem { index } => {
                    let _ = self.networking_system.request_equip_switch_remove(index);
                }
                InputEvent::SaveEquipLoadout => {
                    self.client_state.follow_mut(client_state().equip_switch()).save_loadout();
                }
                InputEvent::ToggleGearSets => {
                    self.client_state.follow_mut(client_state().equip_switch()).toggle_sets_menu();
                }
                InputEvent::BeginRenameLoadout => {
                    self.client_state.follow_mut(client_state().equip_switch()).begin_rename();
                }
                InputEvent::DeleteEquipLoadout => {
                    self.client_state.follow_mut(client_state().equip_switch()).delete_selected();
                }
                InputEvent::RenameEquipLoadout => {
                    self.client_state.follow_mut(client_state().equip_switch()).rename_selected();
                }
                InputEvent::CancelRenameLoadout => {
                    self.client_state.follow_mut(client_state().equip_switch()).cancel_rename();
                }
                InputEvent::ApplyEquipLoadout { index, wear } => {
                    self.client_state.follow_mut(client_state().equip_switch()).select(index);
                    let (current, desired) = {
                        let equip = self.client_state.follow(client_state().equip_switch());
                        let Some(loadout) = equip.loadout(index) else {
                            continue;
                        };
                        (equip.slots().to_vec(), loadout.slots().to_vec())
                    };
                    let delta = switch_delta(&current, &desired);
                    for index in delta.remove {
                        let _ = self.networking_system.request_equip_switch_remove(index);
                    }
                    for (index, position) in delta.add {
                        let _ = self.networking_system.request_equip_switch_add(index, position);
                    }
                    if wear {
                        let _ = self.networking_system.request_equip_switch();
                    }
                }
                InputEvent::ToggleSkillTreeWindow => {
                    if self.client_state.try_follow(this_entity()).is_some() {
                        match self.interface.is_window_with_class_open(WindowClass::SkillTree) {
                            true => self.interface.close_window_with_class(WindowClass::SkillTree),
                            false => self.interface.open_window(SkillTreeWindow::new(
                                client_state().skill_tree_window(),
                                client_state().skill_tree().layout(),
                                client_state().skill_tree().skills(),
                                this_player().manually_asserted().skill_points(),
                            )),
                        }
                    }
                }
                InputEvent::ToggleStatsWindow => {
                    if self.client_state.try_follow(this_entity()).is_some() {
                        match self.interface.is_window_with_class_open(WindowClass::Stats) {
                            true => self.interface.close_window_with_class(WindowClass::Stats),
                            false => self.interface.open_window(StatsWindow::new(this_player().manually_asserted())),
                        }
                    }
                }
                InputEvent::ToggleGameSettingsWindow => match self.interface.is_window_with_class_open(WindowClass::GameSettings) {
                    true => self.interface.close_window_with_class(WindowClass::GameSettings),
                    false => self.interface.open_window(GameSettingsWindow::new(client_state().game_settings())),
                },
                InputEvent::ToggleInterfaceSettingsWindow => match self.interface.is_window_with_class_open(WindowClass::InterfaceSettings)
                {
                    true => self.interface.close_window_with_class(WindowClass::InterfaceSettings),
                    false => self.interface.open_window(InterfaceSettingsWindow::new(
                        client_state().interface_settings(),
                        client_state().interface_settings_capabilities(),
                    )),
                },
                InputEvent::ToggleGraphicsSettingsWindow => match self.interface.is_window_with_class_open(WindowClass::GraphicsSettings) {
                    true => self.interface.close_window_with_class(WindowClass::GraphicsSettings),
                    false => self.interface.open_window(GraphicsSettingsWindow::new(
                        client_state().graphics_settings(),
                        client_state().graphics_settings_capabilities(),
                    )),
                },
                InputEvent::ToggleAudioSettingsWindow => match self.interface.is_window_with_class_open(WindowClass::AudioSettings) {
                    true => self.interface.close_window_with_class(WindowClass::AudioSettings),
                    false => self
                        .interface
                        .open_window(AudioSettingsWindow::new(client_state().audio_settings())),
                },
                InputEvent::SetQuestActive { quest_id, active } => {
                    self.client_state
                        .follow_mut(client_state().quest_log())
                        .set_active_local(quest_id, active);
                    let _ = self.networking_system.set_quest_active(quest_id, active);
                }
                InputEvent::ToggleSkillDescription { skill_id, title, body } => {
                    if self.client_state.try_follow(this_entity()).is_none() {
                        continue;
                    }
                    let same_skill = *self.client_state.follow(client_state().skill_tree_window().description_skill()) == Some(skill_id);
                    let window_open = self.interface.is_window_with_class_open(WindowClass::SkillDescription);
                    if same_skill && window_open {
                        self.interface.close_window_with_class(WindowClass::SkillDescription);
                        self.client_state.follow_mut(client_state().skill_tree_window()).cancel_hover_open();
                    } else {
                        self.client_state
                            .follow_mut(client_state().skill_tree_window())
                            .set_description(skill_id, title, body);
                        if !window_open {
                            self.interface
                                .open_window(SkillDescriptionWindow::new(client_state().skill_tree_window()));
                        }
                    }
                }
                InputEvent::ToggleQuestJournalWindow => {
                    if self.client_state.try_follow(this_entity()).is_some() {
                        match self.interface.is_window_with_class_open(WindowClass::QuestJournal) {
                            true => self.interface.close_window_with_class(WindowClass::QuestJournal),
                            false => self.interface.open_window(QuestJournalWindow::new(client_state().quest_log())),
                        }
                    }
                }
                InputEvent::ToggleAchievementsWindow => {
                    if self.client_state.try_follow(this_entity()).is_some() {
                        match self.interface.is_window_with_class_open(WindowClass::Achievements) {
                            true => self.interface.close_window_with_class(WindowClass::Achievements),
                            false => {
                                self.interface.open_window(AchievementsWindow::new(client_state().achievements()));
                            }
                        }
                    }
                }
                InputEvent::RequestAttendance => {
                    let _ = self.networking_system.request_attendance();
                }
                InputEvent::ClaimAchievementReward => {
                    if let Some(achievement_id) = self.client_state.follow(client_state().achievements()).claimable_id() {
                        let _ = self.networking_system.request_achievement_reward(achievement_id);
                    }
                }
                InputEvent::CycleAchievementFilter => {
                    self.client_state.follow_mut(client_state().achievements()).cycle_filter();
                }
                InputEvent::CycleAchievementCategory => {
                    self.client_state.follow_mut(client_state().achievements()).cycle_category();
                }
                InputEvent::ToggleNavigationWindow => {
                    if self.client_state.try_follow(this_entity()).is_some() {
                        match self.interface.is_window_with_class_open(WindowClass::Navigation) {
                            true => self.interface.close_window_with_class(WindowClass::Navigation),
                            false => self.interface.open_window(NavigationWindow::new(client_state().navigation())),
                        }
                    }
                }
                InputEvent::ToggleStatusPanelWindow => {
                    if self.client_state.try_follow(this_entity()).is_some() {
                        match self.interface.is_window_with_class_open(WindowClass::StatusPanel) {
                            true => self.interface.close_window_with_class(WindowClass::StatusPanel),
                            false => self.interface.open_window(StatusPanelWindow::new(
                                client_state().player_name(),
                                this_player().manually_asserted(),
                                client_state().status_effects(),
                            )),
                        }
                    }
                }
                InputEvent::ToggleStatusEffectsWindow => {
                    if self.client_state.try_follow(this_entity()).is_some() {
                        match self.interface.is_window_with_class_open(WindowClass::StatusEffects) {
                            true => self.interface.close_window_with_class(WindowClass::StatusEffects),
                            false => self
                                .interface
                                .open_window(StatusEffectsWindow::new(client_state().status_effects())),
                        }
                    }
                }
                InputEvent::ToggleFriendListWindow => {
                    if self.client_state.try_follow(this_entity()).is_some() {
                        match self.interface.is_window_with_class_open(WindowClass::FriendList) {
                            true => self.interface.close_window_with_class(WindowClass::FriendList),
                            false => self.interface.open_window(FriendListWindow::new(
                                client_state().friend_list_window(),
                                client_state().friend_list(),
                                client_state().party(),
                            )),
                        }
                    }
                }
                InputEvent::ToggleClanWindow => {
                    if self.client_state.try_follow(this_entity()).is_some() {
                        match self.interface.is_window_with_class_open(WindowClass::Clan) {
                            true => self.interface.close_window_with_class(WindowClass::Clan),
                            false => self.interface.open_window(ClanWindow::new(client_state().clan())),
                        }
                    }
                }
                InputEvent::ToggleItemFinderWindow => {
                    if self.client_state.try_follow(this_entity()).is_some() {
                        match self.interface.is_window_with_class_open(WindowClass::ItemFinder) {
                            true => self.interface.close_window_with_class(WindowClass::ItemFinder),
                            false => {
                                load_item_finder(self.game_file_loader.as_ref(), &mut self.client_state);
                                self.interface.open_window(ItemFinderWindow::new(client_state().item_finder()));
                            }
                        }
                    }
                }
                InputEvent::SearchItemFinder => {
                    self.client_state.follow_mut(client_state().item_finder()).search();
                }
                InputEvent::SetItemFinderCategory { category } => {
                    self.client_state.follow_mut(client_state().item_finder()).set_category(category);
                }
                InputEvent::ToggleGuildWindow => {
                    if self.client_state.try_follow(this_entity()).is_some() {
                        match self.interface.is_window_with_class_open(WindowClass::Guild) {
                            true => self.interface.close_window_with_class(WindowClass::Guild),
                            false => {
                                let _ = self.networking_system.request_guild_window();
                                self.interface
                                    .open_window(GuildWindow::new(client_state().guild_window(), client_state().guild()));
                            }
                        }
                    }
                }
                InputEvent::RequestGuildWindow => {
                    let _ = self.networking_system.request_guild_window();
                }
                InputEvent::UpdateGuildNotice { subject, notice } => {
                    let guild_id = self.client_state.follow(client_state().guild()).guild_id;
                    let is_master = self.client_state.follow(client_state().guild()).is_master;
                    if guild_id != 0 && is_master {
                        let _ = self.networking_system.update_guild_notice(guild_id, subject, notice);
                    }
                }
                InputEvent::CreateGuild { name } => {
                    let character_id = *self.client_state.follow(client_state().player_character_id());
                    let _ = self.networking_system.create_guild(character_id, name);
                }
                InputEvent::InviteToGuild { name } => {
                    let player_name = self.client_state.follow(client_state().player_name()).clone();
                    let allowed = self
                        .client_state
                        .follow(client_state().guild())
                        .can(&player_name, crate::state::guild::GPERM_INVITE);
                    if allowed {
                        let _ = self.networking_system.invite_guild_by_name(name);
                    }
                }
                InputEvent::AcceptGuildInvite { guild_id } => {
                    let _ = self.networking_system.reply_guild_invite(guild_id, 1);
                    self.interface.close_window_with_class(WindowClass::GuildInvite);
                }
                InputEvent::RejectGuildInvite { guild_id } => {
                    let _ = self.networking_system.reply_guild_invite(guild_id, 0);
                    self.interface.close_window_with_class(WindowClass::GuildInvite);
                }
                InputEvent::LeaveGuild { reason } => {
                    let guild_id = self.client_state.follow(client_state().guild()).guild_id;
                    let character_id = *self.client_state.follow(client_state().player_character_id());
                    let account_id = self
                        .client_state
                        .try_follow(this_entity())
                        .map(|player| AccountId(player.get_entity_id().0))
                        .unwrap_or(AccountId(0));
                    if guild_id != 0 && account_id.0 != 0 {
                        let _ = self.networking_system.leave_guild(guild_id, account_id, character_id, reason);
                    }
                }
                InputEvent::ExpelGuildMember {
                    account_id,
                    character_id,
                    reason,
                } => {
                    let player_name = self.client_state.follow(client_state().player_name()).clone();
                    let guild_id = self.client_state.follow(client_state().guild()).guild_id;
                    let allowed = self
                        .client_state
                        .follow(client_state().guild())
                        .can(&player_name, crate::state::guild::GPERM_EXPEL);
                    if allowed && guild_id != 0 {
                        let _ = self
                            .networking_system
                            .expel_guild_member(guild_id, account_id, character_id, reason);
                    }
                }
                InputEvent::SetGuildMemberPosition {
                    account_id,
                    character_id,
                    position_id,
                } => {
                    let is_master = self.client_state.follow(client_state().guild()).is_master;
                    if is_master && position_id >= 0 {
                        let _ = self
                            .networking_system
                            .change_guild_member_position(account_id, character_id, position_id);
                    }
                }
                InputEvent::SaveGuildPosition {
                    position_id,
                    name,
                    mode,
                    ranking,
                    pay_rate,
                } => {
                    let is_master = self.client_state.follow(client_state().guild()).is_master;
                    if is_master && position_id >= 0 && (0..=100).contains(&pay_rate) {
                        let _ = self.networking_system.change_guild_positions(vec![GuildPositionEdit {
                            position_id,
                            mode,
                            ranking,
                            pay_rate,
                            name,
                        }]);
                    }
                }
                InputEvent::LevelUpGuildSkill { skill_id } => {
                    let guild = self.client_state.follow(client_state().guild());
                    let ready = guild.skill_points > 0
                        && guild
                            .skills
                            .iter()
                            .any(|skill| skill.skill_id == skill_id && skill.upgradable);
                    if ready {
                        let _ = self.networking_system.level_up_skill(SkillId(skill_id));
                    }
                }
                InputEvent::SendGuildMessage { text } => {
                    let player_name = self.client_state.follow(client_state().player_name()).clone();
                    let _ = self.networking_system.send_guild_chat(&player_name, &text);
                }
                InputEvent::UploadGuildEmblem => {
                    match crate::state::guild_emblem::load_upload_file() {
                        Ok(bytes) => {
                            let _ = self.networking_system.upload_guild_emblem(bytes);
                            self.client_state
                                .follow_mut(client_state().guild())
                                .set_status("Emblem sent.".to_owned());
                        }
                        Err(message) => {
                            self.client_state.follow_mut(client_state().guild()).set_status(message.clone());
                            self.client_state
                                .follow_mut(client_state().chat_messages())
                                .push(ChatMessage::new(message, MessageColor::Information));
                        }
                    }
                }
                InputEvent::BreakGuildRelation { guild_id, opposition } => {
                    let is_master = self.client_state.follow(client_state().guild()).is_master;
                    if is_master {
                        let _ = self.networking_system.break_guild_relation(guild_id, opposition);
                    }
                }
                InputEvent::CloseTopWindow => self.on_close_top_window(),
                InputEvent::Escape => {
                    if self.interface.is_window_with_class_open(WindowClass::WorldMap) {
                        self.interface.close_window_with_class(WindowClass::WorldMap);
                        self.client_state.follow_mut(client_state().world_map_window()).close_floors();
                    } else {
                        match crate::input::escape_action(
                            self.interface.has_focus(),
                            self.interface.has_closable_window(&self.client_state),
                        ) {
                            crate::input::EscapeAction::Unfocus => self.interface.unfocus(),
                            crate::input::EscapeAction::CloseTop => self.on_close_top_window(),
                            crate::input::EscapeAction::ToggleMenu => {
                                if self.client_state.try_follow(this_entity()).is_some() {
                                    match self.interface.is_window_with_class_open(WindowClass::Menu) {
                                        true => self.interface.close_window_with_class(WindowClass::Menu),
                                        false => self.interface.open_window(MenuWindow),
                                    }
                                }
                            }
                        }
                    }
                }
                InputEvent::ReloadTheme => self.force_reload_themes = true,
                InputEvent::OpenItemDetails { key, title, body } => {
                    self.interface.open_or_raise_instance(key, ItemDetailsWindow { title, body });
                }
                InputEvent::ToggleShowInterface => self.show_interface = !self.show_interface,
                InputEvent::SelectCharacter { slot } => {
                    if self.client_state.follow(client_state().pincode_window()).allows_select() {
                        let _ = self.networking_system.select_character(slot);
                    }
                }
                InputEvent::OpenCharacterCreationWindow { slot } => {
                    // The creation form is a character-select dialog. Opening it after the
                    // player is already on a map paints "Create Character" over Gear.
                    if self.client_state.try_follow(this_entity()).is_some() {
                        self.interface.close_window_with_class(WindowClass::CharacterCreation);
                    } else {
                        self.client_state.follow_mut(client_state().create_character_name()).clear();
                        self.interface
                            .open_window(CharacterCreationWindow::new(client_state().create_character_name(), slot));
                    }
                }
                InputEvent::CreateCharacter { slot, name } => {
                    let _ = self.networking_system.create_character(slot, name);
                }
                InputEvent::DeleteCharacter { character_id } => {
                    if self.client_state.follow(client_state().pincode_window()).allows_select()
                        && self.client_state.follow(client_state().currently_deleting()).is_none()
                    {
                        let _ = self.networking_system.delete_character(character_id);
                        *self.client_state.follow_mut(client_state().currently_deleting()) = Some(character_id);
                    }
                }
                InputEvent::SwitchCharacterSlot {
                    origin_slot,
                    destination_slot,
                } => {
                    let _ = self.networking_system.switch_character_slot(origin_slot, destination_slot);
                }
                InputEvent::PlayerMove { destination } => {
                    crate::interface::npc_flow::abort_npc_progress_for_move(&mut self.client_state, &mut self.networking_system);

                    if self.client_state.try_follow(this_entity()).is_some() {
                        let _ = self.networking_system.player_move(WorldPosition {
                            x: destination.x,
                            y: destination.y,
                            direction: Direction::North,
                        });
                    }

                    // Unbuffer any buffered action.
                    *self.client_state.follow_mut(client_state().buffered_action()) = None;
                }
                InputEvent::PlayerInteract { entity_id } => {
                    let interaction = self
                        .client_state
                        .follow(client_state().entities())
                        .iter()
                        .find(|entity| entity.get_entity_id() == entity_id)
                        .map(|entity| (entity.get_entity_type(), entity.get_tile_position()));

                    if let Some((entity_type, tile_position)) = interaction {
                        let _ = match entity_type {
                            EntityType::Npc => self.networking_system.start_dialog(entity_id),
                            EntityType::Player => {
                                let account_id = AccountId(entity_id.0);
                                let buying_store = self
                                    .client_state
                                    .follow(client_state().player_shops())
                                    .iter()
                                    .find(|shop| shop.account_id == account_id)
                                    .map(|shop| shop.buying_store);
                                match buying_store {
                                    Some(true) => self.networking_system.request_buying_store_list(account_id),
                                    Some(false) => self.networking_system.request_vending_list(account_id),
                                    None => Ok(()),
                                }
                            }
                            EntityType::Monster => {
                                let auto_attack = *self.client_state.follow(client_state().game_settings().auto_attack());
                                let buffered_action = self.client_state.follow_mut(client_state().buffered_action());

                                if auto_attack {
                                    *buffered_action = Some(BufferedAction::AttackEntity { entity_id });
                                }

                                self.networking_system.player_attack(entity_id)
                            }
                            EntityType::Warp => {
                                crate::interface::npc_flow::abort_npc_progress_for_move(
                                    &mut self.client_state,
                                    &mut self.networking_system,
                                );
                                self.networking_system.player_move(WorldPosition {
                                    x: tile_position.x,
                                    y: tile_position.y,
                                    direction: Direction::North,
                                })
                            }
                            _ => Ok(()),
                        };
                    }
                }
                InputEvent::PickUpItem { entity_id } => {
                    self.mouse_cursor.set_state(MouseCursorState::PickUpItem, client_tick);

                    if let Some(map) = &self.map {
                        let player_position = self.client_state.try_follow(this_entity()).map(|player| player.get_tile_position());
                        let item_position = self
                            .client_state
                            .follow(client_state().ground_items())
                            .iter()
                            .find(|item| item.get_entity_id() == entity_id)
                            .map(|item| item.get_tile_position());

                        if let (Some(player_position), Some(item_position)) = (player_position, item_position) {
                            if player_position
                                .x
                                .abs_diff(item_position.x)
                                .max(player_position.y.abs_diff(item_position.y))
                                <= ITEM_PICKUP_RANGE.0
                            {
                                let _ = self.networking_system.pick_up_item(entity_id);

                                *self.client_state.follow_mut(client_state().buffered_action()) = None;
                            } else if let Some(path) =
                                self.path_finder
                                    .find_walkable_path_in_range(&**map, player_position, item_position, ITEM_PICKUP_RANGE)
                                && let Some(nearest_tile) = path.last()
                            {
                                crate::interface::npc_flow::abort_npc_progress_for_move(
                                    &mut self.client_state,
                                    &mut self.networking_system,
                                );
                                let _ = self.networking_system.player_move(WorldPosition {
                                    x: nearest_tile.x,
                                    y: nearest_tile.y,
                                    direction: Direction::North,
                                });

                                *self.client_state.follow_mut(client_state().buffered_action()) =
                                    Some(BufferedAction::PickUpItem { entity_id });
                            } else {
                                *self.client_state.follow_mut(client_state().buffered_action()) = None;
                            }
                        }
                    }
                }
                #[cfg(feature = "debug")]
                InputEvent::WarpToMap { map_name, position } => {
                    let _ = self.networking_system.warp_to_map(map_name, position);
                }
                InputEvent::SendMessage { text } => {
                    // Handle special client commands.
                    if text.as_str() == "/nc" {
                        let auto_attack = self.client_state.follow_mut(client_state().game_settings().auto_attack());
                        *auto_attack = !*auto_attack;
                        continue;
                    }

                    if text == "/finder" || text.starts_with("/finder ") {
                        if self.client_state.try_follow(this_entity()).is_some() {
                            let query = text.trim_start_matches("/finder").trim().to_owned();
                            self.client_state.follow_mut(client_state().item_finder()).query = query;
                            load_item_finder(self.game_file_loader.as_ref(), &mut self.client_state);
                            if !self.interface.is_window_with_class_open(WindowClass::ItemFinder) {
                                self.interface.open_window(ItemFinderWindow::new(client_state().item_finder()));
                            }
                        }
                        continue;
                    }

                    if let Some(message) = text.strip_prefix('%') {
                        let message = message.trim();
                        if !message.is_empty() {
                            let player_name = self.client_state.follow(client_state().player_name()).clone();
                            let _ = self.networking_system.send_party_message(&player_name, message);
                        }
                        continue;
                    }

                    match self.client_state.follow(client_state()).outgoing_chat(&text) {
                        state::chat::OutgoingChat::Whisper { target, text } => {
                            let (target, text) = self.client_state.follow_mut(client_state()).begin_whisper(target, text);
                            let _ = self.networking_system.send_whisper(&target, &text);
                        }
                        state::chat::OutgoingChat::Guild(text) => {
                            let player_name = self.client_state.follow(client_state().player_name()).clone();
                            let _ = self.networking_system.send_guild_chat(&player_name, &text);
                        }
                        state::chat::OutgoingChat::Public(text) => {
                            if let Some(entity_id) = self.client_state.try_follow(this_entity()).map(|player| player.get_entity_id()) {
                                let bubble = state::chat::speech_bubble_text(&text).to_owned();
                                self.client_state
                                    .follow_mut(client_state())
                                    .note_local_speech(entity_id, bubble, client_tick.0);
                            }
                            let _ = self
                                .networking_system
                                .send_chat_message(self.client_state.follow(client_state().player_name()), &text);
                        }
                    }
                }
                InputEvent::NextDialog { npc_id } => {
                    let _ = self.networking_system.next_dialog(npc_id);
                }
                InputEvent::CloseDialog { npc_id } => {
                    crate::interface::npc_flow::finish_npc_dialog(
                        &mut self.client_state,
                        &mut self.interface,
                        &mut self.networking_system,
                        npc_id,
                    );
                }
                InputEvent::SubmitNpcInput => {
                    crate::interface::npc_flow::reply_npc_input(
                        &mut self.client_state,
                        &mut self.interface,
                        &mut self.networking_system,
                        false,
                    );
                }
                InputEvent::CancelNpcInput => {
                    crate::interface::npc_flow::reply_npc_input(
                        &mut self.client_state,
                        &mut self.interface,
                        &mut self.networking_system,
                        true,
                    );
                }
                InputEvent::ChooseDialogOption { npc_id, option } => {
                    let _ = self.networking_system.choose_dialog_option(npc_id, option);

                    if option == -1 {
                        crate::interface::npc_flow::dismiss_npc_dialog_ui(&mut self.client_state, &mut self.interface);
                    }
                }
                InputEvent::UseItem { index } => {
                    let item = self
                        .client_state
                        .follow(client_state().inventory().items())
                        .iter()
                        .find(|item| item.index == index)
                        .cloned();
                    if let Some(item) = item {
                        match item_activation(&item) {
                            ItemActivation::Use => {
                                let _ = self.networking_system.use_item(index);
                            }
                            ItemActivation::Equip { position } => {
                                let _ = self.networking_system.request_item_equip(index, position);
                            }
                            ItemActivation::InsertCard => {
                                crate::interface::windows::item_skill_selection::begin_card_insert(
                                    &mut self.client_state,
                                    &mut self.networking_system,
                                    index,
                                );
                            }
                            ItemActivation::Ignore => {}
                        }
                    }
                }
                InputEvent::DropItem { item } => match ground_drop(&item) {
                    Some(GroundDrop::Now(amount)) => {
                        let _ = self.networking_system.drop_item(item.index, amount);
                    }
                    Some(GroundDrop::Ask { max }) => {
                        self.client_state
                            .follow_mut(client_state().item_drop())
                            .begin(item.index.0, max, &item.metadata.name);
                        if !self.interface.is_window_with_class_open(WindowClass::ItemDrop) {
                            self.interface.open_window(ItemDropWindow::new());
                        }
                        self.interface.request_focus(ItemDropAmountBox);
                    }
                    None => {}
                },
                InputEvent::ConfirmItemDrop => {
                    let (index, amount, message) = {
                        let prompt = self.client_state.follow(client_state().item_drop());
                        (prompt.index(), prompt.amount(), prompt.invalid_amount_message())
                    };
                    if let Some(amount) = amount {
                        let _ = self.networking_system.drop_item(InventoryIndex(index), amount);
                        self.interface.close_window_with_class(WindowClass::ItemDrop);
                    } else {
                        self.client_state
                            .follow_mut(client_state().chat_messages())
                            .push(ChatMessage::new(message, MessageColor::Error));
                    }
                }
                InputEvent::CancelItemDrop => {
                    self.interface.close_window_with_class(WindowClass::ItemDrop);
                }
                InputEvent::SelectItemSkillRow { index } => {
                    crate::interface::windows::item_skill_selection::on_select(&mut self.client_state, index);
                }
                InputEvent::ConfirmItemSkillSelection => {
                    crate::interface::windows::item_skill_selection::on_confirm(
                        &mut self.client_state,
                        &mut self.networking_system,
                        &mut self.interface,
                    );
                }
                InputEvent::CancelItemSkillSelection => {
                    crate::interface::windows::item_skill_selection::on_cancel(
                        &mut self.client_state,
                        &mut self.networking_system,
                        &mut self.interface,
                    );
                }
                InputEvent::MoveItem { source, destination, item } => match (source, destination) {
                    (ItemSource::Inventory, ItemSource::Equipment { position }) => {
                        let _ = self.networking_system.request_item_equip(item.index, position);
                    }
                    (ItemSource::Equipment { .. }, ItemSource::Inventory) => {
                        let _ = self.networking_system.request_item_unequip(item.index);
                    }
                    (ItemSource::Inventory, ItemSource::Storage) => {
                        let amount = match &item.details {
                            korangar_networking::InventoryItemDetails::Regular { amount, .. } => u32::from(*amount),
                            korangar_networking::InventoryItemDetails::Equippable { .. } => 1,
                        };
                        if amount > 0 {
                            let _ = self.networking_system.move_item_to_storage(item.index, amount);
                        }
                    }
                    (ItemSource::Storage, ItemSource::Inventory) => {
                        let amount = match &item.details {
                            korangar_networking::InventoryItemDetails::Regular { amount, .. } => u32::from(*amount),
                            korangar_networking::InventoryItemDetails::Equippable { .. } => 1,
                        };
                        if amount > 0 {
                            let _ = self.networking_system.move_item_from_storage(item.index, amount);
                        }
                    }
                    (ItemSource::Inventory, ItemSource::Cart) => {
                        let _ = self.networking_system.move_item_to_cart(item.index, cart_move_amount(&item));
                    }
                    (ItemSource::Cart, ItemSource::Inventory) => {
                        let _ = self.networking_system.move_item_from_cart(item.index, cart_move_amount(&item));
                    }
                    _ => {}
                },
                InputEvent::CloseStorage => {
                    if self.interface.is_window_with_class_open(WindowClass::Storage) {
                        let _ = self.networking_system.close_storage();
                        self.interface.close_window_with_class(WindowClass::Storage);
                    }
                }
                InputEvent::RequestRandomEnchant => match self.client_state.follow(client_state().enchant_window()).request() {
                    Ok((group, index, ..)) => {
                        let _ = self.networking_system.request_random_enchant(group, index);
                    }
                    Err(message) => self
                        .client_state
                        .follow_mut(client_state().enchant_window())
                        .set_status(message.to_owned()),
                },
                InputEvent::RequestPerfectEnchant => match self.client_state.follow(client_state().enchant_window()).request() {
                    Ok((group, index, material, _)) => {
                        let _ = self
                            .networking_system
                            .request_perfect_enchant(group, index, ragnarok_packets::ItemId(material));
                    }
                    Err(message) => self
                        .client_state
                        .follow_mut(client_state().enchant_window())
                        .set_status(message.to_owned()),
                },
                InputEvent::RequestUpgradeEnchant => match self.client_state.follow(client_state().enchant_window()).request() {
                    Ok((group, index, _, slot)) => {
                        let _ = self.networking_system.request_upgrade_enchant(group, index, slot);
                    }
                    Err(message) => self
                        .client_state
                        .follow_mut(client_state().enchant_window())
                        .set_status(message.to_owned()),
                },
                InputEvent::RequestResetEnchant => match self.client_state.follow(client_state().enchant_window()).request() {
                    Ok((group, index, ..)) => {
                        let _ = self.networking_system.request_reset_enchant(group, index);
                    }
                    Err(message) => self
                        .client_state
                        .follow_mut(client_state().enchant_window())
                        .set_status(message.to_owned()),
                },
                InputEvent::OpenStable => {
                    if self.client_state.try_follow(this_entity()).is_some() {
                        let _ = self.networking_system.request_stable_open();
                        self.interface.open_window(StableWindow::new());
                    }
                }
                InputEvent::RequestStableHomunculus { homunculus_id } => {
                    let _ = self.networking_system.request_stable_homunculus(homunculus_id);
                }
                InputEvent::RequestStablePet { action, id } => {
                    let _ = self.networking_system.request_stable_pet(action, id);
                }
                InputEvent::OpenEden => {
                    if self.client_state.try_follow(this_entity()).is_some() {
                        let _ = self.networking_system.request_eden(korangar_networking::EDEN_ACTION_LIST, 0);
                        self.interface.open_window(EdenWindow::new());
                    }
                }
                InputEvent::EdenAccept => match self.client_state.follow(client_state().eden_window()).quest_id() {
                    Ok(quest_id) => {
                        let _ = self
                            .networking_system
                            .request_eden(korangar_networking::EDEN_ACTION_ACCEPT, quest_id);
                    }
                    Err(message) => self
                        .client_state
                        .follow_mut(client_state().eden_window())
                        .set_status(message.to_owned()),
                },
                InputEvent::EdenComplete => match self.client_state.follow(client_state().eden_window()).quest_id() {
                    Ok(quest_id) => {
                        let _ = self
                            .networking_system
                            .request_eden(korangar_networking::EDEN_ACTION_COMPLETE, quest_id);
                    }
                    Err(message) => self
                        .client_state
                        .follow_mut(client_state().eden_window())
                        .set_status(message.to_owned()),
                },
                InputEvent::EdenAbandon => match self.client_state.follow(client_state().eden_window()).quest_id() {
                    Ok(quest_id) => {
                        let _ = self
                            .networking_system
                            .request_eden(korangar_networking::EDEN_ACTION_ABANDON, quest_id);
                    }
                    Err(message) => self
                        .client_state
                        .follow_mut(client_state().eden_window())
                        .set_status(message.to_owned()),
                },
                InputEvent::EdenBuy => match self.client_state.follow(client_state().eden_window()).shop_slot() {
                    Ok(slot) => {
                        let _ = self.networking_system.request_eden(korangar_networking::EDEN_ACTION_BUY, slot);
                    }
                    Err(message) => self
                        .client_state
                        .follow_mut(client_state().eden_window())
                        .set_status(message.to_owned()),
                },
                InputEvent::OpenWarpra | InputEvent::RequestWarpraList => {
                    if self.client_state.try_follow(this_entity()).is_some() {
                        self.client_state.follow_mut(client_state().warpra_window()).mark_text_surface();
                        self.client_state.follow_mut(client_state().warpra_window()).begin_list_request();
                        let _ = self.networking_system.request_warpra(WarpraWindowState::list_action(), 0, 0);
                        self.interface.open_window(WarpraWindow::new());
                    }
                }
                InputEvent::RequestWarpraWarp => match self.client_state.follow(client_state().warpra_window()).warp_request() {
                    Ok((kind, location_id)) => {
                        self.client_state
                            .follow_mut(client_state().warpra_window())
                            .set_last_action(WarpraWindowState::warp_action());
                        self.client_state.follow_mut(client_state().warpra_window()).mark_text_surface();
                        let _ = self
                            .networking_system
                            .request_warpra(WarpraWindowState::warp_action(), kind, location_id);
                    }
                    Err(message) => self.client_state.follow_mut(client_state().warpra_window()).set_status(message),
                },
                InputEvent::WorldMapWarp { kind, location_id } => {
                    if self.client_state.try_follow(this_entity()).is_some() {
                        self.client_state
                            .follow_mut(client_state().warpra_window())
                            .set_last_action(WarpraWindowState::warp_action());
                        self.client_state.follow_mut(client_state().warpra_window()).clear_text_surface();
                        let _ = self
                            .networking_system
                            .request_warpra(WarpraWindowState::warp_action(), kind, location_id);
                    }
                }
                InputEvent::OpenCompanions => {
                    self.interface.open_window(CompanionWindow::new());
                }
                InputEvent::PetFeed => {
                    let _ = self.networking_system.command_pet(korangar_networking::PET_ACT_FEED);
                }
                InputEvent::PetPerformance => {
                    let _ = self.networking_system.command_pet(korangar_networking::PET_ACT_PERFORMANCE);
                }
                InputEvent::PetReturn => {
                    let _ = self.networking_system.command_pet(korangar_networking::PET_ACT_RETURN);
                }
                InputEvent::PetUnequip => {
                    let _ = self.networking_system.command_pet(korangar_networking::PET_ACT_UNEQUIP);
                }
                InputEvent::HomunculusFeed => {
                    let _ = self.networking_system.command_homunculus(korangar_networking::HOM_ACT_FEED);
                }
                InputEvent::HomunculusRelease => {
                    let _ = self.networking_system.command_homunculus(korangar_networking::HOM_ACT_DELETE);
                }
                InputEvent::MercenaryDismiss => {
                    let _ = self.networking_system.command_mercenary(korangar_networking::MERC_ACT_DISMISS);
                }
                InputEvent::RequestWarpraBuy => match self.client_state.follow(client_state().warpra_window()).buy_request() {
                    Ok((kind, location_id)) => {
                        self.client_state
                            .follow_mut(client_state().warpra_window())
                            .set_last_action(WarpraWindowState::buy_action());
                        self.client_state.follow_mut(client_state().warpra_window()).mark_text_surface();
                        let _ = self
                            .networking_system
                            .request_warpra(WarpraWindowState::buy_action(), kind, location_id);
                    }
                    Err(message) => self.client_state.follow_mut(client_state().warpra_window()).set_status(message),
                },
                InputEvent::OpenMail => {
                    if self.client_state.try_follow(this_entity()).is_some() {
                        let _ = self.networking_system.open_rodex(0);
                        let _ = self.networking_system.refresh_rodex(0);
                        self.interface.open_window(MailWindow::new());
                    }
                }
                InputEvent::RefreshMail => {
                    if self.client_state.try_follow(this_entity()).is_some() {
                        let _ = self.networking_system.refresh_rodex(0);
                        self.interface.open_window(MailWindow::new());
                    }
                }
                InputEvent::NextMailPage => match self.client_state.follow(client_state().mail_window()).next_page_id() {
                    Ok(mail_id) => {
                        let _ = self.networking_system.refresh_rodex(mail_id);
                    }
                    Err(message) => self
                        .client_state
                        .follow_mut(client_state().mail_window())
                        .set_status(message.to_owned()),
                },
                InputEvent::ReadMail => match self.client_state.follow(client_state().mail_window()).mail_id() {
                    Ok(mail_id) => {
                        let _ = self.networking_system.read_rodex(0, mail_id);
                    }
                    Err(message) => self
                        .client_state
                        .follow_mut(client_state().mail_window())
                        .set_status(message.to_owned()),
                },
                InputEvent::TakeMailZeny => match self.client_state.follow(client_state().mail_window()).zeny_request() {
                    Ok(mail_id) => {
                        let _ = self.networking_system.take_rodex_zeny(0, mail_id);
                    }
                    Err(message) => self
                        .client_state
                        .follow_mut(client_state().mail_window())
                        .set_status(message.to_owned()),
                },
                InputEvent::TakeMailItems => match self.client_state.follow(client_state().mail_window()).item_request() {
                    Ok(mail_id) => {
                        let _ = self.networking_system.take_rodex_items(0, mail_id);
                    }
                    Err(message) => self
                        .client_state
                        .follow_mut(client_state().mail_window())
                        .set_status(message.to_owned()),
                },
                InputEvent::DeleteMail => match self.client_state.follow(client_state().mail_window()).mail_id() {
                    Ok(mail_id) => {
                        let _ = self.networking_system.delete_rodex(0, mail_id);
                    }
                    Err(message) => self
                        .client_state
                        .follow_mut(client_state().mail_window())
                        .set_status(message.to_owned()),
                },
                InputEvent::CloseMail => {
                    let _ = self.networking_system.close_rodex();
                    self.interface.close_window_with_class(WindowClass::Mail);
                }
                InputEvent::OpenBank => {
                    if let Some(account_id) = self.saved_login_data.as_ref().map(|login| login.account_id) {
                        let _ = self.networking_system.open_bank(account_id);
                        let _ = self.networking_system.check_bank(account_id);
                        self.interface.open_window(BankWindow::new());
                    }
                }
                InputEvent::RefreshBank => {
                    if let Some(account_id) = self.saved_login_data.as_ref().map(|login| login.account_id) {
                        let _ = self.networking_system.check_bank(account_id);
                    }
                }
                InputEvent::DepositBank => match self.client_state.follow(client_state().bank_window()).amount() {
                    Ok(zeny) => {
                        if let Some(account_id) = self.saved_login_data.as_ref().map(|login| login.account_id) {
                            let _ = self.networking_system.deposit_bank(account_id, zeny);
                        }
                    }
                    Err(message) => self.client_state.follow_mut(client_state().bank_window()).status = message.to_owned(),
                },
                InputEvent::WithdrawBank => match self.client_state.follow(client_state().bank_window()).amount() {
                    Ok(zeny) => {
                        if let Some(account_id) = self.saved_login_data.as_ref().map(|login| login.account_id) {
                            let _ = self.networking_system.withdraw_bank(account_id, zeny);
                        }
                    }
                    Err(message) => self.client_state.follow_mut(client_state().bank_window()).status = message.to_owned(),
                },
                InputEvent::BankAdd100k => self.client_state.follow_mut(client_state().bank_window()).add_amount(100_000),
                InputEvent::BankAdd1m => self.client_state.follow_mut(client_state().bank_window()).add_amount(1_000_000),
                InputEvent::BankAdd10m => self.client_state.follow_mut(client_state().bank_window()).add_amount(10_000_000),
                InputEvent::BankSetMax => {
                    if let Err(message) = self.client_state.follow_mut(client_state().bank_window()).set_max_pocket() {
                        self.client_state.follow_mut(client_state().bank_window()).status = message.to_owned();
                    }
                }
                InputEvent::CloseBank => {
                    if let Some(account_id) = self.saved_login_data.as_ref().map(|login| login.account_id) {
                        let _ = self.networking_system.close_bank(account_id);
                    }
                    self.interface.close_window_with_class(WindowClass::Bank);
                }
                InputEvent::OpenRoulette => {
                    let _ = self.networking_system.open_roulette();
                    let _ = self.networking_system.request_roulette_info();
                    self.interface.open_window(RouletteWindow::new());
                }
                InputEvent::RequestRouletteInfo => {
                    let _ = self.networking_system.request_roulette_info();
                }
                InputEvent::SpinRoulette => {
                    let _ = self.networking_system.generate_roulette();
                }
                InputEvent::ClaimRoulette => {
                    let _ = self.networking_system.claim_roulette();
                }
                InputEvent::CloseRoulette => {
                    let _ = self.networking_system.close_roulette();
                    self.interface.close_window_with_class(WindowClass::Roulette);
                }
                InputEvent::ToggleMvpMinimap => {
                    if self.map.is_some() {
                        match self.interface.is_window_with_class_open(WindowClass::MvpMinimap) {
                            true => self.interface.close_window_with_class(WindowClass::MvpMinimap),
                            false => self.interface.open_window(MvpMinimapWindow),
                        }
                    }
                }
                InputEvent::CloseEnchant => {
                    if self.interface.is_window_with_class_open(WindowClass::Enchant) {
                        let _ = self.networking_system.close_ui_enchant();
                        self.interface.close_window_with_class(WindowClass::Enchant);
                    }
                }
                InputEvent::SendEmotion { emotion } => {
                    let _ = self.networking_system.send_emotion(emotion);
                }
                InputEvent::MoveSkill {
                    source,
                    destination,
                    skill,
                } => match (source, destination) {
                    (SkillSource::SkillTree, SkillSource::Hotbar { slot }) => {
                        self.client_state
                            .follow_mut(client_state().hotbar())
                            .update_slot(&mut self.networking_system, slot, skill);
                    }
                    (SkillSource::Hotbar { slot }, SkillSource::SkillTree) => {
                        self.client_state
                            .follow_mut(client_state().hotbar())
                            .clear_slot(&mut self.networking_system, slot);
                    }
                    (SkillSource::Hotbar { slot: source_slot }, SkillSource::Hotbar { slot: destination_slot }) => {
                        self.client_state.follow_mut(client_state().hotbar()).swap_slot(
                            &mut self.networking_system,
                            source_slot,
                            destination_slot,
                        );
                    }
                    _ => {}
                },
                InputEvent::CastSkill { slot } => {
                    // Copy the cast before Change Cart opens a window. The hotbar borrow cannot
                    // overlap that mutable client-state update.
                    let cast = self
                        .client_state
                        .follow(client_state().hotbar())
                        .get_skill_in_slot(slot)
                        .as_ref()
                        .and_then(|learnable_skill| {
                            self.client_state
                                .follow(client_state().skill_tree().skills())
                                .iter()
                                .find(|learned_skill| {
                                    learned_skill.skill_id == learnable_skill.skill_id
                                        && learned_skill.skill_level.0 >= learnable_skill.maximum_level.0
                                })
                                .map(|learned_skill| (learnable_skill.skill_id, learnable_skill.maximum_level, learned_skill.skill_type))
                        });
                    if let Some((skill_id, skill_level, skill_type)) = cast {
                        if skill_id == SkillId(crate::state::item_skill_selection::CHANGE_CART_SKILL) {
                            crate::interface::windows::item_skill_selection::open_change_cart(&mut self.client_state, &mut self.interface);
                        }
                        match skill_type {
                            SkillType::Passive => {}
                            SkillType::Attack => {
                                if let PickerTarget::Entity(entity_id) = input_report.mouse_target {
                                    let _ = self.networking_system.cast_skill(skill_id, skill_level, entity_id);
                                }
                            }
                            SkillType::Ground | SkillType::Trap => {
                                if let PickerTarget::Tile { x, y } = input_report.mouse_target {
                                    let _ = self
                                        .networking_system
                                        .cast_ground_skill(skill_id, skill_level, TilePosition { x, y });
                                }
                            }
                            SkillType::SelfCast => match skill_id == ROLLING_CUTTER_ID {
                                true => {
                                    let _ = self.networking_system.cast_channeling_skill(
                                        skill_id,
                                        skill_level,
                                        self.client_state.follow(this_entity().manually_asserted()).get_entity_id(),
                                    );
                                }
                                false => {
                                    let _ = self.networking_system.cast_skill(
                                        skill_id,
                                        skill_level,
                                        self.client_state.follow(this_entity().manually_asserted()).get_entity_id(),
                                    );
                                }
                            },
                            SkillType::Support => {
                                if let PickerTarget::Entity(entity_id) = input_report.mouse_target {
                                    let _ = self.networking_system.cast_skill(skill_id, skill_level, entity_id);
                                } else {
                                    let _ = self.networking_system.cast_skill(
                                        skill_id,
                                        skill_level,
                                        self.client_state.follow(this_entity().manually_asserted()).get_entity_id(),
                                    );
                                }
                            }
                        }
                    }
                }
                InputEvent::StopSkill { slot } => {
                    if let Some(skill) = self.client_state.follow(client_state().hotbar()).get_skill_in_slot(slot).as_ref()
                        && skill.skill_id == ROLLING_CUTTER_ID
                    {
                        let _ = self.networking_system.stop_channeling_skill(skill.skill_id);
                        // `0x0B11` is outbound. The server cancel is `0x01B9`; this clears the local bar.
                        let player_id = self.client_state.try_follow(this_entity()).map(|player| player.get_entity_id().0);
                        if let Some(player_id) = player_id {
                            self.cast_bars.cancel(player_id);
                        }
                    }
                }
                InputEvent::AddFriend { character_name } => {
                    if character_name.len() > 24 {
                        #[cfg(feature = "debug")]
                        print_debug!("[{}] friend name {} is too long", "error".red(), character_name.magenta());
                    } else {
                        let _ = self.networking_system.add_friend(character_name);
                    }
                }
                InputEvent::RemoveFriend { account_id, character_id } => {
                    let _ = self.networking_system.remove_friend(account_id, character_id);
                }
                InputEvent::RejectFriendRequest { account_id, character_id } => {
                    let _ = self.networking_system.reject_friend_request(account_id, character_id);
                    self.interface.close_window_with_class(WindowClass::FriendRequest);
                }
                InputEvent::AcceptFriendRequest { account_id, character_id } => {
                    let _ = self.networking_system.accept_friend_request(account_id, character_id);
                    self.interface.close_window_with_class(WindowClass::FriendRequest);
                }
                InputEvent::RejectPartyInvite { party_id } => {
                    let _ = self.client_state.follow_mut(client_state().party()).take_pending_invite();
                    let _ = self.networking_system.reject_party_invite(party_id);
                    self.interface.close_window_with_class(WindowClass::PartyInvite);
                }
                InputEvent::AcceptPartyInvite { party_id } => {
                    let _ = self.client_state.follow_mut(client_state().party()).take_pending_invite();
                    let _ = self.networking_system.accept_party_invite(party_id);
                    self.interface.close_window_with_class(WindowClass::PartyInvite);
                }
                InputEvent::AcceptPendingPartyInvite => {
                    if let Some(party_id) = self.client_state.follow_mut(client_state().party()).take_pending_invite() {
                        let _ = self.networking_system.accept_party_invite(party_id);
                        self.interface.close_window_with_class(WindowClass::PartyInvite);
                    }
                }
                InputEvent::CreateParty {
                    name,
                    item_pickup,
                    item_division,
                } => {
                    let _ = self.networking_system.create_party(name, item_pickup, item_division);
                }
                InputEvent::LeaveParty => {
                    let _ = self.networking_system.leave_party();
                }
                InputEvent::InviteToParty { character_name } => {
                    let _ = self.networking_system.invite_to_party(character_name);
                }
                InputEvent::KickPartyMember { account_id, name } => {
                    let _ = self.networking_system.expel_party_member(account_id, name);
                }
                InputEvent::ChangePartyLeader { account_id } => {
                    let _ = self.networking_system.change_party_leader(account_id);
                }
                InputEvent::SetPartyOptions {
                    experience_share,
                    item_pickup,
                    item_division,
                } => {
                    let _ = self
                        .networking_system
                        .set_party_options(experience_share, item_pickup, item_division);
                }
                InputEvent::SetPartyInviteBlock { refuse } => {
                    let _ = self.networking_system.set_party_invite_block(refuse);
                }
                InputEvent::VendorTrade { list_index } => {
                    let buying = *self.client_state.follow(client_state().vendor_buying());
                    let account_id = *self.client_state.follow(client_state().vendor_account_id());
                    let store_id = *self.client_state.follow(client_state().vendor_shop_id());
                    if buying {
                        let item_id = self.client_state.follow(client_state().vendor_item_ids()).get(list_index).copied();
                        let inventory_index = item_id.and_then(|item_id| {
                            self.client_state
                                .follow(client_state().inventory().items())
                                .iter()
                                .find(|item| item.item_id == item_id)
                                .map(|item| item.index.0 as i16)
                        });
                        match (item_id, inventory_index) {
                            (Some(item_id), Some(inventory_index)) => {
                                let _ = self
                                    .networking_system
                                    .sell_to_buying_store(account_id, store_id, inventory_index, item_id, 1);
                            }
                            _ => {
                                self.client_state.follow_mut(client_state().chat_messages()).push(ChatMessage::new(
                                    "That item is not in your inventory.".to_owned(),
                                    MessageColor::Error,
                                ));
                            }
                        }
                    } else if let Some(index) = self.client_state.follow(client_state().vendor_indexes()).get(list_index).copied() {
                        let _ = self.networking_system.buy_from_vending(account_id, store_id, index, 1);
                    }
                }
                InputEvent::OpenPlayerShop { account_id, buying_store } => {
                    if buying_store {
                        let _ = self.networking_system.request_buying_store_list(account_id);
                    } else {
                        let _ = self.networking_system.request_vending_list(account_id);
                    }
                }
                InputEvent::LocateShop {
                    account_id,
                    store_id,
                    item_id,
                } => {
                    let _ = self.networking_system.select_search_store_item(account_id, store_id, item_id);
                }
                InputEvent::BuyItems { items } => {
                    let _ = self.networking_system.purchase_items(items);
                }
                InputEvent::CloseShop => {
                    let _ = self.networking_system.close_shop();

                    // Clear the carts.
                    self.client_state.follow_mut(client_state().buy_cart()).clear();
                    self.client_state.follow_mut(client_state().sell_cart()).clear();

                    self.interface.close_window_with_class(WindowClass::Buy);
                    self.interface.close_window_with_class(WindowClass::BuyCart);
                    self.interface.close_window_with_class(WindowClass::Sell);
                    self.interface.close_window_with_class(WindowClass::SellCart);
                }
                InputEvent::BuyOrSell { shop_id, buy_or_sell } => {
                    let _ = self.networking_system.select_buy_or_sell(shop_id, buy_or_sell);
                    self.interface.close_window_with_class(WindowClass::BuyOrSell);
                }
                InputEvent::SellItems { items } => {
                    let _ = self.networking_system.sell_items(items);
                }
                InputEvent::StatUp { stat_type } => {
                    let _ = self.networking_system.request_stat_up(stat_type);
                }
                InputEvent::DistributePointsForSkill { skill_id } => {
                    if let Some(available_skill_points) = self.client_state.try_follow(this_player().skill_points()).copied() {
                        let job_id = self.client_state.follow(this_entity().manually_asserted()).get_job_id();
                        let pending_skill_points = self
                            .client_state
                            .follow(client_state().skill_tree_window().pending_skill_points())
                            .len();
                        let available_skill_points = (available_skill_points as usize).saturating_sub(pending_skill_points);

                        let skill_information = self.library.get::<SkillListInformation>(skill_id);
                        let learned_skills = self.client_state.follow(client_state().skill_tree().skills());

                        let mut new_skill_points = self
                            .client_state
                            .follow(client_state().skill_tree_window().pending_skill_points())
                            .clone();

                        let current_skill_level = learned_skills
                            .iter()
                            .find(|skill| skill.skill_id == skill_id)
                            .map(|skill| skill.skill_level.0)
                            .unwrap_or_default()
                            + new_skill_points
                                .iter()
                                .filter(|pending_skill_level| **pending_skill_level == skill_id)
                                .count() as u16;

                        // If the skill is already at max level we don't do anything
                        if current_skill_level < skill_information.maximum_level.0 {
                            let target_skill_level = SkillLevel(current_skill_level + 1);

                            bring_skill_to_level(
                                &mut new_skill_points,
                                &self.library,
                                learned_skills,
                                job_id,
                                skill_id,
                                target_skill_level,
                                available_skill_points,
                            );

                            *self
                                .client_state
                                .follow_mut(client_state().skill_tree_window().pending_skill_points()) = new_skill_points;
                        }
                    }
                }
                InputEvent::AcceptTrade => {
                    let _ = self.networking_system.respond_to_trade(true);
                    self.interface.close_window_with_class(WindowClass::TradeRequest);
                }
                InputEvent::DeclineTrade => {
                    let _ = self.networking_system.respond_to_trade(false);
                    self.client_state.follow_mut(client_state().trade()).clear();
                    self.interface.close_window_with_class(WindowClass::TradeRequest);
                }
                InputEvent::TradeAddItem { index, amount } => {
                    let trade = self.client_state.follow(client_state().trade());
                    if trade.can_note_item_add(index, u32::from(amount)) {
                        let label = trade
                            .inventory_offers
                            .iter()
                            .find(|offer| offer.index == index)
                            .map(|offer| offer.label.clone())
                            .unwrap_or_else(|| format!("Item {index}"));
                        if self
                            .networking_system
                            .add_trade_item(Some(InventoryIndex(index)), u32::from(amount))
                            .is_ok()
                        {
                            self.client_state
                                .follow_mut(client_state().trade())
                                .note_item_add(index, label, u32::from(amount));
                        }
                    }
                }
                InputEvent::TradeAddZeny { amount } => {
                    let trade = self.client_state.follow(client_state().trade());
                    if !trade.self_locked && amount > 0 && self.networking_system.add_trade_item(None, amount).is_ok() {
                        self.client_state.follow_mut(client_state().trade()).offer_own_zeny(amount);
                    }
                }
                InputEvent::TradeConfirm => {
                    let trade = self.client_state.follow(client_state().trade());
                    if !trade.self_locked && self.networking_system.conclude_trade().is_ok() {
                        // The server echoes the lock in `TradeConfirmed`.
                    }
                }
                InputEvent::TradeDeal => {
                    let trade = self.client_state.follow(client_state().trade());
                    if trade.self_locked && trade.partner_locked && !trade.self_committed && self.networking_system.commit_trade().is_ok() {
                        self.client_state.follow_mut(client_state().trade()).self_committed = true;
                        self.client_state.follow_mut(client_state().trade()).refresh_summaries();
                    }
                }
                InputEvent::TradeCancel => {
                    let _ = self.networking_system.cancel_trade();
                    self.client_state.follow_mut(client_state().trade()).clear();
                    self.interface.close_window_with_class(WindowClass::Trade);
                    self.interface.close_window_with_class(WindowClass::TradeRequest);
                }
                InputEvent::RefineSelectItem { index } => {
                    self.client_state.follow_mut(client_state().refine()).select(index);
                    let _ = self.networking_system.refine_select_item(InventoryIndex(index));
                }
                InputEvent::RefineWith { item_id } => {
                    let index = self.client_state.follow(client_state().refine()).selected_index;
                    if let Some(index) = index {
                        let _ = self
                            .networking_system
                            .refine_item(InventoryIndex(index), ragnarok_packets::ItemId(item_id), false);
                    }
                }
                InputEvent::RefineClose => {
                    let _ = self.networking_system.close_refine();
                    self.client_state.follow_mut(client_state().refine()).close();
                    self.interface.close_window_with_class(WindowClass::Refine);
                }
                InputEvent::ItemReformSubmit => {
                    let (catalyst, index) = {
                        let reform = self.client_state.follow(client_state().item_reform());
                        (reform.catalyst_item_id, reform.index())
                    };
                    match index {
                        Ok(index) if catalyst != 0 => {
                            let _ = self
                                .networking_system
                                .item_reform(ragnarok_packets::ItemId(catalyst), InventoryIndex(index));
                        }
                        Ok(_) => {
                            self.client_state.follow_mut(client_state().item_reform()).status = "Reform is not open.".to_owned();
                        }
                        Err(message) => {
                            self.client_state.follow_mut(client_state().item_reform()).status = message.to_owned();
                        }
                    }
                }
                InputEvent::ItemReformClose => {
                    let _ = self.networking_system.close_item_reform();
                    self.interface.close_window_with_class(WindowClass::ItemReform);
                }
                InputEvent::GradeEnchantSelect => {
                    let index = self
                        .client_state
                        .follow(client_state().enchant_grade())
                        .index_text
                        .trim()
                        .parse::<u16>();
                    match index {
                        Ok(index) => {
                            let _ = self.networking_system.grade_enchant_select(InventoryIndex(index));
                        }
                        Err(_) => {
                            self.client_state.follow_mut(client_state().enchant_grade()).status = "Enter an inventory index.".to_owned();
                        }
                    }
                }
                InputEvent::GradeEnchantPick { material_index } => {
                    self.client_state.follow_mut(client_state().enchant_grade()).pick(material_index);
                }
                InputEvent::GradeEnchantSubmit => {
                    let submission = self.client_state.follow(client_state().enchant_grade()).submission();
                    match submission {
                        Ok(submission) => {
                            let _ = self.networking_system.grade_enchant_request(
                                InventoryIndex(submission.index),
                                submission.material_index,
                                submission.blessing_flag,
                                submission.blessing_amount,
                            );
                        }
                        Err(message) => {
                            self.client_state.follow_mut(client_state().enchant_grade()).status = message.to_owned();
                        }
                    }
                }
                InputEvent::GradeEnchantClose => {
                    let _ = self.networking_system.close_grade_enchant();
                    self.interface.close_window_with_class(WindowClass::EnchantGrade);
                }
                InputEvent::LaphineSynthesisAdd => {
                    if let Err(crate::state::item_workshop::SynthesisAddError::Message(message)) =
                        self.client_state.follow_mut(client_state().laphine_synthesis()).add_row()
                    {
                        self.client_state.follow_mut(client_state().laphine_synthesis()).status = message.to_owned();
                    }
                }
                InputEvent::LaphineSynthesisClear => {
                    self.client_state.follow_mut(client_state().laphine_synthesis()).clear_rows();
                }
                InputEvent::LaphineSynthesisSubmit => {
                    let (item_id, stacks) = {
                        let synthesis = self.client_state.follow(client_state().laphine_synthesis());
                        (synthesis.item_id, synthesis.stacks())
                    };
                    if item_id == 0 || stacks.is_empty() {
                        self.client_state.follow_mut(client_state().laphine_synthesis()).status = "Add at least one stack.".to_owned();
                    } else {
                        let _ = self.networking_system.laphine_synthesis(ragnarok_packets::ItemId(item_id), &stacks);
                    }
                }
                InputEvent::LaphineSynthesisClose => {
                    let _ = self.networking_system.close_laphine_synthesis();
                    self.interface.close_window_with_class(WindowClass::LaphineSynthesis);
                }
                InputEvent::LaphineUpgradeSubmit => {
                    let (item_id, index) = {
                        let upgrade = self.client_state.follow(client_state().laphine_upgrade());
                        (upgrade.item_id, upgrade.index())
                    };
                    match index {
                        Ok(index) if item_id != 0 => {
                            let _ = self
                                .networking_system
                                .laphine_upgrade(ragnarok_packets::ItemId(item_id), InventoryIndex(index));
                        }
                        Ok(_) => {
                            self.client_state.follow_mut(client_state().laphine_upgrade()).status = "Upgrade is not open.".to_owned();
                        }
                        Err(message) => {
                            self.client_state.follow_mut(client_state().laphine_upgrade()).status = message.to_owned();
                        }
                    }
                }
                InputEvent::LaphineUpgradeClose => {
                    let _ = self.networking_system.close_laphine_upgrade();
                    self.interface.close_window_with_class(WindowClass::LaphineUpgrade);
                }
                InputEvent::StylingShopTab { headgear } => {
                    let library = Arc::clone(&self.library);
                    let doram = Self::styling_is_doram(&self.client_state);
                    self.client_state
                        .follow_mut(client_state().styling_shop())
                        .set_tab(headgear, doram, Some(library.as_ref()));
                    Self::request_styling_preview(&mut self.client_state, &self.async_loader, &self.library);
                }
                InputEvent::StylingShopStep { row, delta } => {
                    let library = Arc::clone(&self.library);
                    let doram = Self::styling_is_doram(&self.client_state);
                    self.client_state
                        .follow_mut(client_state().styling_shop())
                        .step(row, delta, doram, Some(library.as_ref()));
                    Self::request_styling_preview(&mut self.client_state, &self.async_loader, &self.library);
                }
                InputEvent::StylingShopReset => {
                    let library = Arc::clone(&self.library);
                    let doram = Self::styling_is_doram(&self.client_state);
                    self.client_state
                        .follow_mut(client_state().styling_shop())
                        .reset(doram, Some(library.as_ref()));
                    Self::request_styling_preview(&mut self.client_state, &self.async_loader, &self.library);
                }
                InputEvent::StylingShopBuy => {
                    let purchase = self.client_state.follow(client_state().styling_shop()).purchase();
                    if purchase.is_empty() {
                        self.client_state.follow_mut(client_state().styling_shop()).status = "Choose a style.".to_owned();
                    } else {
                        let _ = self.networking_system.buy_styling(
                            purchase.head_palette,
                            purchase.head_style,
                            purchase.body_palette,
                            purchase.top_accessory,
                            purchase.mid_accessory,
                            purchase.bottom_accessory,
                        );
                        self.client_state.follow_mut(client_state().styling_shop()).status = "Asking the stylist...".to_owned();
                    }
                }
                InputEvent::StylingShopClose => {
                    self.client_state.follow_mut(client_state().styling_shop()).mark_button_close();
                    let _ = self.networking_system.close_styling_shop();
                    self.interface.close_window_with_class(WindowClass::StylingShop);
                }
                InputEvent::SelectShopDraft { index } => self.select_shop_draft(index),
                InputEvent::CommitShopDraft => self.commit_shop_draft(),
                InputEvent::UnlistShopDraft => self.unlist_shop_draft(),
                InputEvent::SubmitShopSetup => self.submit_shop_setup(),
                InputEvent::CancelShopSetup => self.cancel_shop_setup(),
                InputEvent::CloseOwnShop => self.close_own_shop(),
                InputEvent::SuggestShopSearch => self.suggest_shop_search(),
                InputEvent::AddShopSearchItem { index } => self.add_shop_search_item(index),
                InputEvent::RemoveShopSearchItem { index } => self.remove_shop_search_item(index),
                InputEvent::SubmitShopSearch => self.submit_shop_search(),
                InputEvent::ShopSearchNextPage => self.shop_search_next_page(),
                InputEvent::CloseShopSearch => self.close_shop_search(),
                InputEvent::SetShopSearchBuying { buying } => self.set_shop_search_buying(buying),
                InputEvent::LevelUpSkills { skill_ids } => {
                    for skill_id in skill_ids {
                        if self.networking_system.level_up_skill(skill_id).is_err() {
                            break;
                        }
                    }
                }
                InputEvent::PurchaseBarter => {
                    crate::state::barter::submit_barter_purchase(&mut self.networking_system, &mut self.client_state);
                }
                InputEvent::CloseBarter => {
                    let expanded = self.client_state.follow(client_state().barter()).is_expanded();
                    let _ = self.networking_system.close_barter(expanded);
                    self.client_state.follow_mut(client_state().barter()).mark_button_close();
                    self.interface.close_window_with_class(WindowClass::Barter);
                }
                #[cfg(feature = "debug")]
                InputEvent::ReloadLanguage => {
                    let language = *self.client_state.follow(client_state().interface_settings().language());
                    *self.client_state.follow_mut(client_state().localization()) =
                        Localization::load_language(&self.game_file_loader, language);
                }
                #[cfg(feature = "debug")]
                InputEvent::SaveLanguage => {
                    let language = *self.client_state.follow(client_state().interface_settings().language());
                    self.client_state.follow(client_state().localization()).save_language(language);
                }
                #[cfg(feature = "debug")]
                InputEvent::OpenMarkerDetails { marker_identifier } => {
                    if let Some(map) = &self.map {
                        match marker_identifier {
                            MarkerIdentifier::Object(key) => {
                                let inspecting_objects = self.client_state.follow_mut(client_state().inspecting_objects());
                                let object = map.get_object(key);
                                let object_path = state::prepare_object_inspection(inspecting_objects, object);

                                self.interface.open_state_window(object_path);
                            }
                            MarkerIdentifier::LightSource(key) => {
                                let inspecting_lights = self.client_state.follow_mut(client_state().inspecting_light_sources());
                                let light_source = map.get_light_source(key);
                                let light_source_path = state::prepare_light_source_inspection(inspecting_lights, light_source);

                                self.interface.open_state_window(light_source_path);
                            }
                            MarkerIdentifier::SoundSource(index) => {
                                let inspecting_sounds = self.client_state.follow_mut(client_state().inspecting_sound_sources());
                                let sound_source = map.get_sound_source(index);
                                let sound_source_path = state::prepare_sound_source_inspection(inspecting_sounds, sound_source);

                                self.interface.open_state_window(sound_source_path);
                            }
                            MarkerIdentifier::EffectSource(index) => {
                                let inspecting_effects = self.client_state.follow_mut(client_state().inspecting_effect_sources());
                                let effect_source = map.get_effect_source(index);
                                let effect_source_path = state::prepare_effect_source_inspection(inspecting_effects, effect_source);

                                self.interface.open_state_window(effect_source_path);
                            }
                            MarkerIdentifier::Particle(..) => {
                                // TODO:
                            }
                            MarkerIdentifier::Entity(index) => {
                                let entity_id = self
                                    .client_state
                                    .try_follow(client_state().entities().index(index as usize))
                                    .expect("entity should exist")
                                    .get_entity_id();

                                // This can technically still be `None`, violating the API but we handle this
                                // case in the state window.
                                let entity_path = client_state().entities().lookup(entity_id).manually_asserted();

                                self.interface.open_state_window(entity_path);
                            }
                            MarkerIdentifier::Shadow(..) => {
                                // TODO:
                            }
                        }
                    }
                }
                #[cfg(feature = "debug")]
                InputEvent::ToggleRenderOptionsWindow => match self.interface.is_window_with_class_open(WindowClass::RenderOptions) {
                    true => self.interface.close_window_with_class(WindowClass::RenderOptions),
                    false => self
                        .interface
                        .open_window(RenderOptionsWindow::new(client_state().render_options())),
                },
                #[cfg(feature = "debug")]
                InputEvent::OpenMapDataWindow => {
                    if let Some(map) = self.map.as_ref() {
                        let inspecting_maps = self.client_state.follow_mut(client_state().inspecting_maps());
                        let map_data_path = state::prepare_map_inspection(inspecting_maps, map.get_map_data());

                        self.interface.open_state_window(map_data_path);
                    }
                }
                #[cfg(feature = "debug")]
                InputEvent::ToggleClientStateInspectorWindow => {
                    match self.interface.is_window_with_class_open(WindowClass::ClientStateInspector) {
                        true => self.interface.close_window_with_class(WindowClass::ClientStateInspector),
                        false => self.interface.open_state_window_mut(client_state()),
                    }
                }
                #[cfg(feature = "debug")]
                InputEvent::ToggleMapsWindow => {
                    if self.map.is_some() {
                        match self.interface.is_window_with_class_open(WindowClass::Maps) {
                            true => self.interface.close_window_with_class(WindowClass::Maps),
                            false => self.interface.open_window(MapsWindow),
                        }
                    }
                }
                #[cfg(feature = "debug")]
                InputEvent::ToggleCommandsWindow => {
                    if self.map.is_some() {
                        match self.interface.is_window_with_class_open(WindowClass::Commands) {
                            true => self.interface.close_window_with_class(WindowClass::Commands),
                            false => self.interface.open_window(CommandsWindow),
                        }
                    }
                }
                #[cfg(feature = "debug")]
                InputEvent::ToggleThemeInspectorWindow => match self.interface.is_window_with_class_open(WindowClass::ThemeInspector) {
                    true => self.interface.close_window_with_class(WindowClass::ThemeInspector),
                    false => self.interface.open_window(ThemeInspectorWindow::new(
                        client_state().theme_inspector_window(),
                        client_state().menu_theme(),
                        client_state().in_game_theme(),
                        client_state().world_theme(),
                    )),
                },
                #[cfg(feature = "debug")]
                InputEvent::ToggleProfilerWindow => match self.interface.is_window_with_class_open(WindowClass::Profiler) {
                    true => self.interface.close_window_with_class(WindowClass::Profiler),
                    false => self.interface.open_window(ProfilerWindow::new(client_state().profiler_window())),
                },
                #[cfg(feature = "debug")]
                InputEvent::TogglePacketInspectorWindow => match self.interface.is_window_with_class_open(WindowClass::PacketInspector) {
                    true => self.interface.close_window_with_class(WindowClass::PacketInspector),
                    false => self
                        .interface
                        .open_window(PacketInspectorWindow::new(client_state().packet_history())),
                },
                #[cfg(feature = "debug")]
                InputEvent::ToggleCacheStatisticsWindow => match self.interface.is_window_with_class_open(WindowClass::CacheStatistics) {
                    true => self.interface.close_window_with_class(WindowClass::CacheStatistics),
                    false => self.interface.open_state_window(client_state().cache_statistics()),
                },
                #[cfg(feature = "debug")]
                InputEvent::CameraLookAround { offset } => self.debug_camera.look_around(offset),
                #[cfg(feature = "debug")]
                InputEvent::CameraMoveForward => self.debug_camera.move_forward(delta_time),
                #[cfg(feature = "debug")]
                InputEvent::CameraMoveBackward => self.debug_camera.move_backward(delta_time),
                #[cfg(feature = "debug")]
                InputEvent::CameraMoveLeft => self.debug_camera.move_left(delta_time),
                #[cfg(feature = "debug")]
                InputEvent::CameraMoveRight => self.debug_camera.move_right(delta_time),
                #[cfg(feature = "debug")]
                InputEvent::CameraMoveUp => self.debug_camera.move_up(delta_time),
                #[cfg(feature = "debug")]
                InputEvent::CameraAccelerate => self.debug_camera.accelerate(),
                #[cfg(feature = "debug")]
                InputEvent::CameraDecelerate => self.debug_camera.decelerate(),
                #[cfg(feature = "debug")]
                InputEvent::InspectFrame { measurement } => self.interface.open_window(FrameInspectorWindow::new(measurement)),
            }
        }

        self.open_pending_skill_description();

        for effect in pincode_effects {
            self.apply_pincode_effect(effect);
        }

        self.sync_styling_shop_close();
        self.sync_barter_close();

        interface_has_focus
    }

    fn styling_is_doram(state: &State<ClientState>) -> bool {
        state
            .try_follow(this_player())
            .is_some_and(|player| crate::state::styling_shop::job_is_doram(player.get_common().job_id.0))
    }

    fn apply_account_look(
        state: &mut State<ClientState>,
        async_loader: &AsyncLoader,
        library: &Library,
        account_id: AccountId,
        look: ragnarok_packets::SpriteChangeType,
        value: u32,
    ) {
        let entity = state
            .follow_mut(client_state().entities())
            .iter_mut()
            .find(|entity| entity.get_entity_id().0 == account_id.0)
            .unwrap();

        if entity.apply_player_look(look, value)
            && let Some(animation_data) = async_loader.request_animation_data_load(
                entity.get_entity_id(),
                entity.get_entity_type(),
                entity.get_entity_part_files(library),
            )
        {
            entity.set_animation_data(animation_data);
        }
    }

    /// Load a hair-only preview. The sentinel entity id is never assigned to a
    /// world entity.
    fn request_styling_preview(state: &mut State<ClientState>, async_loader: &AsyncLoader, library: &Library) {
        let Some(worn) = state.try_follow(this_player()).map(|player| player.hair_id) else {
            return;
        };
        let wanted = state.follow(client_state().styling_shop()).preview_hair_id(worn);
        if wanted == worn {
            state.follow_mut(client_state().styling_shop()).clear_preview();
            return;
        }
        state.follow_mut(client_state().styling_shop()).begin_preview(wanted as u32);
        let Some(files) = state
            .try_follow(this_player())
            .map(|player| player.preview_part_files(library, wanted))
        else {
            return;
        };
        let entity_id = ragnarok_packets::EntityId(0xF000_0000 | (wanted as u32 & 0x00FF_FFFF));
        if let Some(animation) = async_loader.request_animation_data_load(entity_id, EntityType::Player, files) {
            state
                .follow_mut(client_state().styling_shop())
                .store_preview(wanted as u32, animation);
        }
    }

    fn sync_styling_shop_close(&mut self) {
        let open = self.interface.is_window_with_class_open(WindowClass::StylingShop);
        if self
            .client_state
            .follow_mut(client_state().styling_shop())
            .take_unsolicited_close(open)
        {
            let _ = self.networking_system.close_styling_shop();
        }
    }

    /// Escape and the title-bar close remove the window without the Close
    /// button.
    fn sync_barter_close(&mut self) {
        let open = self.interface.is_window_with_class_open(WindowClass::Barter);
        if self.client_state.follow_mut(client_state().barter()).take_unsolicited_close(open) {
            let expanded = self.client_state.follow(client_state().barter()).is_expanded();
            let _ = self.networking_system.close_barter(expanded);
        }
    }

    #[cfg(feature = "debug")]
    #[inline(always)]
    #[cfg_attr(feature = "debug", korangar_debug::profile)]
    fn update_debug_windows(&mut self, delta_time: f64) {
        let is_packet_inspector_open = self.interface.is_window_with_class_open(WindowClass::PacketInspector);
        self.client_state
            .follow_mut(client_state().packet_history())
            .update(is_packet_inspector_open);

        self.client_state.follow_mut(client_state().cache_statistics()).update(
            delta_time,
            &self.map_loader,
            &self.texture_loader,
            &self.sprite_loader,
            &self.font_loader,
            &self.audio_engine,
            &self.action_loader,
            &self.animation_loader,
            &self.effect_loader,
        );
    }

    /// Drain finished async loads from the loader thread and apply their
    /// results to client state. This is the only place that promotes
    /// `self.map` from `None` to `Some` (when a map load completes), so it
    /// must run before the `self.map` check in [`Self::update_and_render`].
    ///
    /// Called as late as possible in the frame to give the loader thread the
    /// maximum window to finish in-flight work.
    #[inline(always)]
    #[cfg_attr(feature = "debug", korangar_debug::profile)]
    fn update_loaded_resources(&mut self, client_tick: ClientTick) {
        let mut autopilot_effects = Vec::new();
        for completed in self.async_loader.take_completed() {
            match completed {
                (LoaderId::AnimationData(entity_id), LoadableResource::AnimationData(animation_data)) => {
                    if let Some(entity) = self
                        .client_state
                        .follow_mut(client_state().entities())
                        .iter_mut()
                        .find(|entity| entity.get_entity_id() == entity_id)
                    {
                        entity.set_animation_data(animation_data);
                    } else if let Some(entity) = self
                        .client_state
                        .follow_mut(client_state().dead_entities())
                        .iter_mut()
                        .find(|entity| entity.get_entity_id() == entity_id)
                    {
                        entity.set_animation_data(animation_data);
                    } else if let Some(item) = self
                        .client_state
                        .follow_mut(client_state().ground_items())
                        .iter_mut()
                        .find(|item| item.get_entity_id() == entity_id)
                    {
                        item.set_animation_data(animation_data);
                    } else if entity_id.0 & 0xFF00_0000 == 0xF000_0000 {
                        let hair = entity_id.0 & 0x00FF_FFFF;
                        self.client_state
                            .follow_mut(client_state().styling_shop())
                            .store_preview(hair, animation_data);
                    }
                }
                (LoaderId::ItemSprite(item_id), LoadableResource::ItemSprite { texture }) => {
                    self.client_state
                        .follow_mut(client_state().item_skill_selection())
                        .note_item_texture(item_id.0, texture.clone());
                    self.client_state
                        .follow_mut(client_state().shop_items())
                        .iter_mut()
                        .filter(|item| item.item_id == item_id)
                        .for_each(|item| item.metadata.texture = Some(texture.clone()));

                    self.client_state
                        .follow_mut(client_state().inventory())
                        .update_item_sprite(item_id, texture.clone());
                    if let Ok(stable_item_id) = i32::try_from(item_id.0) {
                        self.client_state
                            .follow_mut(client_state().stable_window())
                            .set_item_texture(stable_item_id, texture.clone());
                    }
                    self.client_state
                        .follow_mut(client_state().barter())
                        .apply_icon(item_id, texture.clone());
                    self.client_state
                        .follow_mut(client_state().cart())
                        .update_item_sprite(item_id, texture);
                }
                (LoaderId::Map(..), LoadableResource::Map { map, position }) => {
                    match self.client_state.try_follow(this_player()).is_none() {
                        true => {
                            // Load of main menu map. Skip this when a map session is already
                            // up: that load is stale and was painting Create Character in-world.
                            let map = self.map.insert(map);

                            map.set_ambient_sound_sources(&self.audio_engine);
                            self.audio_engine.play_background_music_track(DEFAULT_BACKGROUND_MUSIC);

                            if !self.networking_system.is_map_server_connected() {
                                self.interface.open_window(CharacterSelectionWindow::new(
                                    client_state().character_slots(),
                                    client_state().switch_request(),
                                ));
                            }

                            self.start_camera.set_focus_point(START_CAMERA_FOCUS_POINT);
                            self.directional_shadow_camera.set_level_bound(map.get_level_bound());
                        }
                        false => {
                            // In-world map. A stale main-menu load must not reopen character select.
                            self.interface.close_window_with_class(WindowClass::CharacterCreation);
                            self.interface.close_window_with_class(WindowClass::CharacterSelection);
                            let map = self.map.insert(map);

                            map.set_ambient_sound_sources(&self.audio_engine);
                            let npc_track = {
                                let presentation = self.client_state.follow(client_state().npc_presentation());
                                if presentation.npc_bgm_active() {
                                    Some(presentation.npc_bgm().to_owned())
                                } else {
                                    None
                                }
                            };
                            match npc_track {
                                Some(track) => self.audio_engine.play_background_music_track(Some(&track)),
                                None => self.audio_engine.play_background_music_track(map.background_music_track_name()),
                            }

                            if let Some(position) = position {
                                // `manually_asserted` is safe because we are in the branch where `this_player`
                                // is not `None`.
                                let player = self.client_state.follow_mut(this_entity().manually_asserted());

                                player.set_position(map, position, client_tick);
                                self.player_camera.set_focus_point(player.get_position());
                            }

                            self.directional_shadow_camera.set_level_bound(map.get_level_bound());
                            let (width, height) = map.tile_size();
                            self.client_state
                                .follow_mut(client_state().mvp_minimap())
                                .set_map_size(width, height);
                            let _ = self.networking_system.map_loaded();
                            if self.autopilot.is_some() {
                                self.interface.close_window_with_class(WindowClass::CharacterOverview);
                                self.interface.close_window_with_class(WindowClass::CharacterCreation);
                                self.interface.close_window_with_class(WindowClass::CharacterSelection);
                                self.interface.close_window_with_class(WindowClass::Menu);
                                if let Some(auto) = self.autopilot.as_mut() {
                                    autopilot_effects.extend(auto.on_map_loaded(Instant::now()));
                                }
                            } else if headless_auto() && !HEADLESS_AUTO_INWORLD_UI.swap(true, Ordering::SeqCst) {
                                self.interface.close_window_with_class(WindowClass::CharacterOverview);
                                self.interface.close_window_with_class(WindowClass::CharacterCreation);
                                self.interface.close_window_with_class(WindowClass::CharacterSelection);
                                self.interface.close_window_with_class(WindowClass::Menu);
                                self.input_event_buffer.push(InputEvent::ToggleEquipmentWindow);
                            }
                        }
                    }
                }
                (LoaderId::SkillSprite(skill_id), LoadableResource::SkillSprite { sprite }) => {
                    if let Some(icon) = sprite.textures.first().cloned() {
                        self.client_state
                            .follow_mut(client_state().item_skill_selection())
                            .note_skill_icon(skill_id.0, icon);
                    }
                    self.client_state
                        .follow_mut(client_state().hotbar().skills())
                        .iter_mut()
                        .filter_map(|slot| slot.as_mut())
                        .filter(|skill| skill.skill_id == skill_id)
                        .for_each(|skill| {
                            skill.sprite = Some(sprite.clone());
                        });

                    self.client_state
                        .follow_mut(client_state().guild())
                        .attach_skill_sprite(skill_id.0, sprite.clone());

                    if let Some(skill) = self
                        .client_state
                        .follow_mut(client_state().skill_tree().layout().tabs())
                        .iter_mut()
                        .flat_map(|tab| tab.skills.values_mut())
                        .find(|skill| skill.skill_id == skill_id)
                    {
                        skill.sprite = Some(sprite);
                    }
                }
                (LoaderId::SkillActions(skill_id), LoadableResource::SkillActions { actions }) => {
                    self.client_state
                        .follow_mut(client_state().hotbar().skills())
                        .iter_mut()
                        .filter_map(|slot| slot.as_mut())
                        .filter(|skill| skill.skill_id == skill_id)
                        .for_each(|skill| {
                            skill.actions = Some(actions.clone());
                        });

                    self.client_state
                        .follow_mut(client_state().guild())
                        .attach_skill_actions(skill_id.0, actions.clone());

                    if let Some(skill) = self
                        .client_state
                        .follow_mut(client_state().skill_tree().layout().tabs())
                        .iter_mut()
                        .flat_map(|tab| tab.skills.values_mut())
                        .find(|skill| skill.skill_id == skill_id)
                    {
                        skill.actions = Some(actions);
                    }
                }
                _ => {}
            }
        }
        self.apply_autopilot(autopilot_effects);
    }

    #[inline(always)]
    #[cfg_attr(feature = "debug", korangar_debug::profile)]
    fn update_main_camera(&mut self, window_size: ScreenSize, delta_time: f64, #[cfg(feature = "debug")] render_options: &RenderOptions) {
        if self.client_state.try_follow(this_entity()).is_some() {
            self.player_camera.update(delta_time);
            self.player_camera.generate_view_projection(window_size);
        } else {
            self.start_camera.update(delta_time);
            self.start_camera.generate_view_projection(window_size);
        }

        #[cfg(feature = "debug")]
        self.interface_renderer.update_render_options(render_options);

        #[cfg(feature = "debug")]
        if render_options.use_debug_camera {
            self.debug_camera.generate_view_projection(window_size);
        }
    }

    /// Per-frame tick for all entities, dead entities, and ground items.
    ///
    /// Must be called after [`Self::handle_network_events`] so the entity
    /// set reflects the latest spawn/despawn/move packets. Running this
    /// with a stale entity list would tick entities that the network has
    /// already removed, or miss new ones that just appeared (and on a map
    /// transition, would tick entities from the previous map).
    #[inline(always)]
    #[cfg_attr(feature = "debug", korangar_debug::profile)]
    fn update_entities(
        &mut self,
        map: &Map,
        currently_playing: bool,
        client_tick: ClientTick,
        #[cfg(feature = "debug")] render_options: &RenderOptions,
    ) {
        let current_camera: &(dyn Camera + Send + Sync) = match currently_playing {
            #[cfg(feature = "debug")]
            _ if render_options.use_debug_camera => &self.debug_camera,
            true => &self.player_camera,
            false => &self.start_camera,
        };

        self.client_state
            .follow_mut(client_state().entities())
            .iter_mut()
            .for_each(|entity| entity.update(&self.audio_engine, map, current_camera, client_tick));

        // The navigation "you" pin follows the tile the player occupies now,
        // including cells crossed while a walk is still in progress.
        let player_cell = self.client_state.try_follow(this_entity()).map(|player| player.get_tile_position());
        if let Some(tile) = player_cell {
            {
                let navigation = self.client_state.follow_mut(client_state().navigation());
                let (x, y) = navigation.player_cell();
                if x != tile.x || y != tile.y {
                    navigation.set_player_cell(tile.x, tile.y);
                }
            }
            self.client_state
                .follow_mut(client_state().mvp_minimap())
                .set_player_cell(tile.x, tile.y);
        }

        self.client_state
            .follow_mut(client_state().dead_entities())
            .iter_mut()
            .for_each(|entity| {
                entity.update(&self.audio_engine, map, current_camera, client_tick);

                if entity.is_death_animation_over() && !entity.is_fading() {
                    entity.fade_out(DisappearanceReason::Died, client_tick);
                }
            });

        self.client_state
            .follow_mut(client_state().ground_items())
            .iter_mut()
            .for_each(|item| item.update(client_tick));

        // Remove entities that have finished fading out.
        self.client_state
            .follow_mut(client_state().entities())
            .retain(|entity| !entity.should_be_removed(client_tick));

        self.client_state
            .follow_mut(client_state().dead_entities())
            .retain(|entity| !entity.should_be_removed(client_tick));

        self.client_state
            .follow_mut(client_state().ground_items())
            .retain(|item| !item.should_be_removed(client_tick));
    }

    /// Fire any action that the player buffered while out of range or while
    /// still moving (attack, pick up item). Must be called after
    /// [`Self::update_entities`] so that the player's `stopped_moving` state
    /// reflects this frame.
    #[inline(always)]
    #[cfg_attr(feature = "debug", korangar_debug::profile)]
    fn process_buffered_action(&mut self) {
        let Some(true) = self.client_state.try_follow(this_entity()).map(|player| player.stopped_moving()) else {
            return;
        };

        let Some(buffered_action) = self.client_state.follow_mut(client_state().buffered_action()).take() else {
            return;
        };

        match buffered_action {
            BufferedAction::AttackEntity { entity_id } => {
                let _ = self.networking_system.player_attack(entity_id);

                let auto_attack = *self.client_state.follow(client_state().game_settings().auto_attack());
                if auto_attack {
                    *self.client_state.follow_mut(client_state().buffered_action()) = Some(BufferedAction::AttackEntity { entity_id });
                }
            }
            BufferedAction::PickUpItem { entity_id } => {
                if self
                    .client_state
                    .follow(client_state().ground_items())
                    .iter()
                    .any(|item| item.get_entity_id() == entity_id)
                {
                    let _ = self.networking_system.pick_up_item(entity_id);
                }
            }
        }
    }

    #[inline(always)]
    #[cfg_attr(feature = "debug", korangar_debug::profile)]
    fn update_audio_engine(&self, current_camera: &dyn Camera) {
        // We set the listener roughly at ear height.
        const EAR_HEIGHT: Vector3<f32> = Vector3::new(0.0, 5.0, 0.0);
        let listener = current_camera.focus_point() + EAR_HEIGHT;

        self.audio_engine
            .set_spatial_listener(listener, current_camera.view_direction(), current_camera.look_up_vector());
        self.audio_engine.update();
    }

    #[inline(always)]
    #[cfg_attr(feature = "debug", korangar_debug::profile)]
    fn create_point_light_set<'a>(
        point_light_manager: &'a mut PointLightManager,
        point_light_set_buffer: &mut ResourceSetBuffer<LightSourceKey>,
        map: &Map,
        effect_holder: &EffectHolder,
        current_camera: &dyn Camera,
        lighting_mode: LightingMode,
    ) -> PointLightSet<'a> {
        point_light_manager.prepare();

        effect_holder.register_point_lights(point_light_manager, current_camera);

        map.register_point_lights(point_light_manager, point_light_set_buffer, current_camera);

        match lighting_mode {
            LightingMode::Classic => point_light_manager.create_point_light_set(0),
            LightingMode::Enhanced => point_light_manager.create_point_light_set(NUMBER_OF_POINT_LIGHTS_WITH_SHADOWS),
        }
    }

    /// Applies any buffered drag from the previous frame's input and draws the
    /// per-frame top-level overlays (FPS counter when in debug, mouse cursor).
    /// Must be called after the laid-out interface frame has been dropped so
    /// that `self.interface` is no longer borrowed.
    #[inline(always)]
    #[cfg_attr(feature = "debug", korangar_debug::profile)]
    fn render_ui_overlays(
        &mut self,
        input_report: &InputReport,
        scaling: Scaling,
        #[cfg(feature = "debug")] render_options: &RenderOptions,
    ) {
        if let Some(delta) = input_report.drag {
            // TODO: The scaling should be removed here.
            self.interface.handle_drag(delta, scaling.get_factor());
        }

        #[cfg(feature = "debug")]
        if render_options.show_frames_per_second {
            let world_theme = self.client_state.follow(client_state().world_theme());

            self.top_interface_renderer.render_text(
                &self.game_timer.last_frames_per_second().to_string(),
                world_theme.overlay.text_offset,
                world_theme.overlay.foreground_color,
                world_theme.overlay.font_size,
                AlignHorizontal::Left,
            );
        }

        if self.show_interface {
            self.mouse_cursor.render(
                &self.top_interface_renderer,
                input_report.mouse_position,
                self.interface.get_mouse_mode().grabbed(),
                *self.client_state.follow(client_state().world_theme().cursor().color()),
                self.client_state.follow(client_state().interface_settings().scaling()).get_factor(),
            );
        }
    }

    fn headless_auto_login(&self) -> bool {
        headless_auto() && self.autopilot.is_none()
    }

    fn drive_autopilot(&mut self) {
        let effects = self.autopilot.as_mut().map(|auto| auto.poll(Instant::now())).unwrap_or_default();
        self.apply_autopilot(effects);
    }

    fn apply_autopilot(&mut self, effects: Vec<autopilot::Effect>) {
        for effect in effects {
            if let Some(line) = effect.log_line() {
                autopilot_println(&line);
                if matches!(effect, autopilot::Effect::Ready(_)) {
                    autopilot_println("AUTOPILOT_READY");
                }
            }
            match effect {
                autopilot::Effect::Login => self.autopilot_login(),
                autopilot::Effect::SelectServer(index) => {
                    if let Some(character_server_information) =
                        self.client_state.follow(client_state().character_servers()).get(index).cloned()
                    {
                        self.input_event_buffer.push(InputEvent::SelectServer {
                            character_server_information,
                        });
                    }
                }
                autopilot::Effect::SelectCharacter(slot) => {
                    self.input_event_buffer.push(InputEvent::SelectCharacter { slot });
                }
                autopilot::Effect::CreateCharacter { slot, name } => {
                    self.input_event_buffer.push(InputEvent::CreateCharacter { slot, name });
                }
                autopilot::Effect::Input(event) => self.input_event_buffer.push(event),
                autopilot::Effect::Window { class, open, event } => {
                    if self.interface.is_window_with_class_open(class) != open {
                        self.input_event_buffer.push(event);
                    }
                }
                autopilot::Effect::Click { x, y, kind } => {
                    let factor = self.client_state.follow(client_state().interface_settings().scaling()).get_factor();
                    let factor = if factor.is_finite() && factor > 0.0 { factor } else { 1.0 };
                    let button = match kind {
                        autopilot::ClickKind::Left => MouseButton::Left,
                        autopilot::ClickKind::Right => MouseButton::Right,
                        autopilot::ClickKind::Double => MouseButton::DoubleLeft,
                    };
                    self.input_system.synthesize_click(x * factor, y * factor, button);
                }
                autopilot::Effect::Key { code, alt, ctrl, shift } => self.input_system.synthesize_key(code, alt, ctrl, shift),
                autopilot::Effect::Dump(path) => {
                    let body = self.autopilot_dump(path);
                    autopilot_println(&format!("AUTOPILOT_DUMP {} {body}", path.as_str()));
                }
                autopilot::Effect::Quit => SHUTDOWN_SIGNAL.store(true, Ordering::SeqCst),
                autopilot::Effect::Log { .. } | autopilot::Effect::Ready(_) | autopilot::Effect::PinArmed => {}
            }
        }
    }

    fn autopilot_login(&mut self) {
        let Some(host) = self.autopilot.as_ref().map(|auto| auto.login_host().to_owned()) else {
            return;
        };
        let Some(port) = self.autopilot.as_ref().map(|auto| auto.login_port()) else {
            return;
        };
        let Some(username) = self.autopilot.as_ref().map(|auto| auto.login_username()) else {
            return;
        };
        let Some(password) = self.autopilot.as_ref().map(|auto| auto.password().to_owned()) else {
            return;
        };
        let service_id = self
            .client_state
            .follow(client_state().client_info().services())
            .iter()
            .find(|service| service.address == host && service.port == port)
            .map(|service| service.service_id());
        match service_id {
            Some(service_id) => self.input_event_buffer.push(InputEvent::LogIn {
                service_id,
                username,
                password,
            }),
            None => autopilot_println(&format!("AUTOPILOT_CMD err no service matches {host}:{port}")),
        }
    }

    fn autopilot_dump(&self, path: autopilot::DumpPath) -> String {
        match path {
            autopilot::DumpPath::Inventory => {
                let items = self.client_state.follow(client_state().inventory().items());
                let rows: Vec<String> = items
                    .iter()
                    .map(|item| {
                        let amount = match &item.details {
                            InventoryItemDetails::Regular { amount, .. } => u32::from(*amount),
                            InventoryItemDetails::Equippable { .. } => 1,
                        };
                        format!("(item_id: {:?}, index: {:?}, amount: {amount})", item.item_id, item.index)
                    })
                    .collect();
                format!("[{}]", rows.join(", "))
            }
            autopilot::DumpPath::Hotbar => {
                let skills = self.client_state.follow(client_state().hotbar().skills());
                let rows: Vec<String> = skills
                    .iter()
                    .enumerate()
                    .map(|(index, skill)| match skill {
                        Some(skill) => format!("{index}: {:?} {}", skill.skill_id, skill.skill_name),
                        None => format!("{index}: empty"),
                    })
                    .collect();
                format!("[{}]", rows.join(", "))
            }
            autopilot::DumpPath::ChatMessages => {
                let messages = self.client_state.follow(client_state().chat_messages());
                let tail: Vec<_> = messages.iter().rev().take(20).collect();
                let rows: Vec<String> = tail
                    .iter()
                    .rev()
                    .map(|message| format!("{:?}: {}", message.color, message.text))
                    .collect();
                format!("[{}]", rows.join(", "))
            }
            autopilot::DumpPath::Windows => format!("{:?}", self.interface.open_window_classes()),
            autopilot::DumpPath::Player => {
                let name = self.client_state.follow(client_state().player_name()).clone();
                self.client_state
                    .try_follow(this_player())
                    .map(|player| {
                        format!(
                            "(name: {name:?}, base_level: {}, job_level: {}, strength: {}, agility: {}, vitality: {}, intelligence: {}, \
                             dexterity: {}, luck: {}, spell_points: {}, stat_points: {})",
                            player.base_level,
                            player.job_level,
                            player.strength,
                            player.agility,
                            player.vitality,
                            player.intelligence,
                            player.dexterity,
                            player.luck,
                            player.spell_points,
                            player.stat_points,
                        )
                    })
                    .unwrap_or_else(|| "none".to_owned())
            }
        }
    }

    #[inline(always)]
    fn update_and_render(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            return;
        }

        self.input_system.expire_synthesized();
        self.drive_autopilot();

        if SHUTDOWN_SIGNAL.load(Ordering::SeqCst) {
            event_loop.exit();
            return;
        }

        // Clear the previous render instructions so we can rendering the new frame.
        self.clear_render_instructions();

        // It is important that we first apply any changes that were dispatched during
        // the last frame.
        self.update_client_state();

        // We can only apply the graphic changes and reconfigure the surface once the
        // previous image was presented. Moving this function to the end of the
        // function results in surface configuration errors under DX12.
        self.update_settings();

        // TODO: Shouldn't this happen later? After the scaling has been potentially
        // changed by the UI.
        let scaling = *self.client_state.follow(client_state().interface_settings().scaling());
        self.update_interface_scaling(scaling);

        let FrameTimers {
            delta_time,
            client_tick,
            animation_timer_ms,
        } = self.game_timer.update();

        let input_report = self.input_system.update_delta(client_tick);

        self.request_entity_details(&input_report);

        self.handle_network_events(client_tick);
        crate::interface::npc_flow::poll_npc_progress(&mut self.client_state, &mut self.networking_system, client_tick.0);

        let interface_has_focus = self.process_user_events(
            &input_report,
            client_tick,
            #[cfg(feature = "debug")]
            (delta_time as f32),
        );
        crate::interface::npc_flow::recover_dismissed_npc_windows(&mut self.client_state, &mut self.interface, &mut self.networking_system);

        // Some debug windows, such as the packet history or cache statistics, require
        // special update logic.
        #[cfg(feature = "debug")]
        self.update_debug_windows(delta_time);

        // We run this last to give the loader thread as much time as possible to
        // complete the loading. When starting the actual render process, we
        // can't modify resources anymore until the next frame.
        self.update_loaded_resources(client_tick);

        #[cfg(feature = "debug")]
        let render_options = *self.client_state.follow(client_state().render_options());

        let screen_size = self.graphics_engine.get_window_size().into();
        *self.client_state.follow_mut(client_state().window_size()) = screen_size;
        let currently_playing = self.client_state.try_follow(this_player()).is_some();

        self.mouse_cursor.update(client_tick);

        // Acquire the swapchain image as late as possible so all CPU-side
        // preparation overlaps with the previous frame's GPU work. After this
        // point we may not reconfigure the surface (see `update_settings`).
        let maybe_frame = self.graphics_engine.wait_for_next_frame();

        // If we don't have a map, the rendering ends here.
        let Some(map) = self.map.clone() else {
            if let Some(frame) = maybe_frame {
                self.graphics_engine.render_next_frame(frame, Default::default());
            }

            return;
        };

        self.update_entities(
            &map,
            currently_playing,
            client_tick,
            #[cfg(feature = "debug")]
            &render_options,
        );

        self.process_buffered_action();

        self.update_main_camera(
            screen_size,
            delta_time,
            #[cfg(feature = "debug")]
            &render_options,
        );

        map.advance_videos(&self.queue, delta_time);

        if let Some(player) = self.client_state.try_follow(this_entity()) {
            self.player_camera.set_smoothed_focus_point(player.get_position());
        }

        // Update particles.
        self.particle_holder.update(delta_time as f32);
        self.effect_holder
            .update(self.client_state.follow(client_state().entities()), delta_time as f32);

        self.sync_world_map_textures();

        let current_camera: &(dyn Camera + Send + Sync) = match currently_playing {
            #[cfg(feature = "debug")]
            _ if render_options.use_debug_camera => &self.debug_camera,
            true => &self.player_camera,
            false => &self.start_camera,
        };

        let (view_matrix, projection_matrix) = current_camera.view_projection_matrices();
        let camera_position = current_camera.camera_position().to_homogeneous();

        #[cfg(feature = "debug")]
        let update_shadow_camera_measurement = Profiler::start_measurement("update directional shadow camera");

        let lighting_mode = *self.client_state.follow(client_state().graphics_settings().lighting_mode());
        let shadow_resolution = *self.client_state.follow(client_state().graphics_settings().shadow_resolution());
        let shadow_method = *self.client_state.follow(client_state().graphics_settings().shadow_method());
        let shadow_detail = *self.client_state.follow(client_state().graphics_settings().shadow_detail());
        let sdsm_enabled = *self.client_state.follow(client_state().graphics_settings().sdsm());

        let use_sdsm = sdsm_enabled & !self.player_camera.is_rotating_or_zooming_fast();

        let (directional_light_direction, directional_light_color) = map.directional_light();

        match use_sdsm {
            true => {
                self.directional_shadow_camera.update_camera_sdsm(
                    directional_light_direction,
                    &view_matrix,
                    &projection_matrix,
                    shadow_resolution.directional_shadow_resolution(),
                    self.directional_shadow_partitions.lock().unwrap().deref(),
                );
            }
            false => {
                self.directional_shadow_camera.update_camera_pssm(
                    directional_light_direction,
                    &view_matrix,
                    &projection_matrix,
                    shadow_resolution.directional_shadow_resolution(),
                );
            }
        }

        #[cfg(feature = "debug")]
        update_shadow_camera_measurement.stop();

        self.update_audio_engine(current_camera);

        #[cfg(feature = "debug")]
        let prepare_frame_measurement = Profiler::start_measurement("prepare frame");

        #[cfg(feature = "debug")]
        let hovered_marker_identifier = match input_report.mouse_target {
            PickerTarget::Marker(marker_identifier) => Some(marker_identifier),
            _ => None,
        };

        let point_light_set = Self::create_point_light_set(
            &mut self.point_light_manager,
            &mut self.point_light_set_buffer,
            &map,
            &self.effect_holder,
            current_camera,
            lighting_mode,
        );

        #[cfg(feature = "debug")]
        prepare_frame_measurement.stop();

        let mut indicator_instruction = None;
        let mut water_instruction = None;

        let mouse_mode = self.interface.get_mouse_mode();
        let is_mouse_mode_default = mouse_mode.is_default();
        let last_walking_destination = mouse_mode.walk_destination();
        let dragged_inventory_item = match mouse_mode {
            korangar_interface::MouseMode::Custom {
                mode:
                    MouseInputMode::MoveItem {
                        source: ItemSource::Inventory,
                        item,
                    },
            } => Some(item.clone()),
            _ => None,
        };

        let focus_npc_input = self.client_state.follow_mut(client_state().dialog_window()).take_input_focus();
        let dialog_npc_id = self.client_state.follow(client_state().dialog_window()).npc_id();
        let dialog_key = if !interface_has_focus {
            let dialog = self.client_state.follow(client_state().dialog_window());
            dialog_hotkey(
                dialog.is_awaiting_next(),
                dialog.is_awaiting_close(),
                dialog.is_awaiting_choice(),
                dialog.input_is_open(),
                &input_report.characters,
            )
        } else {
            None
        };

        let mut interface_frame = {
            #[cfg(feature = "debug")]
            profile_block!("user interface");

            let is_rotating_camera = mouse_mode.is_rotating_camera();
            let is_grabbing = mouse_mode.is_grabbing();
            let is_chat_open = self.interface.is_window_with_class_open(WindowClass::Chat);

            let mut interface_frame = self
                .interface
                .lay_out_windows(&self.client_state, scaling.get_factor(), input_report.mouse_position);

            // We can only decide what to do with the user input once we know if the mouse
            // is hovering a window, so we buffer any actions for the next frame.

            let is_interface_hovered = interface_frame.is_interface_hovered();
            let shop_hit = if is_interface_hovered {
                None
            } else {
                crate::interface::shop_boards::shop_board_hit(current_camera, &self.client_state, screen_size, input_report.mouse_position)
            };

            let cursor_state = match input_report.mouse_target {
                _ if is_rotating_camera => MouseCursorState::RotateCamera,
                _ if is_grabbing => MouseCursorState::GrabResource,
                _ if shop_hit.is_some() => MouseCursorState::Dialog,
                PickerTarget::Entity(entity_id) if !is_interface_hovered => {
                    if self
                        .client_state
                        .follow(client_state().ground_items())
                        .iter()
                        .any(|item| item.get_entity_id() == entity_id)
                    {
                        MouseCursorState::HoverItem
                    } else {
                        self.client_state
                            .follow(client_state().entities())
                            .iter()
                            .find(|entity| entity.get_entity_id() == entity_id)
                            .map(|entity| match entity.get_entity_type() {
                                EntityType::Npc => MouseCursorState::Dialog,
                                EntityType::Warp => MouseCursorState::Warp,
                                EntityType::Monster => MouseCursorState::Attack,
                                _ => MouseCursorState::Default,
                            })
                            .unwrap_or(MouseCursorState::Default)
                    }
                }
                _ => MouseCursorState::Default,
            };
            self.mouse_cursor.set_state(cursor_state, client_tick);

            if let Some(mouse_button) = input_report.mouse_click {
                if is_interface_hovered {
                    interface_frame.click(&self.client_state, mouse_button);
                } else {
                    interface_frame.unfocus();

                    if mouse_button == MouseButton::Left {
                        if let Some(hit) = shop_hit {
                            self.input_event_buffer.push(InputEvent::OpenPlayerShop {
                                account_id: hit.account_id,
                                buying_store: hit.buying_store,
                            });
                        } else {
                            match input_report.mouse_target {
                                PickerTarget::Nothing => {}
                                PickerTarget::Entity(entity_id) => {
                                    let is_ground_item = self
                                        .client_state
                                        .follow(client_state().ground_items())
                                        .iter()
                                        .any(|item| item.get_entity_id() == entity_id);

                                    if is_ground_item {
                                        self.input_event_buffer.push(InputEvent::PickUpItem { entity_id })
                                    } else {
                                        self.input_event_buffer.push(InputEvent::PlayerInteract { entity_id })
                                    }
                                }
                                PickerTarget::Tile { x, y } => {
                                    let destination = TilePosition { x, y };

                                    interface_frame.set_mouse_mode(MouseInputMode::Walk { destination });

                                    self.input_event_buffer.push(InputEvent::PlayerMove { destination });
                                }
                                #[cfg(feature = "debug")]
                                PickerTarget::Marker(marker_identifier) => {
                                    self.input_event_buffer.push(InputEvent::OpenMarkerDetails { marker_identifier })
                                }
                            }
                        }
                    } else if mouse_button == MouseButton::Right && currently_playing {
                        #[cfg_attr(feature = "debug", korangar_debug::debug_condition(!render_options.use_debug_camera))]
                        interface_frame.set_mouse_mode(MouseInputMode::RotateCamera);
                    } else if mouse_button == MouseButton::DoubleRight && currently_playing {
                        #[cfg_attr(feature = "debug", korangar_debug::debug_condition(!render_options.use_debug_camera))]
                        self.input_event_buffer.push(InputEvent::ResetCameraRotation);
                    }
                }
            } else if let Some(last_destination) = last_walking_destination
                && let PickerTarget::Tile { x, y } = input_report.mouse_target
                && input_report.left_mouse_button_down
            {
                let destination = TilePosition { x, y };

                if last_destination != destination {
                    interface_frame.set_mouse_mode(MouseInputMode::Walk { destination });
                    self.input_event_buffer.push(InputEvent::PlayerMove { destination });
                }
            }

            if input_report.mouse_button_released {
                if !is_interface_hovered && let Some(item) = dragged_inventory_item {
                    self.input_event_buffer.push(InputEvent::DropItem { item });
                }
                interface_frame.drop(&self.client_state);
            }

            if let Some(delta) = input_report.scroll {
                if is_interface_hovered {
                    interface_frame.scroll(&self.client_state, delta);
                } else {
                    #[cfg_attr(feature = "debug", korangar_debug::debug_condition(!render_options.use_debug_camera))]
                    self.input_event_buffer.push(InputEvent::ZoomCamera { zoom_factor: delta });
                }
            }

            // Focus the NPC input once when it opens, then let Enter submit that box.
            // Next and Close take Enter and Space before chat does.
            if focus_npc_input {
                interface_frame.focus_element(NpcInputBox);
            }

            if let Some(dialog_key) = dialog_key {
                let event = match dialog_key {
                    DialogHotkey::Next => InputEvent::NextDialog { npc_id: dialog_npc_id },
                    DialogHotkey::Close => InputEvent::CloseDialog { npc_id: dialog_npc_id },
                };
                self.input_event_buffer.push(event);
            } else if (!interface_has_focus || !interface_frame.input_characters(&self.client_state, &input_report.characters))
                && input_report.characters.contains(&'\x0d')
                && is_chat_open
            {
                interface_frame.focus_element(ChatTextBox);
            }

            interface_frame
        };

        self.cast_bars.expire(client_tick.0);

        {
            let mut render_context = MapRenderContext {
                map: &map,
                current_camera,
                point_light_set: &point_light_set,
                client_state: &self.client_state,
                library: &self.library,
                mouse_position: input_report.mouse_position,
                mouse_target: input_report.mouse_target,
                screen_size,
                scaling,
                client_tick,
                animation_timer_ms,
                currently_playing,
                is_mouse_mode_default,
                is_interface_hovered: interface_frame.is_interface_hovered(),
                last_walking_destination,
                buffered_action: *self.client_state.follow(client_state().buffered_action()),
                #[cfg(feature = "debug")]
                render_options: &render_options,
                #[cfg(feature = "debug")]
                hovered_marker_identifier,
                #[cfg(feature = "debug")]
                pathing_texture_set: &self.pathing_texture_set,
                #[cfg(feature = "debug")]
                tile_texture_set: &self.tile_texture_set,
                #[cfg(feature = "debug")]
                player_camera: &self.player_camera,
                #[cfg(feature = "debug")]
                start_camera: &self.start_camera,
                model_batches: &mut self.model_batches,
                model_instructions: &mut self.model_instructions,
                entity_instructions: &mut self.entity_instructions,
                directional_shadow_camera: &mut self.directional_shadow_camera,
                directional_shadow_model_batches: &mut self.directional_shadow_model_batches,
                directional_shadow_model_instructions: &mut self.directional_shadow_model_instructions,
                directional_shadow_entity_instructions: &mut self.directional_shadow_entity_instructions,
                point_shadow_camera: &mut self.point_shadow_camera,
                point_shadow_model_instructions: &mut self.point_shadow_model_instructions,
                point_shadow_entity_instructions: &mut self.point_shadow_entity_instructions,
                point_light_with_shadow_instructions: &mut self.point_light_with_shadow_instructions,
                point_light_instructions: &mut self.point_light_instructions,
                directional_shadow_object_set_buffer: &mut self.directional_shadow_object_set_buffer,
                point_shadow_object_set_buffer: &mut self.point_shadow_object_set_buffer,
                deferred_object_set_buffer: &mut self.deferred_object_set_buffer,
                indicator_instruction: &mut indicator_instruction,
                water_instruction: &mut water_instruction,
                particle_holder: &mut self.particle_holder,
                effect_holder: &mut self.effect_holder,
                cast_bars: &self.cast_bars,
                effect_renderer: &mut self.effect_renderer,
                bottom_interface_renderer: &self.bottom_interface_renderer,
                middle_interface_renderer: &mut self.middle_interface_renderer,
                #[cfg(feature = "debug")]
                aabb_instructions: &mut self.aabb_instructions,
                #[cfg(feature = "debug")]
                circle_instructions: &mut self.circle_instructions,
                #[cfg(feature = "debug")]
                rectangle_instructions: &mut self.rectangle_instructions,
                #[cfg(feature = "debug")]
                bounding_box_object_set_buffer: &mut self.bounding_box_object_set_buffer,
                #[cfg(feature = "debug")]
                debug_marker_renderer: &mut self.debug_marker_renderer,
            };

            #[cfg(feature = "debug")]
            render_context.render_markers();
            render_context.render_directional_shadows();
            render_context.render_point_lights();
            render_context.render_geometry();
            #[cfg(feature = "debug")]
            render_context.render_bounding_boxes();
            render_context.render_world_overlays();
        }

        let player_position = self.client_state.try_follow(this_entity()).map(|player| player.get_position());
        crate::interface::npc_overlay::render_npc_cutin(
            self.client_state.follow(client_state().npc_presentation()),
            self.game_file_loader.as_ref(),
            &self.texture_loader,
            &self.middle_interface_renderer,
            screen_size,
        );
        crate::interface::npc_overlay::render_npc_progress(
            self.client_state.follow(client_state().npc_presentation()),
            &self.middle_interface_renderer,
            current_camera,
            self.client_state.follow(client_state().world_theme()),
            screen_size,
            player_position,
            client_tick.0,
        );

        let in_game_theme_path = client_state().in_game_theme().tooltip();
        let menu_theme_path = client_state().menu_theme().tooltip();
        let tooltip_theme = match currently_playing {
            true => self.client_state.follow(in_game_theme_path),
            false => self.client_state.follow(menu_theme_path),
        };

        interface_frame.render(
            &self.client_state,
            &self.interface_renderer,
            tooltip_theme,
            input_report.mouse_position,
        );

        drop(interface_frame);

        self.render_ui_overlays(
            &input_report,
            scaling,
            #[cfg(feature = "debug")]
            &render_options,
        );

        let picker_position = ScreenPosition {
            left: input_report.mouse_position.left.clamp(0.0, screen_size.width),
            top: input_report.mouse_position.top.clamp(0.0, screen_size.height),
        };

        let uniforms = Uniforms {
            view_matrix,
            projection_matrix,
            camera_position,
            animation_timer_ms,
            ambient_light_color: map.ambient_light_color(),
            enhanced_lighting: lighting_mode == LightingMode::Enhanced,
            shadow_method,
            shadow_detail,
            use_sdsm,
            sdsm_enabled,
        };

        let interface_instructions = self.interface_renderer.get_instructions();
        let bottom_layer_instructions = self.bottom_interface_renderer.get_instructions();
        let middle_layer_instructions = self.middle_interface_renderer.get_instructions();
        let top_layer_instructions = self.top_interface_renderer.get_instructions();

        let directional_light = DirectionalLightInstruction {
            view_projection_matrix: self.directional_shadow_camera.view_projection_matrix(),
            direction: directional_light_direction,
            color: directional_light_color,
        };

        let render_instruction = RenderInstruction {
            show_interface: self.show_interface,
            picker_position,
            uniforms,
            indicator: indicator_instruction,
            interface: interface_instructions.as_slice(),
            bottom_layer_rectangles: bottom_layer_instructions.as_slice(),
            middle_layer_rectangles: middle_layer_instructions.as_slice(),
            top_layer_rectangles: top_layer_instructions.as_slice(),
            directional_light,
            directional_light_partitions: &self.directional_shadow_camera.get_partition_instructions(),
            point_light: &self.point_light_instructions,
            point_light_with_shadows: &self.point_light_with_shadow_instructions,
            model_batches: &self.model_batches,
            models: &mut self.model_instructions,
            entities: &mut self.entity_instructions,
            directional_shadow_model_batches: &self.directional_shadow_model_batches,
            directional_shadow_models: &self.directional_shadow_model_instructions,
            directional_shadow_entities: &mut self.directional_shadow_entity_instructions,
            point_shadow_models: &self.point_shadow_model_instructions,
            point_shadow_entities: &mut self.point_shadow_entity_instructions,
            effects: self.effect_renderer.get_instructions(),
            water: water_instruction,
            map_picker_tile_vertex_buffer: Some(map.get_tile_picker_vertex_buffer()),
            map_picker_tile_index_buffer: Some(map.get_tile_picker_index_buffer()),
            slug_font: Some(self.font_loader.get_slug_font()),
            #[cfg(feature = "debug")]
            render_options,
            #[cfg(feature = "debug")]
            aabb: &self.aabb_instructions,
            #[cfg(feature = "debug")]
            circles: &self.circle_instructions,
            #[cfg(feature = "debug")]
            rectangles: &self.rectangle_instructions,
            #[cfg(feature = "debug")]
            marker: self.debug_marker_renderer.get_instructions(),
        };

        if let Some(frame) = maybe_frame {
            self.graphics_engine.render_next_frame(frame, render_instruction);
        }
    }

}

/// Loads each guild skill's sprite from `SkillListInformation.file_name`.
///
/// The packet name is the Aegis name, so the path is keyed by skill id.
fn load_guild_skill_icons(state: &mut State<ClientState>, async_loader: &AsyncLoader, library: &Library, skill_ids: &[u16]) {
    for &skill_id in skill_ids {
        let information = library.get::<SkillListInformation>(SkillId(skill_id));
        let path = crate::state::guild::guild_skill_icon_path(&information.file_name);
        if let Some(sprite) = async_loader.request_skill_sprite_load(SkillId(skill_id), &path) {
            state.follow_mut(client_state().guild()).attach_skill_sprite(skill_id, sprite);
        }
        if let Some(actions) = async_loader.request_skill_actions_load(SkillId(skill_id), &path) {
            state.follow_mut(client_state().guild()).attach_skill_actions(skill_id, actions);
        }
    }
}

/// Sets the local entity's guild mark, then asks for the emblem image.
///
/// These take the two fields they touch so the network-event drain can keep
/// its borrow of `network_event_buffer`.
fn apply_local_guild_mark<Callback>(
    state: &mut State<ClientState>,
    networking: &mut NetworkingSystem<Callback>,
    guild_id: u32,
    version: u32,
) where
    Callback: ragnarok_packets::handler::PacketCallback + Send,
{
    if let Some(player) = state.try_follow_mut(this_entity()) {
        player.set_guild(guild_id, version);
    }
    request_emblem_if_needed(state, networking, guild_id, version);
}

fn request_emblem_if_needed<Callback>(
    state: &mut State<ClientState>,
    networking: &mut NetworkingSystem<Callback>,
    guild_id: u32,
    version: u32,
) where
    Callback: ragnarok_packets::handler::PacketCallback + Send,
{
    if guild_id == 0 || version == 0 {
        return;
    }
    if state.follow(client_state().guild()).wants_emblem(guild_id, version) {
        state.follow_mut(client_state().guild()).note_emblem_request(guild_id, version);
        let _ = networking.request_guild_emblem(guild_id);
    }
}

impl ApplicationHandler for Client {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // To be as portable as possible, winit recommends to initialize the window and
        // graphics backend after the first resume event is received.
        if self.window.is_none() {
            time_phase!("create window", {
                let reader = ImageReader::with_format(Cursor::new(ICON_DATA), ImageFormat::Png);
                let image_buffer = reader.decode().unwrap().to_rgba8();
                let image_data = image_buffer.as_bytes().to_vec();

                assert_eq!(image_buffer.width(), image_buffer.height(), "icon must be square");
                let icon = Icon::from_rgba(image_data, image_buffer.width(), image_buffer.height()).unwrap();

                let window_attributes = Window::default_attributes()
                    .with_inner_size(LogicalSize {
                        width: INITIAL_SCREEN_SIZE.width,
                        height: INITIAL_SCREEN_SIZE.height,
                    })
                    .with_title(CLIENT_NAME)
                    .with_window_icon(Some(icon))
                    .with_visible(false);
                let window = match event_loop.create_window(window_attributes) {
                    Ok(window) => Arc::new(window),
                    Err(error) => {
                        self.startup_error = Some(format!("Cannot create native window: {error}"));
                        event_loop.exit();
                        return;
                    }
                };

                let backend_name = self.graphics_engine.get_backend_name();
                window.set_title(&format!("{CLIENT_NAME} ({})", str::to_uppercase(&backend_name)));
                window.set_cursor_visible(false);

                self.window = Some(window);

                #[cfg(feature = "debug")]
                print_debug!("created {}", "window".magenta());
            });
        }

        // Android devices need to drop the surface on suspend, so we might need to
        // re-create it.
        if let Some(window) = self.window.as_ref() {
            let path = client_state().graphics_settings();
            let graphics_settings = self.client_state.follow(path);

            if let Err(error) = self.graphics_engine.on_resume(
                window.clone(),
                graphics_settings.triple_buffering,
                graphics_settings.vsync,
                graphics_settings.limit_framerate,
                graphics_settings.shadow_resolution,
                graphics_settings.texture_filtering,
                graphics_settings.msaa,
                graphics_settings.ssaa,
                graphics_settings.screen_space_anti_aliasing,
                graphics_settings.high_quality_interface,
            ) {
                self.startup_error = Some(format!("Cannot initialize window presentation: {error}"));
                event_loop.exit();
                return;
            }

            // Update graphics settings capabilities based on the new surface.
            // We don't expect the capabilities to change on consecutive calls but we
            // can't get the present mode info when initializing the client, so
            // we do it here instead.
            self.client_state
                .follow_mut(client_state().graphics_settings_capabilities())
                .update(
                    self.graphics_engine.get_supported_msaa(),
                    self.graphics_engine.get_present_mode_info(),
                );

            window.set_visible(true);
            if self.autopilot.is_some() {
                window.request_redraw();
            }
        }

        let shown = self
            .autopilot
            .as_mut()
            .map(|auto| auto.on_window_shown(Instant::now()))
            .unwrap_or_default();
        self.apply_autopilot(shown);

        if *self.client_state.follow(client_state().audio_settings().mute_on_focus_loss()) {
            self.audio_engine.mute(false);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(screen_size) => {
                let screen_size = screen_size.max(PhysicalSize::new(1, 1)).into();
                *self.client_state.follow_mut(client_state().window_size()) = screen_size;
                self.graphics_engine.on_resize(screen_size);
                self.interface.update_window_size(screen_size);
                self.interface_renderer.update_window_size(screen_size);
                self.bottom_interface_renderer.update_window_size(screen_size);
                self.middle_interface_renderer.update_window_size(screen_size);
                self.top_interface_renderer.update_window_size(screen_size);
                self.effect_renderer.update_window_size(screen_size);

                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::Focused(focused) => {
                if !focused {
                    self.input_system.reset();
                    self.keyboard_modifiers = ModifiersState::empty();
                }

                if *self.client_state.follow(client_state().audio_settings().mute_on_focus_loss()) {
                    self.audio_engine.mute(!focused);
                }
            }
            WindowEvent::CursorLeft { .. } => self.mouse_cursor.hide(),
            WindowEvent::CursorEntered { .. } => self.mouse_cursor.show(),
            WindowEvent::CursorMoved { position, .. } => self.input_system.update_mouse_position(position),
            WindowEvent::MouseInput { button, state, .. } => self.input_system.update_mouse_buttons(button, state),
            WindowEvent::MouseWheel { delta, .. } => self.input_system.update_mouse_wheel(delta),
            WindowEvent::ModifiersChanged(modifiers) => self.keyboard_modifiers = modifiers.state(),
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(keycode) = event.physical_key {
                    self.input_system.update_keyboard(keycode, event.state);
                }

                if event.state.is_pressed() && input::is_paste_shortcut(&event.logical_key, self.keyboard_modifiers) {
                    self.input_system
                        .paste_if_focused(self.interface.has_focus(), input::read_clipboard_text);
                    return;
                }

                // TODO: NHA We should also support IME in the long term (winit::event::Ime)
                if let Some(text) = event.text
                    && event.state.is_pressed()
                {
                    for char in text.chars() {
                        self.input_system.buffer_character(char);
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                #[cfg(feature = "debug")]
                let _measurement = threads::Main::start_frame();

                self.update_and_render(event_loop);

                if let Some(window) = self.window.as_mut() {
                    window.request_redraw();
                }
            }
            _ignored => {}
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.graphics_engine.on_suspended();

        if let Some(window) = self.window.as_ref() {
            window.set_visible(false);
        }

        if *self.client_state.follow(client_state().audio_settings().mute_on_focus_loss()) {
            self.audio_engine.mute(true);
        }
    }
}

/// Bundles all the borrows needed by the per-frame rendering pipeline so that
/// they can be passed as a single `self` argument.
struct MapRenderContext<'a, 'm: 'a> {
    map: &'m Map,
    current_camera: &'a (dyn Camera + Send + Sync),
    point_light_set: &'a PointLightSet<'a>,
    client_state: &'a State<ClientState>,
    library: &'a Library,
    mouse_position: ScreenPosition,
    mouse_target: PickerTarget,
    screen_size: ScreenSize,
    scaling: Scaling,
    client_tick: ClientTick,
    animation_timer_ms: f32,
    currently_playing: bool,
    is_mouse_mode_default: bool,
    is_interface_hovered: bool,
    last_walking_destination: Option<TilePosition>,
    buffered_action: Option<BufferedAction>,
    #[cfg(feature = "debug")]
    render_options: &'a RenderOptions,
    #[cfg(feature = "debug")]
    hovered_marker_identifier: Option<MarkerIdentifier>,
    #[cfg(feature = "debug")]
    pathing_texture_set: &'a Arc<TextureSet>,
    #[cfg(feature = "debug")]
    tile_texture_set: &'a Arc<TextureSet>,
    #[cfg(feature = "debug")]
    player_camera: &'a PlayerCamera,
    #[cfg(feature = "debug")]
    start_camera: &'a StartCamera,

    // Mutable rendering state
    model_batches: &'a mut Vec<ModelBatch>,
    model_instructions: &'a mut Vec<ModelInstruction>,
    entity_instructions: &'a mut Vec<EntityInstruction>,
    directional_shadow_camera: &'a mut DirectionalShadowCamera,
    directional_shadow_model_batches: &'a mut [Vec<ModelBatch>; PARTITION_COUNT],
    directional_shadow_model_instructions: &'a mut Vec<ModelInstruction>,
    directional_shadow_entity_instructions: &'a mut [Vec<EntityInstruction>; PARTITION_COUNT],
    point_shadow_camera: &'a mut PointShadowCamera,
    point_shadow_model_instructions: &'a mut Vec<ModelInstruction>,
    point_shadow_entity_instructions: &'a mut Vec<EntityInstruction>,
    point_light_with_shadow_instructions: &'a mut Vec<PointLightWithShadowInstruction>,
    point_light_instructions: &'a mut Vec<PointLightInstruction>,
    directional_shadow_object_set_buffer: &'a mut ResourceSetBuffer<ObjectKey>,
    point_shadow_object_set_buffer: &'a mut ResourceSetBuffer<ObjectKey>,
    deferred_object_set_buffer: &'a mut ResourceSetBuffer<ObjectKey>,
    indicator_instruction: &'a mut Option<IndicatorInstruction>,
    water_instruction: &'a mut Option<WaterInstruction<'m>>,
    particle_holder: &'a mut ParticleHolder,
    effect_holder: &'a mut EffectHolder,
    cast_bars: &'a effects::CastBars,
    effect_renderer: &'a mut EffectRenderer,
    bottom_interface_renderer: &'a GameInterfaceRenderer,
    middle_interface_renderer: &'a mut GameInterfaceRenderer,
    #[cfg(feature = "debug")]
    aabb_instructions: &'a mut Vec<DebugAabbInstruction>,
    #[cfg(feature = "debug")]
    circle_instructions: &'a mut Vec<DebugCircleInstruction>,
    #[cfg(feature = "debug")]
    rectangle_instructions: &'a mut Vec<DebugRectangleInstruction>,
    #[cfg(feature = "debug")]
    bounding_box_object_set_buffer: &'a mut ResourceSetBuffer<ObjectKey>,
    #[cfg(feature = "debug")]
    debug_marker_renderer: &'a mut DebugMarkerRenderer,
}

impl<'a, 'm: 'a> MapRenderContext<'a, 'm> {
    #[inline(always)]
    #[cfg(feature = "debug")]
    #[korangar_debug::profile]
    fn render_markers(&mut self) {
        let entities = self.client_state.follow(client_state().entities());

        self.map.render_markers(
            self.debug_marker_renderer,
            self.current_camera,
            self.render_options,
            entities,
            self.point_light_set,
            self.hovered_marker_identifier,
        );

        self.map.render_markers(
            self.middle_interface_renderer,
            self.current_camera,
            self.render_options,
            entities,
            self.point_light_set,
            self.hovered_marker_identifier,
        );
    }

    #[inline(always)]
    #[cfg_attr(feature = "debug", korangar_debug::profile)]
    fn render_directional_shadows(&mut self) {
        let entities = self.client_state.follow(client_state().entities());
        let dead_entities = self.client_state.follow(client_state().dead_entities());
        let ground_items = self.client_state.follow(client_state().ground_items());

        for partition_index in 0..PARTITION_COUNT {
            let partition_camera = self.directional_shadow_camera.get_partition_camera(partition_index);

            let object_set = self.map.cull_objects_with_frustum(
                &partition_camera,
                self.directional_shadow_object_set_buffer,
                #[cfg(feature = "debug")]
                self.render_options.frustum_culling,
            );

            let offset = self.directional_shadow_model_instructions.len();
            let model_batches = &mut self.directional_shadow_model_batches[partition_index];
            let entity_instructions = &mut self.directional_shadow_entity_instructions[partition_index];

            #[cfg_attr(feature = "debug", korangar_debug::debug_condition(self.render_options.show_objects))]
            self.map.render_objects(
                self.directional_shadow_model_instructions,
                &object_set,
                self.animation_timer_ms,
                &partition_camera,
            );

            #[cfg_attr(feature = "debug", korangar_debug::debug_condition(self.render_options.show_map))]
            self.map.render_ground(self.directional_shadow_model_instructions);

            let count = self.directional_shadow_model_instructions.len() - offset;

            model_batches.push(ModelBatch {
                offset,
                count,
                texture_set: self.map.get_texture_set().clone(),
                vertex_buffer: self.map.get_model_vertex_buffer().clone(),
                index_buffer: self.map.get_model_index_buffer().clone(),
            });

            #[cfg(feature = "debug")]
            #[cfg_attr(feature = "debug", korangar_debug::debug_condition(self.render_options.show_map_tiles))]
            self.map
                .render_overlay_tiles(self.directional_shadow_model_instructions, model_batches, self.tile_texture_set);

            #[cfg(feature = "debug")]
            #[cfg_attr(feature = "debug", korangar_debug::debug_condition(self.render_options.show_pathing))]
            self.map.render_entity_pathing(
                self.directional_shadow_model_instructions,
                model_batches,
                entities,
                self.pathing_texture_set,
            );

            #[cfg_attr(feature = "debug", korangar_debug::debug_condition(self.render_options.show_ground_items))]
            self.map
                .render_ground_items(entity_instructions, ground_items, &partition_camera, self.client_tick);

            #[cfg_attr(feature = "debug", korangar_debug::debug_condition(self.render_options.show_entities))]
            self.map
                .render_entities(entity_instructions, entities, &partition_camera, self.client_tick);

            #[cfg_attr(feature = "debug", korangar_debug::debug_condition(self.render_options.show_entities))]
            self.map
                .render_dead_entities(entity_instructions, dead_entities, &partition_camera, self.client_tick);
        }
    }

    #[inline(always)]
    #[cfg_attr(feature = "debug", korangar_debug::profile)]
    fn render_point_lights(&mut self) {
        let point_shadow_entities = PointShadowEntities {
            entities: self.client_state.follow(client_state().entities()),
            dead_entities: self.client_state.follow(client_state().dead_entities()),
            ground_items: self.client_state.follow(client_state().ground_items()),
            client_tick: self.client_tick,
        };

        #[cfg_attr(feature = "debug", korangar_debug::debug_condition(self.render_options.enable_point_lights))]
        self.point_light_set.render_point_lights(self.point_light_instructions);

        #[cfg_attr(feature = "debug", korangar_debug::debug_condition(self.render_options.enable_point_lights))]
        self.point_light_set.render_point_lights_with_shadows(
            self.map,
            self.point_shadow_camera,
            self.point_shadow_object_set_buffer,
            self.point_shadow_model_instructions,
            self.point_shadow_entity_instructions,
            self.point_light_with_shadow_instructions,
            &point_shadow_entities,
            self.animation_timer_ms,
            #[cfg(feature = "debug")]
            self.render_options,
        );
    }

    #[inline(always)]
    #[cfg_attr(feature = "debug", korangar_debug::profile)]
    fn render_geometry(&mut self) {
        let entities = self.client_state.follow(client_state().entities());
        let dead_entities = self.client_state.follow(client_state().dead_entities());
        let ground_items = self.client_state.follow(client_state().ground_items());

        let offset = self.model_instructions.len();
        let object_set = self.map.cull_objects_with_frustum(
            self.current_camera,
            self.deferred_object_set_buffer,
            #[cfg(feature = "debug")]
            self.render_options.frustum_culling,
        );

        #[cfg_attr(feature = "debug", korangar_debug::debug_condition(self.render_options.show_objects))]
        self.map.render_objects(
            self.model_instructions,
            &object_set,
            self.animation_timer_ms,
            self.current_camera,
        );

        #[cfg_attr(feature = "debug", korangar_debug::debug_condition(self.render_options.show_map))]
        self.map.render_ground(self.model_instructions);

        self.model_batches.push(ModelBatch {
            offset,
            count: self.model_instructions.len() - offset,
            texture_set: self.map.get_texture_set().clone(),
            vertex_buffer: self.map.get_model_vertex_buffer().clone(),
            index_buffer: self.map.get_model_index_buffer().clone(),
        });

        #[cfg(feature = "debug")]
        #[cfg_attr(feature = "debug", korangar_debug::debug_condition(self.render_options.show_map_tiles))]
        self.map
            .render_overlay_tiles(self.model_instructions, self.model_batches, self.tile_texture_set);

        #[cfg(feature = "debug")]
        #[cfg_attr(feature = "debug", korangar_debug::debug_condition(self.render_options.show_pathing))]
        self.map
            .render_entity_pathing(self.model_instructions, self.model_batches, entities, self.pathing_texture_set);

        let entity_camera: &dyn Camera = match true {
            #[cfg(feature = "debug")]
            _ if self.render_options.show_entities_paper => self.player_camera,
            _ => self.current_camera,
        };

        #[cfg_attr(feature = "debug", korangar_debug::debug_condition(self.render_options.show_ground_items))]
        self.map
            .render_ground_items(self.entity_instructions, ground_items, entity_camera, self.client_tick);

        // Gear's paperdoll does not replace the world sprite.
        let _ = crate::interface::windows::draw_player_in_world(true);

        #[cfg_attr(feature = "debug", korangar_debug::debug_condition(self.render_options.show_entities))]
        self.map
            .render_entities(self.entity_instructions, entities, entity_camera, self.client_tick);

        #[cfg_attr(feature = "debug", korangar_debug::debug_condition(self.render_options.show_entities))]
        self.map
            .render_dead_entities(self.entity_instructions, dead_entities, entity_camera, self.client_tick);

        #[cfg(feature = "debug")]
        if self.render_options.show_entities_debug {
            self.map.render_entities_debug(self.rectangle_instructions, entities, entity_camera);
            self.map
                .render_entities_debug(self.rectangle_instructions, dead_entities, entity_camera);
        }

        #[cfg_attr(feature = "debug", korangar_debug::debug_condition(self.render_options.show_water))]
        self.map.render_water(self.water_instruction, self.animation_timer_ms);
    }

    #[inline(always)]
    #[cfg(feature = "debug")]
    #[korangar_debug::profile]
    fn render_bounding_boxes(&mut self) {
        if self.render_options.show_bounding_boxes {
            let culling_camera: &dyn Camera = match self.currently_playing {
                true => self.player_camera,
                false => self.start_camera,
            };

            let object_set = self.map.cull_objects_with_frustum(
                culling_camera,
                self.bounding_box_object_set_buffer,
                self.render_options.frustum_culling,
            );

            self.map
                .render_bounding(self.aabb_instructions, self.render_options.frustum_culling, &object_set);
        }
    }

    #[inline(always)]
    #[cfg_attr(feature = "debug", korangar_debug::profile)]
    fn render_world_overlays(&mut self) {
        #[cfg(feature = "debug")]
        if let Some(marker_identifier) = self.hovered_marker_identifier {
            self.map.render_marker_overlay(
                self.aabb_instructions,
                self.circle_instructions,
                self.current_camera,
                marker_identifier,
                self.point_light_set,
                self.animation_timer_ms,
            );
        }

        self.particle_holder.render(
            self.bottom_interface_renderer,
            self.current_camera,
            self.screen_size,
            self.scaling,
            self.client_state.follow(client_state().entities()),
        );

        self.effect_holder.render(self.effect_renderer, self.current_camera);
        self.render_cast_bars();

        if let Some(player) = self.client_state.try_follow(this_entity()) {
            #[cfg(feature = "debug")]
            profile_block!("render player status");

            player.render_status(
                self.middle_interface_renderer,
                self.current_camera,
                self.client_state.follow(client_state().world_theme()),
                self.screen_size,
            );
        }

        let party_bars = self.client_state.follow(client_state().party()).world_health_bars();
        for (entity_id, health, maximum) in party_bars {
            let Some(entity) = self
                .client_state
                .follow(client_state().entities())
                .iter()
                .find(|entity| entity.get_entity_id() == entity_id)
            else {
                continue;
            };
            entity.render_party_health(
                self.middle_interface_renderer,
                self.current_camera,
                self.client_state.follow(client_state().world_theme()),
                self.screen_size,
                health as f32,
                maximum as f32,
            );
        }

        if let Some(BufferedAction::AttackEntity { entity_id }) = self.buffered_action
            && let Some(entity) = self
                .client_state
                .follow(client_state().entities())
                .iter()
                .find(|entity| entity.get_entity_id() == entity_id)
        {
            entity.render_status(
                self.middle_interface_renderer,
                self.current_camera,
                self.client_state.follow(client_state().world_theme()),
                self.screen_size,
            );
        }

        let emblem_sprites: Vec<crate::interface::guild_emblem_overlay::EmblemSprite> = self
            .client_state
            .follow(client_state().guild())
            .emblems
            .iter()
            .filter(|emblem| !emblem.pixels.is_empty())
            .map(|emblem| crate::interface::guild_emblem_overlay::EmblemSprite {
                guild_id: emblem.guild_id,
                version: emblem.version,
                width: emblem.width,
                height: emblem.height,
                pixels: emblem.pixels.clone(),
            })
            .collect();
        if !emblem_sprites.is_empty() {
            crate::interface::guild_emblem_overlay::render(&crate::interface::guild_emblem_overlay::EmblemOverlayFrame {
                renderer: self.middle_interface_renderer,
                camera: self.current_camera,
                entities: self.client_state.follow(client_state().entities()),
                emblems: &emblem_sprites,
                window_size: self.screen_size,
            });
        }

        let font_size = self.client_state.follow(client_state().in_game_theme()).text.font_size;
        let chat_theme = &self.client_state.follow(client_state().in_game_theme()).chat;
        crate::interface::chat_overlay::render(&crate::interface::chat_overlay::ChatOverlayFrame {
            renderer: self.middle_interface_renderer,
            camera: self.current_camera,
            entities: self.client_state.follow(client_state().entities()),
            bubbles: self.client_state.follow(client_state().chat_bubbles()),
            announce: self.client_state.follow(client_state().announce()),
            chat_theme,
            font_size,
            window_size: self.screen_size,
            now: self.client_tick.0,
        });

        match self.mouse_target {
            PickerTarget::Tile { x, y } => {
                // Only show if the mouse mode is default or walking.
                if self.currently_playing
                    && !self.is_interface_hovered
                    && (self.is_mouse_mode_default || self.last_walking_destination.is_some())
                {
                    let walk_indicator_color = *self.client_state.follow(client_state().world_theme().indicator().walking());

                    #[cfg_attr(feature = "debug", korangar_debug::debug_condition(self.render_options.show_indicators))]
                    self.map
                        .render_walk_indicator(self.indicator_instruction, walk_indicator_color, TilePosition { x, y });
                }
            }
            PickerTarget::Entity(entity_id) => {
                if !self.is_interface_hovered && self.is_mouse_mode_default {
                    if let Some(entity) = self
                        .client_state
                        .follow(client_state().entities())
                        .iter()
                        .find(|entity| entity.get_entity_id() == entity_id)
                    {
                        // Since the buffered attack entity will render its status anyway,
                        // we make sure not to render it here again if it's the same.
                        if !self
                            .buffered_action
                            .is_some_and(|buffered_action| buffered_action.is_attack_entity(entity_id))
                        {
                            entity.render_status(
                                self.middle_interface_renderer,
                                self.current_camera,
                                self.client_state.follow(client_state().world_theme()),
                                self.screen_size,
                            );
                        }

                        if let Some(name) = &entity.get_details() {
                            let name = name.split('#').next().unwrap();
                            self.middle_interface_renderer
                                .render_hover_text(name, self.scaling, self.mouse_position);
                        }
                    } else if let Some(item) = self
                        .client_state
                        .follow(client_state().ground_items())
                        .iter()
                        .find(|item| item.get_entity_id() == entity_id)
                    {
                        let name = self.library.get::<ItemName>(ItemNameKey {
                            item_id: item.item_id,
                            is_identified: item.is_identified,
                        });

                        // TODO: Don't allocate every frame
                        let text = format!("{name}: {}ea", item.quantity);
                        self.middle_interface_renderer
                            .render_hover_text(&text, self.scaling, self.mouse_position);
                    }
                }
            }
            _ => {}
        }

        crate::interface::shop_boards::render_shop_boards(
            self.middle_interface_renderer,
            self.current_camera,
            self.client_state,
            self.screen_size,
            self.mouse_position,
        );
    }

    fn render_cast_bars(&mut self) {
        let (bar_width, bar_height, border, background, spell_point_color) = {
            let theme = self.client_state.follow(client_state().world_theme());
            (
                theme.status_bar.enemy_bar_width,
                theme.status_bar.spell_point_height,
                theme.status_bar.border_size,
                theme.status_bar.background_color,
                theme.status_bar.spell_point_color,
            )
        };
        let entities = self.client_state.follow(client_state().entities());
        for (entity_id, progress) in self.cast_bars.iter_visible(self.client_tick.0) {
            let Some(entity) = entities.iter().find(|entity| entity.get_entity_id().0 == entity_id) else {
                continue;
            };
            let head = entity.get_position() + Vector3::new(0.0, 12.0, 0.0);
            let clip = self.current_camera.view_projection_matrix() * head.to_homogeneous();
            if clip.w <= 0.1 {
                continue;
            }
            let screen = self.current_camera.clip_to_screen_space(clip);
            let position = ScreenPosition {
                left: screen.x * self.screen_size.width,
                top: screen.y * self.screen_size.height,
            };
            self.middle_interface_renderer.render_rectangle(
                position - border - ScreenSize::only_width(bar_width / 2.0),
                ScreenSize {
                    width: bar_width,
                    height: bar_height,
                } + border * 2.0,
                background,
            );
            self.middle_interface_renderer.render_bar(
                position,
                ScreenSize {
                    width: bar_width,
                    height: bar_height,
                },
                spell_point_color,
                1.0,
                progress,
            );
        }
    }
}
