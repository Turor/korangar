#![cfg_attr(feature = "interface", feature(impl_trait_in_assoc_type))]
#![cfg_attr(feature = "interface", feature(negative_impls))]

mod entity;
mod equip_switch;
mod event;
mod hotkey;
mod inventory_lists;
mod items;
mod message;
mod packet_versions;
mod server;

use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use event::{
    CharacterServerDisconnectedEvent, DisconnectedEvent, LoginServerDisconnectedEvent, MapServerDisconnectedEvent, NetworkEventList,
};
use ragnarok_bytes::encoding::UTF_8;
use ragnarok_bytes::{ByteReader, ByteWriter, FromBytes};
use ragnarok_packets::handler::{DuplicateHandlerError, HandlerResult, NoPacketCallback, PacketCallback, PacketHandler};
use ragnarok_packets::*;
use server::{ServerConnectCommand, ServerConnection};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;

pub use self::entity::EntityData;
#[cfg(feature = "interface")]
pub use self::equip_switch::EquipSwitchSlotPathExt;
pub use self::equip_switch::{EquipSwitchSlot, assign_slot, position_label, replace_slots};
pub use self::event::{
    DisconnectReason, GradeBlessingOffer, GradeMaterialOffer, GuildMemberSnapshot, NetworkEvent, RefineMaterialOffer, SearchStoreHit,
    StatusChangeState,
};
pub use self::hotkey::HotkeyState;
pub use self::items::{InventoryItem, InventoryItemDetails, ItemQuantity, NoMetadata, SellItem, ShopItem};
pub use self::message::MessageColor;
pub use self::packet_versions::SupportedPacketVersion;
pub use self::packet_versions::eden::{
    EDEN_ACTION_ABANDON, EDEN_ACTION_ACCEPT, EDEN_ACTION_BUY, EDEN_ACTION_COMPLETE, EDEN_ACTION_LIST, format_eden_result, parse_eden_id,
};
pub use self::packet_versions::enchant::{format_enchant_result, parse_enchant_index, parse_enchant_item_id, parse_enchant_slot};
pub use self::packet_versions::mvp_minimap::{format_mvp_markers, minimap_point};
pub use self::packet_versions::rodex::{format_rodex_mails, parse_rodex_mail_id};
pub use self::packet_versions::stable::{format_stable_result, parse_stable_action, parse_stable_id};
pub use self::packet_versions::companions::{
    EL_VAR_HP, EL_VAR_MAX_HP, EL_VAR_MAX_SP, EL_VAR_SP, HOM_ACT_DELETE, HOM_ACT_FEED, MERC_ACT_DISMISS, MERC_VAR_ATTACK, MERC_VAR_ATTACK_SPEED,
    MERC_VAR_CRITICAL, MERC_VAR_DEFENSE, MERC_VAR_FAITH, MERC_VAR_FLEE, MERC_VAR_HIT, MERC_VAR_HP, MERC_VAR_KILLS, MERC_VAR_MAGIC_ATTACK,
    MERC_VAR_MAGIC_DEFENSE, MERC_VAR_MAX_HP, MERC_VAR_MAX_SP, MERC_VAR_SP, PET_ACT_FEED, PET_ACT_PERFORMANCE, PET_ACT_RETURN, PET_ACT_UNEQUIP,
    PET_KIND_ACCESSORY, PET_KIND_HUNGER, PET_KIND_INTIMACY, elemental_variable_name, format_elemental, format_homunculus_flags, format_pet,
    intimacy_label, mercenary_variable_name,
};
pub use self::packet_versions::warpra::{
    format_warpra_destinations, format_warpra_result, push_warpra_chunk, warpra_kind_name, warpra_level_requirement,
};
pub use self::server::{
    CharacterServerLoginData, LoginServerLoginData, NotConnectedError, UnifiedCharacterSelectionFailedReason, UnifiedLoginFailedReason,
};
use crate::server::NetworkTaskError;

/// Buffer for networking events. This struct exists to reduce heap allocations
/// and is purely an optimization.
pub struct NetworkEventBuffer(Vec<NetworkEvent>);

impl NetworkEventBuffer {
    pub fn drain(&mut self) -> std::vec::Drain<'_, NetworkEvent> {
        self.0.drain(..)
    }
}

/// Simple time synchronization using the Cristian's algorithm.
struct TimeSynchronization {
    request_send: Instant,
    request_received: Instant,
    client_tick: f64,
}

impl TimeSynchronization {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            request_send: now,
            request_received: now,
            client_tick: 100.0,
        }
    }

    /// Returns the client tick that must be used when sending the time
    /// synchronization request immediately after calling this function.
    fn request_client_tick(&mut self) -> u32 {
        let request_send = Instant::now();
        let elapsed = request_send.duration_since(self.request_received).as_secs_f64();
        (self.client_tick + (elapsed * 1000.0)) as u32
    }

    /// Returns the estimated client tick using the Cristian's algorithm.
    fn estimated_client_tick(&mut self, server_tick: u32, request_received: Instant) -> u32 {
        self.request_received = request_received;
        let round_trip_time = self.request_received.duration_since(self.request_send).as_secs_f64();
        let tick_adjustment = (round_trip_time / 2.0) * 1000.0;
        self.client_tick = f64::from(server_tick) + tick_adjustment;
        self.client_tick as u32
    }
}

pub struct NetworkingSystem<Callback> {
    command_sender: UnboundedSender<ServerConnectCommand>,
    time_synchronization: Arc<Mutex<TimeSynchronization>>,
    login_server_connection: ServerConnection,
    character_server_connection: ServerConnection,
    map_server_connection: ServerConnection,
    packet_callback: Callback,
}

impl NetworkingSystem<NoPacketCallback> {
    pub fn spawn() -> (Self, NetworkEventBuffer) {
        let (command_sender, time_synchronization) = Self::spawn_networking_thread(NoPacketCallback);
        Self::inner_new(command_sender, time_synchronization, NoPacketCallback)
    }
}

impl<Callback> NetworkingSystem<Callback>
where
    Callback: PacketCallback + Send,
{
    fn inner_new(
        command_sender: UnboundedSender<ServerConnectCommand>,
        time_synchronization: Arc<Mutex<TimeSynchronization>>,
        packet_callback: Callback,
    ) -> (Self, NetworkEventBuffer) {
        let networking_system = Self {
            command_sender,
            time_synchronization,
            login_server_connection: ServerConnection::Disconnected,
            character_server_connection: ServerConnection::Disconnected,
            map_server_connection: ServerConnection::Disconnected,
            packet_callback,
        };
        let event_buffer = NetworkEventBuffer(Vec::new());

        (networking_system, event_buffer)
    }

    pub fn spawn_with_callback(packet_callback: Callback) -> (Self, NetworkEventBuffer) {
        let (command_sender, time_synchronization) = Self::spawn_networking_thread(packet_callback.clone());
        Self::inner_new(command_sender, time_synchronization, packet_callback)
    }

    fn spawn_networking_thread(packet_callback: Callback) -> (UnboundedSender<ServerConnectCommand>, Arc<Mutex<TimeSynchronization>>) {
        let (command_sender, mut command_receiver) = tokio::sync::mpsc::unbounded_channel::<ServerConnectCommand>();
        let time_synchronization = Arc::new(Mutex::new(TimeSynchronization::new()));
        let thread_time_synchronization = Arc::clone(&time_synchronization);

        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();

            let _guard = runtime.enter();
            let local_set = tokio::task::LocalSet::new();

            let mut login_server_task_handle: Option<JoinHandle<Result<(), NetworkTaskError>>> = None;
            let mut character_server_task_handle: Option<JoinHandle<Result<(), NetworkTaskError>>> = None;
            let mut map_server_task_handle: Option<JoinHandle<Result<(), NetworkTaskError>>> = None;

            local_set.block_on(&runtime, async {
                while let Some(command) = command_receiver.recv().await {
                    match command {
                        ServerConnectCommand::Login {
                            address,
                            action_receiver,
                            event_sender,
                            packet_version,
                        } => {
                            if let Some(handle) = login_server_task_handle.take() {
                                // TODO: Maybe add a timeout here? Maybe handle Result?
                                let _ = handle.await.unwrap();
                            }

                            let packet_handler = Self::create_login_server_packet_handler(packet_callback.clone(), packet_version).unwrap();
                            let handle = local_set.spawn_local(Self::handle_server_connection(
                                address,
                                action_receiver,
                                event_sender,
                                packet_handler,
                                |_| LoginServerKeepalivePacket::new(),
                                Duration::from_secs(58),
                                false,
                                packet_version,
                                thread_time_synchronization.clone(),
                            ));

                            login_server_task_handle = Some(handle);
                        }
                        ServerConnectCommand::Character {
                            address,
                            action_receiver,
                            event_sender,
                            packet_version,
                        } => {
                            if let Some(handle) = character_server_task_handle.take() {
                                // TODO: Maybe add a timeout here? Maybe handle Result?
                                let _ = handle.await.unwrap();
                            }

                            let packet_handler =
                                Self::create_character_server_packet_handler(packet_callback.clone(), packet_version).unwrap();
                            let handle = local_set.spawn_local(Self::handle_server_connection(
                                address,
                                action_receiver,
                                event_sender,
                                packet_handler,
                                |_| CharacterServerKeepalivePacket::new(),
                                Duration::from_secs(10),
                                true,
                                packet_version,
                                thread_time_synchronization.clone(),
                            ));

                            character_server_task_handle = Some(handle);
                        }
                        ServerConnectCommand::Map {
                            address,
                            action_receiver,
                            event_sender,
                            packet_version,
                        } => {
                            if let Some(handle) = map_server_task_handle.take() {
                                // TODO: Maybe add a timeout here? Maybe handle Result?
                                let _ = handle.await.unwrap();
                            }

                            let packet_handler = Self::create_map_server_packet_handler(packet_callback.clone(), packet_version).unwrap();
                            let handle = local_set.spawn_local(Self::handle_server_connection(
                                address,
                                action_receiver,
                                event_sender,
                                packet_handler,
                                |time_synchronization| match time_synchronization.lock() {
                                    Ok(mut time_synchronization) => {
                                        let client_tick = time_synchronization.request_client_tick();
                                        RequestServerTickPacket::new(ClientTick(client_tick))
                                    }
                                    Err(_) => RequestServerTickPacket::new(ClientTick(100)),
                                },
                                Duration::from_secs(10),
                                false,
                                packet_version,
                                thread_time_synchronization.clone(),
                            ));

                            map_server_task_handle = Some(handle);
                        }
                    }
                }
            });
        });

        (command_sender, time_synchronization)
    }

    fn handle_connection<Event>(connection: &mut ServerConnection, event_buffer: &mut NetworkEventBuffer)
    where
        Event: DisconnectedEvent,
    {
        match connection.take() {
            ServerConnection::Connected {
                action_sender,
                mut event_receiver,
                packet_version,
            } => loop {
                match event_receiver.try_recv() {
                    Ok(login_event) => {
                        event_buffer.0.push(login_event);
                    }
                    Err(TryRecvError::Empty) => {
                        *connection = ServerConnection::Connected {
                            action_sender,
                            event_receiver,
                            packet_version,
                        };
                        break;
                    }
                    Err(..) => {
                        event_buffer.0.push(Event::create_event(DisconnectReason::ConnectionError));
                        *connection = ServerConnection::Disconnected;
                        break;
                    }
                }
            },
            ServerConnection::ClosingManually => {
                event_buffer.0.push(Event::create_event(DisconnectReason::ClosedByClient));
                *connection = ServerConnection::Disconnected;
            }
            _ => (),
        };
    }

    pub fn get_events(&mut self, events: &mut NetworkEventBuffer) {
        Self::handle_connection::<LoginServerDisconnectedEvent>(&mut self.login_server_connection, events);
        Self::handle_connection::<CharacterServerDisconnectedEvent>(&mut self.character_server_connection, events);
        Self::handle_connection::<MapServerDisconnectedEvent>(&mut self.map_server_connection, events);
    }

    #[allow(clippy::too_many_arguments)]
    async fn handle_server_connection<PingPacket>(
        address: SocketAddr,
        mut action_receiver: UnboundedReceiver<Vec<u8>>,
        event_sender: UnboundedSender<NetworkEvent>,
        mut packet_handler: PacketHandler<NetworkEventList, Callback>,
        ping_factory: impl Fn(&Mutex<TimeSynchronization>) -> PingPacket,
        ping_frequency: Duration,
        // After logging in to the character server, it sends the account id without any packet.
        // Since our packet handler has no way of working with this, we need to add some special
        // logic.
        mut read_account_id: bool,
        packet_version: SupportedPacketVersion,
        time_synchronization: Arc<Mutex<TimeSynchronization>>,
    ) -> Result<(), NetworkTaskError>
    where
        PingPacket: Packet + ClientPacket,
        Callback: PacketCallback,
    {
        let mut stream = TcpStream::connect(address).await.map_err(|_| NetworkTaskError::FailedToConnect)?;
        let mut interval = tokio::time::interval(ping_frequency);
        // A tokio interval fires immediately on the first tick, which would race the
        // ping against the login packet. rAthena's map server requires the first read
        // of a session to contain exactly the login packet, so delay the first ping
        // by one full period.
        interval.reset();
        let mut buffer = [0u8; 8192];
        let mut pending = Vec::new();
        let mut events = Vec::new();
        let mut byte_writer = ByteWriter::with_encoding(UTF_8);

        loop {
            tokio::select! {
                // Send a packet to the server.
                action = action_receiver.recv() => {
                    let Some(action) = action else {
                        // Channel was closed by the main thread.
                        break Ok(());
                    };

                    stream.write_all(&action).await.map_err(|_| NetworkTaskError::ConnectionClosed)?;
                }
                // Receive some packets from the server.
                received_bytes = stream.read(&mut buffer) => {
                    let Ok(received_bytes) = received_bytes else {
                        // Channel was closed by the main thread.
                        break Err(NetworkTaskError::ConnectionClosed);
                    };

                    if received_bytes == 0 {
                        // Receiving Ok(0) means the stream was closed by the server, most
                        // likely because the client sent an incorrect packet.
                        break Err(NetworkTaskError::ConnectionClosed);
                    }

                    // TCP has no packet boundaries. Preserve partial headers, the
                    // raw character-server prefix, and frames larger than one read.
                    if pending.len() + received_bytes > 131_072 {
                        return Err(NetworkTaskError::ConnectionClosed);
                    }
                    pending.extend_from_slice(&buffer[..received_bytes]);
                    let mut consumed = 0;
                    if read_account_id {
                        if pending.len() < 4 { continue; }
                        let mut reader = ByteReader::without_metadata(&pending[..4]);
                        let account_id = AccountId::from_bytes(&mut reader).map_err(|_| NetworkTaskError::ConnectionClosed)?;
                        events.push(NetworkEvent::AccountId { account_id });
                        consumed = 4;
                        read_account_id = false;
                    }
                    while consumed < pending.len() {
                        let available = &pending[consumed..];
                        let framed = matches!(packet_version, SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716);
                        let frame_size = if framed {
                            match match packet_version {
                                SupportedPacketVersion::_20250716 => packet_versions::framing_20250716::frame_length(available).map_err(|_|()),
                                _ => packet_versions::framing_20190530::frame_length(available).map_err(|_|()),
                            } {
                                Ok(Some(size)) => size,
                                Ok(None) => break,
                                Err(_) => return Err(NetworkTaskError::ConnectionClosed),
                            }
                        } else { available.len() };
                        if framed {
                            let header = PacketHeader(u16::from_le_bytes([available[0], available[1]]));
                            if !packet_handler.has_handler(header) {
                                // Known-size unsupported UI/plugin messages cannot
                                // eat the following frame or leak opaque payloads.
                                packet_handler.report_unsupported(header);
                                consumed += frame_size;
                                continue;
                            }
                        }
                        let mut reader = ByteReader::without_metadata(&available[..frame_size]);
                        reader.set_encoding(UTF_8);
                        match packet_handler.process_one(&mut reader) {
                            HandlerResult::Ok(packet_events) => {
                                if reader.get_offset() == 0 || (framed && !reader.is_empty()) {
                                    return Err(NetworkTaskError::ConnectionClosed);
                                }
                                consumed += reader.get_offset();
                                events.extend(packet_events.0.into_iter());
                            }
                            HandlerResult::PacketCutOff if !framed => break,
                            // A complete 2019 frame that the codec cannot decode
                            // is a profile mismatch, not permission to guess.
                            _ => return Err(NetworkTaskError::ConnectionClosed),
                        }
                    }
                    pending.drain(..consumed);

                    for event in events.drain(..) {
                        if let NetworkEvent::UpdateClientTick {client_tick,received_at} = &event && let Ok(mut time_synchronization) = time_synchronization.lock() {
                            time_synchronization.estimated_client_tick(client_tick.0, *received_at);
                        }

                        event_sender.send(event).map_err(|_| NetworkTaskError::ConnectionClosed)?;
                    }
                }
                // Send a keep-alive packet to the server.
                _ = interval.tick() => {
                    ping_factory(&time_synchronization).packet_to_bytes(&mut byte_writer).unwrap();
                    stream.write_all(byte_writer.as_slice()).await.map_err(|_| NetworkTaskError::ConnectionClosed)?;
                    byte_writer.clear();
                }
            }
        }
    }

    pub fn connect_to_login_server(
        &mut self,
        packet_version: SupportedPacketVersion,
        address: SocketAddr,
        username: impl Into<String>,
        password: impl Into<String>,
    ) {
        if !matches!(self.login_server_connection, ServerConnection::Disconnected) {
            return;
        }

        let (action_sender, action_receiver) = tokio::sync::mpsc::unbounded_channel();
        let (event_sender, event_receiver) = tokio::sync::mpsc::unbounded_channel();

        self.command_sender
            .send(ServerConnectCommand::Login {
                address,
                action_receiver,
                event_sender,
                packet_version,
            })
            .expect("network thread dropped");

        let mut login_packet = LoginServerLoginPacket::new(username.into(), password.into());
        match packet_version {
            SupportedPacketVersion::_20190530 => login_packet.version = 20190530u32.to_le_bytes(),
            SupportedPacketVersion::_20250716 => login_packet.version = 20250716u32.to_le_bytes(),
            _ => {}
        }

        // Authentication/session material is intentionally excluded from packet
        // callbacks.

        let mut byte_writer = ByteWriter::with_encoding(UTF_8);
        login_packet.packet_to_bytes(&mut byte_writer).unwrap();
        action_sender
            .send(byte_writer.into_inner())
            .expect("action receiver instantly dropped");

        self.login_server_connection = ServerConnection::Connected {
            action_sender,
            event_receiver,
            packet_version,
        };
    }

    pub fn connect_to_character_server(
        &mut self,
        packet_version: SupportedPacketVersion,
        login_data: &LoginServerLoginData,
        server: CharacterServerInformation,
    ) {
        if !matches!(self.character_server_connection, ServerConnection::Disconnected) {
            return;
        }

        let (action_sender, action_receiver) = tokio::sync::mpsc::unbounded_channel();
        let (event_sender, event_receiver) = tokio::sync::mpsc::unbounded_channel();

        let address = SocketAddr::new(IpAddr::V4(server.server_ip.into()), server.server_port);

        self.command_sender
            .send(ServerConnectCommand::Character {
                address,
                action_receiver,
                event_sender,
                packet_version,
            })
            .expect("network thread dropped");

        let login_packet = CharacterServerLoginPacket::new(
            login_data.account_id,
            login_data.login_id1,
            login_data.login_id2,
            login_data.sex,
        );

        // Authentication/session material is intentionally excluded from packet
        // callbacks.

        let mut byte_writer = ByteWriter::with_encoding(UTF_8);
        login_packet.packet_to_bytes(&mut byte_writer).unwrap();
        action_sender
            .send(byte_writer.into_inner())
            .expect("action receiver instantly dropped");

        self.character_server_connection = ServerConnection::Connected {
            action_sender,
            event_receiver,
            packet_version,
        };
    }

    pub fn connect_to_map_server(
        &mut self,
        packet_version: SupportedPacketVersion,
        login_server_login_data: &LoginServerLoginData,
        character_server_login_data: CharacterServerLoginData,
    ) {
        if !matches!(self.map_server_connection, ServerConnection::Disconnected) {
            return;
        }

        let (action_sender, action_receiver) = tokio::sync::mpsc::unbounded_channel();
        let (event_sender, event_receiver) = tokio::sync::mpsc::unbounded_channel();

        let address = SocketAddr::new(character_server_login_data.server_ip, character_server_login_data.server_port);

        self.command_sender
            .send(ServerConnectCommand::Map {
                address,
                action_receiver,
                event_sender,
                packet_version,
            })
            .expect("network thread dropped");

        let mut byte_writer = ByteWriter::with_encoding(UTF_8);
        match packet_version {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20250716 => MapServerLoginPacket::new(
                login_server_login_data.account_id,
                character_server_login_data.character_id,
                login_server_login_data.login_id1,
                ClientTick(100),
                login_server_login_data.sex,
            )
            .packet_to_bytes(&mut byte_writer)
            .unwrap(),
            SupportedPacketVersion::_20190530 => ragnarok_packets::version_20190530::MapServerLoginPacket::new(
                login_server_login_data.account_id,
                character_server_login_data.character_id,
                login_server_login_data.login_id1,
                ClientTick(100),
                login_server_login_data.sex,
            )
            .packet_to_bytes(&mut byte_writer)
            .unwrap(),
        };
        action_sender
            .send(byte_writer.into_inner())
            .expect("action receiver instantly dropped");

        self.map_server_connection = ServerConnection::Connected {
            action_sender,
            event_receiver,
            packet_version,
        };
    }

    pub fn disconnect_from_login_server(&mut self) {
        self.login_server_connection = ServerConnection::ClosingManually;
    }

    pub fn disconnect_from_character_server(&mut self) {
        self.character_server_connection = ServerConnection::ClosingManually;
    }

    pub fn disconnect_from_map_server(&mut self) {
        self.map_server_connection = ServerConnection::ClosingManually;
    }

    pub fn is_login_server_connected(&self) -> bool {
        matches!(self.login_server_connection, ServerConnection::Connected { .. })
    }

    pub fn is_character_server_connected(&self) -> bool {
        matches!(self.character_server_connection, ServerConnection::Connected { .. })
    }

    pub fn is_map_server_connected(&self) -> bool {
        matches!(self.map_server_connection, ServerConnection::Connected { .. })
    }

    fn character_server_packet_version(&self) -> Result<SupportedPacketVersion, NotConnectedError> {
        match &self.character_server_connection {
            ServerConnection::Connected { packet_version, .. } => Ok(*packet_version),
            _ => Err(NotConnectedError),
        }
    }

    fn map_server_packet_version(&self) -> Result<SupportedPacketVersion, NotConnectedError> {
        match &self.map_server_connection {
            ServerConnection::Connected { packet_version, .. } => Ok(*packet_version),
            _ => Err(NotConnectedError),
        }
    }

    fn send_character_server_packet(&mut self, packet: impl CharacterServerPacket) -> Result<(), NotConnectedError> {
        match &mut self.character_server_connection {
            ServerConnection::Connected { action_sender, .. } => {
                self.packet_callback.outgoing_packet(&packet);

                // FIX: Don't unwrap.
                let mut byte_writer = ByteWriter::with_encoding(UTF_8);
                packet.packet_to_bytes(&mut byte_writer).unwrap();
                action_sender.send(byte_writer.into_inner()).map_err(|_| NotConnectedError)
            }
            _ => Err(NotConnectedError),
        }
    }

    fn send_map_server_packet(&mut self, packet: impl MapServerPacket) -> Result<(), NotConnectedError> {
        match &mut self.map_server_connection {
            ServerConnection::Connected { action_sender, .. } => {
                self.packet_callback.outgoing_packet(&packet);

                // FIX: Don't unwrap.
                let mut byte_writer = ByteWriter::with_encoding(UTF_8);
                packet.packet_to_bytes(&mut byte_writer).unwrap();
                action_sender.send(byte_writer.into_inner()).map_err(|_| NotConnectedError)
            }
            _ => Err(NotConnectedError),
        }
    }

    fn create_login_server_packet_handler(
        packet_callback: Callback,
        packet_version: SupportedPacketVersion,
    ) -> Result<PacketHandler<NetworkEventList, Callback>, DuplicateHandlerError> {
        let mut packet_handler = PacketHandler::<NetworkEventList, Callback>::with_callback(packet_callback).without_callbacks();

        match packet_version {
            SupportedPacketVersion::_20220406 => packet_versions::version_20220406::register_login_server_packets(&mut packet_handler)?,
            SupportedPacketVersion::_20250716 => packet_versions::version_20250716::register_login_server_packets(&mut packet_handler)?,
            SupportedPacketVersion::_20190530 => packet_versions::version_20190530::register_login_server_packets(&mut packet_handler)?,
        }

        Ok(packet_handler)
    }

    fn create_character_server_packet_handler(
        packet_callback: Callback,
        packet_version: SupportedPacketVersion,
    ) -> Result<PacketHandler<NetworkEventList, Callback>, DuplicateHandlerError> {
        let mut packet_handler = PacketHandler::<NetworkEventList, Callback>::with_callback(packet_callback).without_callbacks();

        match packet_version {
            SupportedPacketVersion::_20220406 => packet_versions::version_20220406::register_character_server_packets(&mut packet_handler)?,
            SupportedPacketVersion::_20250716 => packet_versions::version_20250716::register_character_server_packets(&mut packet_handler)?,
            SupportedPacketVersion::_20190530 => packet_versions::version_20190530::register_character_server_packets(&mut packet_handler)?,
        }

        Ok(packet_handler)
    }

    fn create_map_server_packet_handler(
        packet_callback: Callback,
        packet_version: SupportedPacketVersion,
    ) -> Result<PacketHandler<NetworkEventList, Callback>, DuplicateHandlerError> {
        let mut packet_handler = PacketHandler::<NetworkEventList, Callback>::with_callback(packet_callback);

        match packet_version {
            SupportedPacketVersion::_20220406 => packet_versions::version_20220406::register_map_server_packets(&mut packet_handler)?,
            SupportedPacketVersion::_20250716 => packet_versions::version_20250716::register_map_server_packets(&mut packet_handler)?,
            SupportedPacketVersion::_20190530 => packet_versions::version_20190530::register_map_server_packets(&mut packet_handler)?,
        }

        Ok(packet_handler)
    }

    pub fn request_character_list(&mut self) -> Result<(), NotConnectedError> {
        match self.character_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_character_server_packet(RequestCharacterListPacket::default())
            }
        }
    }

    pub fn select_character(&mut self, character_slot: usize) -> Result<(), NotConnectedError> {
        match self.character_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_character_server_packet(SelectCharacterPacket::new(character_slot as u8))
            }
        }
    }

    pub fn create_character(&mut self, slot: usize, name: String) -> Result<(), NotConnectedError> {
        let hair_color = 0;
        let hair_style = 0;
        let start_job_id = JobId(0);
        let sex = Sex::Male;

        match self.character_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => self
                .send_character_server_packet(CreateCharacterPacket::new(
                    name,
                    slot as u8,
                    hair_color,
                    hair_style,
                    start_job_id,
                    sex,
                )),
        }
    }

    pub fn delete_character(&mut self, character_id: CharacterId) -> Result<(), NotConnectedError> {
        let email = "a@a.com".to_string();

        match self.character_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_character_server_packet(DeleteCharacterPacket::new(character_id, email))
            }
        }
    }

    pub fn switch_character_slot(&mut self, origin_slot: usize, destination_slot: usize) -> Result<(), NotConnectedError> {
        match self.character_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_character_server_packet(SwitchCharacterSlotPacket::new(origin_slot as u16, destination_slot as u16))
            }
        }
    }

    pub fn map_loaded(&mut self) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(MapLoadedPacket::default())
            }
        }
    }

    pub fn request_client_tick(&mut self) -> Result<(), NotConnectedError> {
        let client_tick = self
            .time_synchronization
            .lock()
            .map(|time_synchronization| time_synchronization.client_tick as u32)
            .unwrap_or(100);

        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(RequestServerTickPacket::new(ClientTick(client_tick)))
            }
        }
    }

    pub fn respawn(&mut self) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(RestartPacket::new(RestartType::Respawn))
            }
        }
    }

    pub fn log_out(&mut self) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(RestartPacket::new(RestartType::Disconnect))
            }
        }
    }

    pub fn player_move(&mut self, position: WorldPosition) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 => self.send_map_server_packet(RequestPlayerMovePacket::new(position)),
            SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(ragnarok_packets::version_20190530::RequestPlayerMovePacket::new(position))
            }
        }
    }

    pub fn warp_to_map(&mut self, map_name: String, position: TilePosition) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(RequestWarpToMapPacket::new(map_name, position))
            }
        }
    }

    pub fn entity_details(&mut self, entity_id: EntityId) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(RequestDetailsPacket::new(entity_id))
            }
        }
    }

    pub fn player_attack(&mut self, entity_id: EntityId) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(RequestActionPacket::new(entity_id, Action::Attack))
            }
        }
    }

    pub fn send_emotion(&mut self, emotion: u8) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(ragnarok_packets::version_20250716::RequestEmotionPacket::new(emotion as u16))
            }
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 => {
                self.send_map_server_packet(RequestEmotionPacket::new(emotion))
            }
        }
    }

    pub fn pick_up_item(&mut self, entity_id: EntityId) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(ItemPickupRequestPacket::new(entity_id))
            }
        }
    }

    pub fn send_chat_message(&mut self, player_name: &str, text: &str) -> Result<(), NotConnectedError> {
        let message = format!("{} : {}", player_name, text);

        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 => self.send_map_server_packet(GlobalMessagePacket::new(message)),
            SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(ragnarok_packets::version_20190530::GlobalMessagePacket::new(message))
            }
        }
    }

    pub fn start_dialog(&mut self, npc_id: EntityId) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(StartDialogPacket::new(npc_id))
            }
        }
    }

    pub fn next_dialog(&mut self, npc_id: EntityId) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(NextDialogPacket::new(npc_id))
            }
        }
    }

    pub fn close_dialog(&mut self, npc_id: EntityId) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(CloseDialogPacket::new(npc_id))
            }
        }
    }

    pub fn choose_dialog_option(&mut self, npc_id: EntityId, option: i8) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(ChooseDialogOptionPacket::new(npc_id, option))
            }
        }
    }

    pub fn request_item_equip(&mut self, item_index: InventoryIndex, equip_position: EquipPosition) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(RequestEquipItemPacket::new(item_index, equip_position))
            }
        }
    }

    pub fn request_item_unequip(&mut self, item_index: InventoryIndex) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(RequestUnequipItemPacket::new(item_index))
            }
        }
    }

    pub fn move_item_to_storage(&mut self, inventory_index: InventoryIndex, amount: u32) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(MoveItemToStoragePacket::new(inventory_index, amount))
            }
        }
    }

    pub fn move_item_from_storage(&mut self, storage_index: InventoryIndex, amount: u32) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(MoveItemFromStoragePacket::new(storage_index, amount))
            }
        }
    }

    pub fn close_storage(&mut self) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(CloseStoragePacket::new())
            }
        }
    }

    /// `CZ_OPEN_MAILBOX2` (`0x0AC0`, 26 bytes). `mail_id` 0 starts at the newest mail.
    pub fn open_rodex(&mut self, mail_id: i64) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(OpenRodexPacket {
                    mail_id,
                    unused: [0; 16],
                })
            }
        }
    }

    /// `CZ_REQ_REFRESH_MAIL_LIST2` (`0x0AC1`, 26 bytes). Reloads the inbox.
    pub fn refresh_rodex(&mut self, mail_id: i64) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(RefreshRodexPacket {
                    mail_id,
                    unused: [0; 16],
                })
            }
        }
    }

    /// `CZ_CLOSE_MAILBOX` (`0x09E9`).
    pub fn close_rodex(&mut self) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(CloseRodexPacket::new())
            }
        }
    }

    /// `CZ_REQ_READ_MAIL` (`0x09EA`).
    pub fn read_rodex(&mut self, open_type: u8, mail_id: i64) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(ReadRodexPacket { open_type, mail_id })
            }
        }
    }

    /// `CZ_REQ_DELETE_MAIL` (`0x09F5`).
    pub fn delete_rodex(&mut self, open_type: u8, mail_id: i64) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(DeleteRodexPacket { open_type, mail_id })
            }
        }
    }

    /// `CZ_CLOSE_UI_ENCHANT` (`0x0BA0`). Required so the map server unfreezes
    /// movement.
    pub fn close_ui_enchant(&mut self) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(CloseUiEnchantPacket::new())
            }
        }
    }

    pub fn request_random_enchant(&mut self, enchant_group: i64, inventory_index: i16) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => self
                .send_map_server_packet(RequestRandomEnchantPacket {
                    enchant_group,
                    inventory_index,
                }),
        }
    }

    pub fn request_perfect_enchant(&mut self, enchant_group: i64, inventory_index: i16, item_id: ItemId) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => self
                .send_map_server_packet(RequestPerfectEnchantPacket {
                    enchant_group,
                    inventory_index,
                    item_id,
                }),
        }
    }

    pub fn request_upgrade_enchant(&mut self, enchant_group: i64, inventory_index: i16, slot: i16) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => self
                .send_map_server_packet(RequestUpgradeEnchantPacket {
                    enchant_group,
                    inventory_index,
                    slot,
                }),
        }
    }

    /// `0x0EF0`. Asks the map server for the stable board.
    pub fn request_stable_open(&mut self) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(RequestStableOpenPacket::new())
            }
        }
    }

    /// `0x0EF1`.
    pub fn request_stable_homunculus(&mut self, homunculus_id: i32) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(RequestStableHomunculusPacket { homunculus_id })
            }
        }
    }

    /// `0x0EF2`.
    pub fn request_stable_pet(&mut self, action: i16, pet_id: i32) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(RequestStablePetPacket { action, pet_id })
            }
        }
    }

    /// `0x0EF3`. `quest_id` is a mission id, or a shop slot for a buy.
    pub fn request_eden(&mut self, action: i16, quest_id: i32) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(RequestEdenPacket { action, quest_id })
            }
        }
    }

    /// `CZ_COMMAND_PET` (`0x01A1`).
    pub fn command_pet(&mut self, action: u8) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(PetCommandPacket { action })
            }
        }
    }

    /// `CZ_COMMAND_MER` (`0x022D`). `kind` is feed (`1`) or release (`2`).
    pub fn command_homunculus(&mut self, kind: u16) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => self
                .send_map_server_packet(HomunculusCommandPacket {
                    kind,
                    command: kind as u8,
                }),
        }
    }

    /// `CZ_MER_COMMAND` (`0x029F`).
    pub fn command_mercenary(&mut self, command: u8) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(MercenaryCommandPacket { command })
            }
        }
    }

    /// `0x0EF4`. `action` is list (`0`), warp (`1`), or unlock (`2`).
    pub fn request_warpra(&mut self, action: i16, kind: i16, location_id: i16) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(RequestWarpraPacket { action, kind, location_id })
            }
        }
    }

    pub fn request_reset_enchant(&mut self, enchant_group: i64, inventory_index: i16) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => self
                .send_map_server_packet(RequestResetEnchantPacket {
                    enchant_group,
                    inventory_index,
                }),
        }
    }

    pub fn request_equip_switch(&mut self) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(RequestEquipSwitchPacket::new())
            }
        }
    }

    /// Add one item to the alternate equipment set (`0x0A97`).
    pub fn request_equip_switch_add(
        &mut self,
        item_index: InventoryIndex,
        equip_position: EquipPosition,
    ) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(RequestEquipSwitchAddPacket {
                    inventory_index: item_index,
                    equip_position,
                })
            }
        }
    }

    /// Remove one item from the alternate equipment set.
    pub fn request_equip_switch_remove(&mut self, item_index: InventoryIndex) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(RequestEquipSwitchRemovePacket::new(item_index))
            }
        }
    }

    pub fn move_item_to_cart(&mut self, item_index: InventoryIndex, amount: i32) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(MoveItemFromBodyToCartPacket::new(item_index, amount))
            }
        }
    }

    pub fn move_item_from_cart(&mut self, item_index: InventoryIndex, amount: i32) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(MoveItemFromCartToBodyPacket::new(item_index, amount))
            }
        }
    }

    pub fn cast_skill(&mut self, skill_id: SkillId, skill_level: SkillLevel, entity_id: EntityId) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(UseSkillAtIdPacket::new(skill_level, skill_id, entity_id))
            }
        }
    }

    pub fn cast_ground_skill(
        &mut self,
        skill_id: SkillId,
        skill_level: SkillLevel,
        target_position: TilePosition,
    ) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(UseSkillOnGroundPacket::new(skill_level, skill_id, target_position))
            }
            SupportedPacketVersion::_20190530 => self.send_map_server_packet(
                ragnarok_packets::version_20190530::UseSkillOnGroundPacket::new(skill_level, skill_id, target_position),
            ),
        }
    }

    pub fn cast_channeling_skill(
        &mut self,
        skill_id: SkillId,
        skill_level: SkillLevel,
        entity_id: EntityId,
    ) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(StartUseSkillPacket::new(skill_id, skill_level, entity_id))
            }
        }
    }

    pub fn stop_channeling_skill(&mut self, skill_id: SkillId) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(EndUseSkillPacket::new(skill_id))
            }
        }
    }

    pub fn add_friend(&mut self, name: String) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(AddFriendPacket::new(name))
            }
        }
    }

    pub fn remove_friend(&mut self, account_id: AccountId, character_id: CharacterId) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(RemoveFriendPacket::new(account_id, character_id))
            }
        }
    }

    pub fn reject_friend_request(&mut self, account_id: AccountId, character_id: CharacterId) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => self
                .send_map_server_packet(FriendRequestResponsePacket::new(
                    account_id,
                    character_id,
                    FriendRequestResponse::Reject,
                )),
        }
    }

    pub fn accept_friend_request(&mut self, account_id: AccountId, character_id: CharacterId) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => self
                .send_map_server_packet(FriendRequestResponsePacket::new(
                    account_id,
                    character_id,
                    FriendRequestResponse::Accept,
                )),
        }
    }

    pub fn reject_party_invite(&mut self, party_id: PartyId) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => self
                .send_map_server_packet(PartyInviteReplyPacket {
                    party_id,
                    response: PartyInviteReply::Reject,
                }),
        }
    }

    pub fn accept_party_invite(&mut self, party_id: PartyId) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => self
                .send_map_server_packet(PartyInviteReplyPacket {
                    party_id,
                    response: PartyInviteReply::Accept,
                }),
        }
    }

    /// Ask for guild info, members, position names, and the notice.
    pub fn request_guild_window(&mut self) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                for menu in [
                    ragnarok_packets::GUILD_MENU_INFO,
                    ragnarok_packets::GUILD_MENU_MEMBERS,
                    ragnarok_packets::GUILD_MENU_POSITIONS,
                    ragnarok_packets::GUILD_MENU_NOTICE,
                ] {
                    self.send_map_server_packet(ragnarok_packets::RequestGuildMenuPacket { menu })?;
                }
                Ok(())
            }
        }
    }

    /// `CZ_GUILD_NOTICE`. Strings are clipped to the 60-byte subject and
    /// 120-byte body.
    pub fn update_guild_notice(&mut self, guild_id: u32, subject: String, notice: String) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => self
                .send_map_server_packet(ragnarok_packets::ChangeGuildNoticePacket {
                    guild_id,
                    subject: clip_guild_text(subject, 60),
                    notice: clip_guild_text(notice, 120),
                }),
        }
    }

    pub fn set_hotkey_data(&mut self, tab: HotbarTab, index: HotbarSlot, hotkey_data: HotkeyData) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(SetHotkeyData2Packet::new(tab, index, hotkey_data))
            }
        }
    }

    pub fn select_buy_or_sell(&mut self, shop_id: ShopId, buy_or_sell: BuyOrSellOption) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(SelectBuyOrSellPacket::new(shop_id, buy_or_sell))
            }
        }
    }

    pub fn purchase_items(&mut self, items: Vec<ShopItem<u32>>) -> Result<(), NotConnectedError> {
        let item_information = items
            .into_iter()
            .map(|item| BuyShopItemInformation {
                item_id: item.item_id,
                amount: item.metadata,
            })
            .collect();

        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(BuyShopItemsPacket::new(item_information))
            }
        }
    }

    pub fn close_shop(&mut self) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(CloseShopPacket::new())
            }
        }
    }

    pub fn sell_items(&mut self, items: Vec<SoldItemInformation>) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(SellItemsPacket { items })
            }
        }
    }

    /// `CZ_REQ_ACH_REWARD` (0x0A25).
    pub fn request_achievement_reward(&mut self, achievement_id: u32) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => self
                .send_map_server_packet(ragnarok_packets::RequestAchievementRewardPacket { achievement_id }),
        }
    }

    /// Claim today's attendance reward (`0x0AEF`). The server answers with `0x0AF0`.
    pub fn request_attendance(&mut self) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(ragnarok_packets::RequestAttendancePacket {})
            }
        }
    }

    pub fn request_stat_up(&mut self, stat_type: StatUpType) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(RequestStatUpPacket::new(stat_type))
            }
        }
    }

    pub fn request_trade(&mut self, partner_account_id: AccountId) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(RequestTradePacket::new(partner_account_id))
            }
        }
    }

    pub fn respond_to_trade(&mut self, accept: bool) -> Result<(), NotConnectedError> {
        let response = if accept { 3 } else { 4 };
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(TradeAckPacket::new(response))
            }
        }
    }

    /// `index` is the stored inventory index. The wire value is `index + 2`.
    /// Pass `None` to offer zeny.
    pub fn add_trade_item(&mut self, index: Option<InventoryIndex>, amount: u32) -> Result<(), NotConnectedError> {
        let index = index.map(|index| index.0.saturating_add(2)).unwrap_or(0);
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(AddTradeItemPacket::new(index, amount))
            }
        }
    }

    pub fn conclude_trade(&mut self) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(ConcludeTradePacket::new())
            }
        }
    }

    pub fn cancel_trade(&mut self) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(CancelTradePacket::new())
            }
        }
    }

    pub fn commit_trade(&mut self) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(CommitTradePacket::new())
            }
        }
    }

    pub fn level_up_skill(&mut self, skill_id: SkillId) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(LevelUpSkillPacket::new(skill_id))
            }
        }
    }

    /// Ask a vending player for the items they are selling.
    pub fn request_vending_list(&mut self, account_id: AccountId) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(RequestVendingListPacket { account_id })
            }
        }
    }

    /// Ask a buying store which items it wants.
    pub fn request_buying_store_list(&mut self, account_id: AccountId) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(RequestBuyingStoreListPacket { account_id })
            }
        }
    }

    /// Buy `amount` of the vend cart slot `index`.
    pub fn buy_from_vending(&mut self, account_id: AccountId, vender_id: u32, index: u16, amount: u16) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => self
                .send_map_server_packet(BuyFromVendingPacket {
                    account_id,
                    vender_id,
                    items: vec![VendingPurchaseInformation { amount, index }],
                }),
        }
    }

    /// Sell one inventory stack into an open buying store.
    pub fn sell_to_buying_store(
        &mut self,
        account_id: AccountId,
        store_id: u32,
        inventory_index: i16,
        item_id: ItemId,
        amount: u16,
    ) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => self
                .send_map_server_packet(SellToBuyingStorePacket {
                    account_id,
                    store_id,
                    items: vec![SellToBuyingStoreInformation {
                        inventory_index,
                        item_id,
                        amount,
                    }],
                }),
        }
    }

    /// Open this client's vend. `items` are `(inventory index, amount, price)`.
    pub fn open_vending_shop(&mut self, shop_name: &str, items: &[(u16, u16, u32)]) -> Result<(), NotConnectedError> {
        let shop_name = shop_name.chars().take(79).collect::<String>();
        let items = items
            .iter()
            .copied()
            .map(|(inventory_index, amount, price)| OpenVendingItemInformation {
                inventory_index,
                amount,
                price: Price(price),
            })
            .collect();
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(OpenVendingShopPacket { shop_name, items })
            }
        }
    }

    pub fn close_vending_shop(&mut self) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(CloseVendingShopPacket {})
            }
        }
    }

    /// Ask the server where a search-store row is standing.
    /// Ask the refine UI which ores apply to this inventory index.
    pub fn refine_select_item(&mut self, index: InventoryIndex) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(RefineSelectItemPacket { index })
            }
        }
    }

    /// Spend one ore on the selected item. `blacksmith_blessing` is sent as 1
    /// when set.
    pub fn refine_item(&mut self, index: InventoryIndex, item_id: ItemId, blacksmith_blessing: bool) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => self
                .send_map_server_packet(RefineRequestPacket {
                    index,
                    item_id,
                    blacksmith_blessing: u8::from(blacksmith_blessing),
                }),
        }
    }

    /// Ask which materials grade this inventory index
    /// (`CZ_GRADE_ENCHANT_SELECT_EQUIPMENT`).
    pub fn grade_enchant_select(&mut self, index: InventoryIndex) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(GradeEnchantSelectPacket { index })
            }
            SupportedPacketVersion::_20190530 => Ok(()),
        }
    }

    /// Spend one listed option on the selected equipment.
    pub fn grade_enchant_request(
        &mut self,
        index: InventoryIndex,
        material_index: i32,
        blessing_flag: u8,
        blessing_amount: i32,
    ) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(GradeEnchantRequestPacket {
                    index,
                    material_index,
                    blessing_flag,
                    blessing_amount,
                    protect_flag: 0,
                })
            }
            SupportedPacketVersion::_20190530 => Ok(()),
        }
    }

    /// Tell the server the grade UI closed.
    pub fn close_grade_enchant(&mut self) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(GradeEnchantClosePacket {})
            }
            SupportedPacketVersion::_20190530 => Ok(()),
        }
    }

    /// Reform `index` with the catalyst that opened the UI.
    pub fn item_reform(&mut self, item_id: ItemId, index: InventoryIndex) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(ItemReformRequestPacket { item_id, index })
            }
            SupportedPacketVersion::_20190530 => Ok(()),
        }
    }

    /// Tell the server the reform UI closed.
    pub fn close_item_reform(&mut self) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20250716 => self.send_map_server_packet(ItemReformClosePacket {}),
            SupportedPacketVersion::_20190530 => Ok(()),
        }
    }

    /// Submit Laphine synthesis. `materials` are stored inventory indexes and
    /// counts.
    pub fn laphine_synthesis(&mut self, item_id: ItemId, materials: &[(u16, u16)]) -> Result<(), NotConnectedError> {
        let materials = materials
            .iter()
            .copied()
            .map(|(index, count)| LaphineSynthesisMaterial {
                index: InventoryIndex(index),
                count,
            })
            .collect();
        self.send_map_server_packet(LaphineSynthesisRequestPacket { item_id, materials })
    }

    pub fn close_laphine_synthesis(&mut self) -> Result<(), NotConnectedError> {
        self.send_map_server_packet(LaphineSynthesisClosePacket {})
    }

    pub fn laphine_upgrade(&mut self, item_id: ItemId, index: InventoryIndex) -> Result<(), NotConnectedError> {
        self.send_map_server_packet(LaphineUpgradeRequestPacket { item_id, index })
    }

    pub fn close_laphine_upgrade(&mut self) -> Result<(), NotConnectedError> {
        self.send_map_server_packet(LaphineUpgradeClosePacket {})
    }

    /// Tell the server the refine UI closed so the character can move again.
    pub fn close_refine(&mut self) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(RefineClosePacket {})
            }
        }
    }

    pub fn select_search_store_item(&mut self, account_id: AccountId, store_id: u32, item_id: ItemId) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => self
                .send_map_server_packet(SearchStoreSelectPacket {
                    account_id,
                    store_id,
                    item_id,
                }),
        }
    }
}

fn clip_guild_text(value: String, max_bytes: usize) -> String {
    let mut clipped = String::new();
    for character in value.chars() {
        let next = clipped.len() + character.len_utf8();
        if next > max_bytes {
            break;
        }
        clipped.push(character);
    }
    clipped
}

#[cfg(test)]
mod packet_handlers {
    use ragnarok_bytes::{ByteReader, ByteWriter};
    use ragnarok_packets::handler::{HandlerResult, NoPacketCallback};
    use ragnarok_packets::{
        DisplayEmotionPacket, EntityId, PacketExt, RequestEmotionPacket, StatusChangePacket, StatusChangeSequencePacket,
    };

    use crate::{NetworkEvent, NetworkingSystem, StatusChangeState, SupportedPacketVersion};

    #[test]
    fn login_server() {
        let result = NetworkingSystem::create_login_server_packet_handler(NoPacketCallback, SupportedPacketVersion::_20220406);
        assert!(result.is_ok());
    }

    #[test]
    fn character_server() {
        let result = NetworkingSystem::create_character_server_packet_handler(NoPacketCallback, SupportedPacketVersion::_20220406);
        assert!(result.is_ok());
    }

    #[test]
    fn map_server() {
        let result = NetworkingSystem::create_map_server_packet_handler(NoPacketCallback, SupportedPacketVersion::_20220406);
        assert!(result.is_ok());
    }

    #[test]
    fn cart_move_packets_are_eight_bytes() {
        let mut body_to_cart = ByteWriter::new();
        ragnarok_packets::MoveItemFromBodyToCartPacket {
            index: ragnarok_packets::InventoryIndex(4),
            amount: 2,
        }
        .packet_to_bytes(&mut body_to_cart)
        .unwrap();
        assert_eq!(body_to_cart.as_slice(), &[0x26, 0x01, 0x06, 0x00, 0x02, 0x00, 0x00, 0x00]);

        let mut cart_to_body = ByteWriter::new();
        ragnarok_packets::MoveItemFromCartToBodyPacket {
            index: ragnarok_packets::InventoryIndex(4),
            amount: 2,
        }
        .packet_to_bytes(&mut cart_to_body)
        .unwrap();
        assert_eq!(cart_to_body.as_slice(), &[0x27, 0x01, 0x06, 0x00, 0x02, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn add_item_to_cart_publishes_cart_not_bag() {
        use ragnarok_packets::version_20250716::AddItemToCartPacket;
        use ragnarok_packets::{ItemId, ItemOptions};

        let packet = AddItemToCartPacket {
            index: ragnarok_packets::InventoryIndex(9),
            amount: 3,
            item_id: ItemId(501),
            item_type: 0,
            identified: 1,
            damaged: 0,
            refine: 0,
            cards: [0; 4],
            option_data: std::array::from_fn(|_| ItemOptions {
                index: 0,
                value: 0,
                parameter: 0,
            }),
        };
        let mut writer = ByteWriter::new();
        packet.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 57);

        let events = handle_map_packet(SupportedPacketVersion::_20250716, packet);
        assert!(matches!(events.0.as_slice(), [NetworkEvent::CartItemAdded { .. }]));
    }

    #[test]
    fn emotion_request_uses_expected_header() {
        let mut writer = ByteWriter::new();
        RequestEmotionPacket { emotion: 3 }.packet_to_bytes(&mut writer).unwrap();

        assert_eq!(writer.as_slice(), &[0xBF, 0x00, 0x03]);
    }

    #[test]
    fn display_emotion_packet_preserves_entity_and_emotion() {
        let mut handler = NetworkingSystem::create_map_server_packet_handler(NoPacketCallback, SupportedPacketVersion::_20220406).unwrap();
        let packet = DisplayEmotionPacket {
            entity_id: EntityId(0x1234_5678),
            emotion: 3,
        };
        let mut writer = ByteWriter::new();
        packet.packet_to_bytes(&mut writer).unwrap();
        let mut reader = ByteReader::without_metadata(writer.as_slice());

        let HandlerResult::Ok(events) = handler.process_one(&mut reader) else {
            panic!("display-emotion packet was not handled");
        };

        assert!(matches!(events.0.as_slice(), [NetworkEvent::DisplayEmotion {
            entity_id: EntityId(0x1234_5678),
            emotion: 3,
        }]));
    }

    #[test]
    fn nav_marks_packets_become_typed_events_on_every_profile() {
        use ragnarok_packets::{
            ColorRGBA, EntityStopMovePacket, LargeTilePosition, MarkMinimapPositionPacket, MarkerType, NavigateToMonsterPacket,
            TilePosition,
        };

        let navigate = NavigateToMonsterPacket {
            target_type: 3,
            flags: 0b101,
            hide_window: 0,
            map_name: "prontera.gat".to_string(),
            target_position: TilePosition { x: 150, y: 180 },
            target_monster_id: 1002,
        };
        let mark = MarkMinimapPositionPacket {
            npc_id: EntityId(42),
            marker_type: MarkerType::DisplayFor15Seconds,
            position: LargeTilePosition { x: 20, y: 30 },
            id: 1,
            // Little-endian image of script color 0x00FF0000 (red).
            color: ColorRGBA {
                red: 0x00,
                green: 0x00,
                blue: 0xFF,
                alpha: 0x00,
            },
        };

        for version in [
            SupportedPacketVersion::_20250716,
            SupportedPacketVersion::_20220406,
            SupportedPacketVersion::_20190530,
        ] {
            let events = handle_map_packet(version, navigate.clone());
            assert!(matches!(events.0.as_slice(), [NetworkEvent::NavigateTo {
                target_type: 3,
                service_flags: 0b101,
                show_window: true,
                map_name,
                x: 150,
                y: 180,
                monster_id: 1002,
            }] if map_name == "prontera.gat"));

            let events = handle_map_packet(version, mark.clone());
            assert!(matches!(events.0.as_slice(), [NetworkEvent::MinimapMark {
                npc_id: EntityId(42),
                kind: 0,
                x: 20,
                y: 30,
                slot: 1,
                red: 255,
                green: 0,
                blue: 0,
            }]));

            let stop = EntityStopMovePacket {
                entity_id: EntityId(7),
                position: TilePosition { x: 11, y: 22 },
            };
            let events = handle_map_packet(version, stop);
            assert!(matches!(events.0.as_slice(), [NetworkEvent::EntityStopMove {
                entity_id: EntityId(7),
                position: TilePosition { x: 11, y: 22 },
            }]));
        }

        let hidden = NavigateToMonsterPacket {
            hide_window: 1,
            ..navigate
        };
        let events = handle_map_packet(SupportedPacketVersion::_20250716, hidden);
        assert!(matches!(events.0.as_slice(), [NetworkEvent::NavigateTo {
            show_window: false,
            ..
        }]));

        let remove = MarkMinimapPositionPacket {
            marker_type: MarkerType::RemoveMark,
            id: 1,
            ..mark
        };
        let events = handle_map_packet(SupportedPacketVersion::_20250716, remove);
        assert!(matches!(events.0.as_slice(), [NetworkEvent::MinimapMark {
            kind: 2,
            slot: 1,
            ..
        }]));
    }

    fn handle_one(
        version: crate::SupportedPacketVersion,
        bytes: &[u8],
    ) -> ragnarok_packets::handler::HandlerResult<crate::event::NetworkEventList> {
        let mut handler = NetworkingSystem::create_map_server_packet_handler(NoPacketCallback, version).unwrap();
        let mut reader = ByteReader::without_metadata(bytes);
        handler.process_one(&mut reader)
    }

    fn handle_map_packet<Packet: PacketExt>(version: SupportedPacketVersion, packet: Packet) -> crate::event::NetworkEventList {
        let mut handler = NetworkingSystem::create_map_server_packet_handler(NoPacketCallback, version).unwrap();
        let mut writer = ByteWriter::new();
        packet.packet_to_bytes(&mut writer).unwrap();
        let mut reader = ByteReader::without_metadata(writer.as_slice());

        let HandlerResult::Ok(events) = handler.process_one(&mut reader) else {
            panic!("packet was not handled");
        };
        events
    }

    #[test]
    fn packetver_20250716_skill_packets_become_typed_events() {
        use ragnarok_packets::{
            ClientTick, DisplaySkillCooldownPacket, ItemId, NotifyGroundSkillPacket, SkillId, SkillLevel, TilePosition,
            ToUseSkillSuccessPacket, UseSkillSuccessPacket,
        };

        let cooldown = DisplaySkillCooldownPacket {
            skill_id: SkillId(46),
            until: ClientTick(1_500),
        };
        let mut writer = ByteWriter::new();
        cooldown.packet_to_bytes(&mut writer).unwrap();
        let HandlerResult::Ok(events) = handle_one(SupportedPacketVersion::_20250716, writer.as_slice()) else {
            panic!("cooldown packet was not handled");
        };
        assert!(matches!(events.0.as_slice(), [NetworkEvent::SkillCooldown {
            skill_id: SkillId(46),
            until: ClientTick(1_500),
        }]));

        let use_skill = UseSkillSuccessPacket {
            source_entity: EntityId(11),
            destination_entity: EntityId(22),
            position: TilePosition { x: 3, y: 4 },
            skill_id: SkillId(28),
            element: 6,
            delay_time: 90,
            disposable: 1,
        };
        let mut writer = ByteWriter::new();
        use_skill.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 25);
        let HandlerResult::Ok(events) = handle_one(SupportedPacketVersion::_20250716, writer.as_slice()) else {
            panic!("use-skill packet was not handled");
        };
        assert!(matches!(events.0.as_slice(), [NetworkEvent::UseSkillSuccess {
            source_entity_id: EntityId(11),
            destination_entity_id: EntityId(22),
            position: TilePosition { x: 3, y: 4 },
            skill_id: SkillId(28),
            element: 6,
            delay_time: 90,
            disposable: 1,
        }]));

        let to_use = ToUseSkillSuccessPacket {
            skill_id: SkillId(7),
            btype: -2,
            item_id: ItemId(501),
            flag: 1,
            cause: 0,
        };
        let mut writer = ByteWriter::new();
        to_use.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 14);
        let HandlerResult::Ok(events) = handle_one(SupportedPacketVersion::_20250716, writer.as_slice()) else {
            panic!("to-use-skill packet was not handled");
        };
        assert!(matches!(events.0.as_slice(), [NetworkEvent::ToUseSkillSuccess {
            skill_id: SkillId(7),
            btype: -2,
            item_id: ItemId(501),
            flag: 1,
            cause: 0,
        }]));

        let ground = NotifyGroundSkillPacket {
            skill_id: SkillId(89),
            entity_id: EntityId(77),
            level: SkillLevel(5),
            position: TilePosition { x: 10, y: 20 },
            start_time: ClientTick(42),
        };
        let mut writer = ByteWriter::new();
        ground.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 18);
        let HandlerResult::Ok(events) = handle_one(SupportedPacketVersion::_20250716, writer.as_slice()) else {
            panic!("ground-skill packet was not handled");
        };
        assert!(matches!(events.0.as_slice(), [NetworkEvent::NotifyGroundSkill {
            skill_id: SkillId(89),
            entity_id: EntityId(77),
            level: SkillLevel(5),
            position: TilePosition { x: 10, y: 20 },
            start_time: ClientTick(42),
        }]));
    }

    #[test]
    fn packetver_20250716_combat_feedback_packets_become_typed_events() {
        use ragnarok_packets::{
            AccountId, ClientTick, DisplayGainedExperiencePacket, DisplayPlayerHealEffect, DisplaySkillEffectAndDamagePacket,
            DisplaySpecialEffectPacket, EffectId, ExperienceSource, ExperienceType, HealType, SkillId, SkillLevel,
        };

        let special = handle_map_packet(SupportedPacketVersion::_20250716, DisplaySpecialEffectPacket {
            entity_id: EntityId(9),
            effect_id: EffectId::Heal,
        });
        assert!(matches!(special.0.as_slice(), [NetworkEvent::SpecialEffect {
            entity_id: EntityId(9),
            effect_id: EffectId::Heal,
        }]));

        let skill = handle_map_packet(SupportedPacketVersion::_20250716, DisplaySkillEffectAndDamagePacket {
            skill_id: SkillId(28),
            source_entity_id: EntityId(1),
            destination_entity_id: EntityId(2),
            start_time: ClientTick(40),
            soruce_delay: 10,
            destination_delay: 20,
            damage: 150,
            level: SkillLevel(3),
            div: 2,
            skill_type: 1,
        });
        assert!(matches!(skill.0.as_slice(), [NetworkEvent::SkillEffectAndDamage {
            skill_id: SkillId(28),
            source_entity_id: EntityId(1),
            destination_entity_id: EntityId(2),
            start_time: ClientTick(40),
            source_delay: 10,
            destination_delay: 20,
            damage: 150,
            level: SkillLevel(3),
            hit_count: 2,
            skill_type: 1,
        }]));

        let heal = handle_map_packet(SupportedPacketVersion::_20250716, DisplayPlayerHealEffect {
            heal_type: HealType::SpellPoints,
            heal_amount: 40,
        });
        assert!(matches!(heal.0.as_slice(), [NetworkEvent::PlayerHealEffect {
            heal_type: HealType::SpellPoints,
            heal_amount: 40,
        }]));

        let experience = handle_map_packet(SupportedPacketVersion::_20250716, DisplayGainedExperiencePacket {
            account_id: AccountId(7),
            amount: 1_250,
            experience_type: ExperienceType::JobExperience,
            experience_source: ExperienceSource::Quest,
        });
        assert!(matches!(experience.0.as_slice(), [NetworkEvent::GainedExperience {
            account_id: AccountId(7),
            amount: 1_250,
            experience_type: ExperienceType::JobExperience,
            experience_source: ExperienceSource::Quest,
        }]));
    }

    #[test]
    fn older_packetvers_still_drop_combat_feedback_packets() {
        use ragnarok_packets::{AccountId, DisplayGainedExperiencePacket, ExperienceSource, ExperienceType};

        let mut writer = ByteWriter::new();
        DisplayGainedExperiencePacket {
            account_id: AccountId(1),
            amount: 10,
            experience_type: ExperienceType::BaseExperience,
            experience_source: ExperienceSource::Regular,
        }
        .packet_to_bytes(&mut writer)
        .unwrap();

        for version in [SupportedPacketVersion::_20220406, SupportedPacketVersion::_20190530] {
            let HandlerResult::Ok(events) = handle_one(version, writer.as_slice()) else {
                panic!("older packetver should still accept the experience packet");
            };
            assert!(events.0.is_empty(), "{version:?} should keep the packet as a noop");
        }
    }

    #[test]
    fn older_packetvers_still_drop_skill_ack_packets() {
        use ragnarok_packets::DisplaySkillCooldownPacket;

        let mut writer = ByteWriter::new();
        DisplaySkillCooldownPacket {
            skill_id: ragnarok_packets::SkillId(1),
            until: ragnarok_packets::ClientTick(2),
        }
        .packet_to_bytes(&mut writer)
        .unwrap();

        for version in [SupportedPacketVersion::_20220406, SupportedPacketVersion::_20190530] {
            let HandlerResult::Ok(events) = handle_one(version, writer.as_slice()) else {
                panic!("older packetver should still accept the cooldown packet");
            };
            assert!(events.0.is_empty(), "{version:?} should keep the packet as a noop");
        }
    }

    #[test]
    fn packetver_20250716_status_change_emits_typed_event() {
        let events = handle_map_packet(SupportedPacketVersion::_20250716, StatusChangePacket {
            index: 12,
            entity_id: EntityId(0x0102_0304),
            state: 1,
            duration_in_milliseconds: 30_000,
            remaining_in_milliseconds: 12_500,
            value: [7, 8, 9],
        });

        assert!(matches!(events.0.as_slice(), [NetworkEvent::StatusChange {
            status_index: 12,
            entity_id: EntityId(0x0102_0304),
            state: StatusChangeState::Active,
            total_duration_milliseconds: 30_000,
            remaining_milliseconds: 12_500,
            values: [7, 8, 9],
        }]));
    }

    #[test]
    fn packetver_20250716_status_change_preserves_end_and_unknown_state() {
        let ended = handle_map_packet(SupportedPacketVersion::_20250716, StatusChangePacket {
            index: 1,
            entity_id: EntityId(2),
            state: 0,
            duration_in_milliseconds: 0,
            remaining_in_milliseconds: 0,
            value: [0, 0, 0],
        });
        assert!(matches!(ended.0.as_slice(), [NetworkEvent::StatusChange {
            state: StatusChangeState::Ended,
            ..
        }]));

        let unknown = handle_map_packet(SupportedPacketVersion::_20250716, StatusChangePacket {
            index: 1,
            entity_id: EntityId(2),
            state: 9,
            duration_in_milliseconds: 0,
            remaining_in_milliseconds: 0,
            value: [0, 0, 0],
        });
        assert!(matches!(unknown.0.as_slice(), [NetworkEvent::StatusChange {
            state: StatusChangeState::Other(9),
            ..
        }]));
    }

    #[test]
    fn packetver_20250716_status_change_sequence_emits_typed_event() {
        let events = handle_map_packet(SupportedPacketVersion::_20250716, StatusChangeSequencePacket {
            index: 44,
            id: 0xAABB_CCDD,
            state: 2,
        });

        assert!(matches!(events.0.as_slice(), [NetworkEvent::StatusChangeSequence {
            status_index: 44,
            entity_id: EntityId(0xAABB_CCDD),
            state: StatusChangeState::StillActive,
        }]));
    }

    #[test]
    fn older_packetvers_keep_status_change_noop() {
        for version in [SupportedPacketVersion::_20190530, SupportedPacketVersion::_20220406] {
            let timed = handle_map_packet(version, StatusChangePacket {
                index: 12,
                entity_id: EntityId(1),
                state: 1,
                duration_in_milliseconds: 1,
                remaining_in_milliseconds: 1,
                value: [1, 2, 3],
            });
            assert!(timed.0.is_empty(), "{version:?} must keep StatusChangePacket as a noop");

            let sequence = handle_map_packet(version, StatusChangeSequencePacket { index: 3, id: 4, state: 0 });
            assert!(
                sequence.0.is_empty(),
                "{version:?} must keep StatusChangeSequencePacket as a noop"
            );
        }
    }

    fn map_events(version: SupportedPacketVersion, bytes: &[u8]) -> Vec<NetworkEvent> {
        let mut handler = NetworkingSystem::create_map_server_packet_handler(NoPacketCallback, version).unwrap();
        let mut reader = ByteReader::without_metadata(bytes);
        let mut events = Vec::new();
        while !reader.is_empty() {
            match handler.process_one(&mut reader) {
                HandlerResult::Ok(list) => events.extend(list.0),
                HandlerResult::UnhandledPacket => panic!("quest packet was not handled"),
                HandlerResult::PacketCutOff => panic!("quest packet was cut off"),
                HandlerResult::InternalError(error) => panic!("quest packet failed: {error:?}"),
            }
        }
        events
    }

    fn sample_objective(name: &str, mob_count: u16) -> ragnarok_packets::ObjectiveDetails1 {
        ragnarok_packets::ObjectiveDetails1 {
            hunt_identification: 7,
            objective_type: 1,
            mob_id: 1002,
            minimum_level: 1,
            maximum_level: 99,
            mob_count,
            mob_name: name.to_owned(),
        }
    }

    fn quest_packet_bytes() -> Vec<u8> {
        let mut writer = ByteWriter::new();
        ragnarok_packets::QuestListPacket {
            quest_count: 1,
            quests: vec![ragnarok_packets::Quest {
                quest_id: 501,
                active: 1,
                remaining_time: 30,
                expire_time: 90,
                objective_count: 1,
                objective_details: vec![ragnarok_packets::QuestDetails {
                    hunt_identification: 4,
                    objective_type: 1,
                    mob_id: 1002,
                    minimum_level: 1,
                    maximum_level: 10,
                    kill_count: 2,
                    total_count: 5,
                    mob_name: "Poring".to_owned(),
                }],
            }],
        }
        .packet_to_bytes(&mut writer)
        .unwrap();

        let mut objectives = [sample_objective("Poring", 5), sample_objective("", 0), sample_objective("", 0)];
        objectives[0].mob_name = "Poring".to_owned();
        ragnarok_packets::QuestNotificationPacket1 {
            quest_id: 501,
            active: 1,
            start_time: 11,
            expire_time: 90,
            objective_count: 1,
            objective_details: objectives,
        }
        .packet_to_bytes(&mut writer)
        .unwrap();

        ragnarok_packets::HuntingQuestNotificationPacket {
            objective_details: vec![ragnarok_packets::HuntingObjective {
                quest_id: 501,
                mob_id: 1002,
                total_count: 5,
                current_count: 1,
            }],
        }
        .packet_to_bytes(&mut writer)
        .unwrap();

        ragnarok_packets::HuntingQuestUpdateObjectivePacket {
            objective_count: 1,
            objective_details: vec![ragnarok_packets::HuntingObjective {
                quest_id: 501,
                mob_id: 1002,
                total_count: 5,
                current_count: 3,
            }],
        }
        .packet_to_bytes(&mut writer)
        .unwrap();

        ragnarok_packets::QuestRemovedPacket { quest_id: 501 }
            .packet_to_bytes(&mut writer)
            .unwrap();

        writer.into_inner()
    }

    #[test]
    fn quest_packets_20250716_emit_typed_events() {
        let events = map_events(SupportedPacketVersion::_20250716, &quest_packet_bytes());
        assert_eq!(events.len(), 5);

        match &events[0] {
            NetworkEvent::QuestList { quests } => {
                assert_eq!(quests.len(), 1);
                assert_eq!(quests[0].quest_id, 501);
                assert_eq!(quests[0].active, 1);
                assert_eq!(quests[0].objective_details.len(), 1);
                assert_eq!(quests[0].objective_details[0].mob_name, "Poring");
                assert_eq!(quests[0].objective_details[0].kill_count, 2);
                assert_eq!(quests[0].objective_details[0].total_count, 5);
            }
            other => panic!("expected quest list, got {other:?}"),
        }
        match &events[1] {
            NetworkEvent::QuestNotification {
                quest_id,
                active,
                start_time,
                expire_time,
                objectives,
            } => {
                assert_eq!(*quest_id, 501);
                assert!(*active);
                assert_eq!(*start_time, 11);
                assert_eq!(*expire_time, 90);
                assert_eq!(objectives.len(), 1);
                assert_eq!(objectives[0].mob_id, 1002);
                assert_eq!(objectives[0].mob_count, 5);
                assert_eq!(objectives[0].mob_name, "Poring");
            }
            other => panic!("expected quest notification, got {other:?}"),
        }
        match &events[2] {
            NetworkEvent::HuntingQuestNotification { objectives } => {
                assert_eq!(objectives.len(), 1);
                assert_eq!(objectives[0].current_count, 1);
                assert_eq!(objectives[0].total_count, 5);
            }
            other => panic!("expected hunting notification, got {other:?}"),
        }
        match &events[3] {
            NetworkEvent::HuntingQuestObjectiveUpdate { objectives } => {
                assert_eq!(objectives.len(), 1);
                assert_eq!(objectives[0].quest_id, 501);
                assert_eq!(objectives[0].current_count, 3);
            }
            other => panic!("expected hunting update, got {other:?}"),
        }
        assert!(matches!(events[4], NetworkEvent::QuestRemoved { quest_id: 501 }));
    }

    #[test]
    fn quest_packets_stay_noop_on_older_profiles() {
        for version in [SupportedPacketVersion::_20190530, SupportedPacketVersion::_20220406] {
            let events = map_events(version, &quest_packet_bytes());
            assert!(events.is_empty(), "{version:?} should not emit quest events");
        }
    }

    #[test]
    fn inactive_quest_notification_drops_padding_objectives() {
        let mut writer = ByteWriter::new();
        ragnarok_packets::QuestNotificationPacket1 {
            quest_id: 9,
            active: 0,
            start_time: 0,
            expire_time: 0,
            objective_count: 0,
            objective_details: [sample_objective("Pad", 9), sample_objective("Pad", 9), sample_objective("Pad", 9)],
        }
        .packet_to_bytes(&mut writer)
        .unwrap();

        let events = map_events(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(
            events.as_slice(),
            [NetworkEvent::QuestNotification {
                quest_id: 9,
                active: false,
                objectives,
                ..
            }] if objectives.is_empty()
        ));
    }

    fn achievement_packet_bytes() -> Vec<u8> {
        let mut writer = ByteWriter::new();
        let entry = ragnarok_packets::AchievementData {
            acheivement_id: 12,
            is_completed: 1,
            objectives: [3, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            completion_timestamp: 40,
            got_rewarded: 0,
        };
        ragnarok_packets::AchievementListPacket {
            achievement_count: 1,
            total_score: 10,
            level: 2,
            acheivement_experience: 4,
            acheivement_experience_to_next_level: 20,
            acheivement_data: vec![entry.clone()],
        }
        .packet_to_bytes(&mut writer)
        .unwrap();
        ragnarok_packets::AchievementUpdatePacket {
            total_score: 15,
            level: 2,
            acheivement_experience: 9,
            acheivement_experience_to_next_level: 20,
            acheivement_data: entry,
        }
        .packet_to_bytes(&mut writer)
        .unwrap();
        ragnarok_packets::AchievementRewardAckPacket {
            failed: 0,
            achievement_id: 12,
        }
        .packet_to_bytes(&mut writer)
        .unwrap();
        ragnarok_packets::AttendanceDataPacket {
            start_time: 100,
            end_time: 200,
            attendance_count: 3,
            rewards: vec![ragnarok_packets::AttendanceReward { item_id: 501, amount: 10 }],
        }
        .packet_to_bytes(&mut writer)
        .unwrap();
        ragnarok_packets::AttendanceRewardAckPacket { unused: 0, result: 4 }
            .packet_to_bytes(&mut writer)
            .unwrap();
        writer.into_inner()
    }

    #[test]
    fn achievement_and_attendance_packets_round_trip() {
        let bytes = achievement_packet_bytes();
        assert_eq!(&bytes[0..2], &0x0A23u16.to_le_bytes());
        let events = map_events(SupportedPacketVersion::_20250716, &bytes);
        assert!(matches!(
            events.first(),
            Some(NetworkEvent::AchievementList {
                total_score: 10,
                level: 2,
                entries,
                ..
            }) if entries.len() == 1 && entries[0].acheivement_id == 12 && entries[0].got_rewarded == 0
        ));
        assert!(matches!(
            events.get(1),
            Some(NetworkEvent::AchievementUpdate { total_score: 15, entry, .. }) if entry.acheivement_id == 12
        ));
        assert!(matches!(
            events.get(2),
            Some(NetworkEvent::AchievementReward {
                failed: false,
                achievement_id: 12
            })
        ));
        assert!(matches!(
            events.get(3),
            Some(NetworkEvent::Attendance {
                start_time: 100,
                end_time: 200,
                count: 3,
                rewards,
            }) if rewards.len() == 1 && rewards[0].item_id == 501 && rewards[0].amount == 10
        ));
        assert!(matches!(events.get(4), Some(NetworkEvent::AttendanceClaim { result: 4 })));
        assert_eq!(events.len(), 5);

        let mut request = ByteWriter::new();
        ragnarok_packets::RequestAchievementRewardPacket { achievement_id: 12 }
            .packet_to_bytes(&mut request)
            .unwrap();
        ragnarok_packets::RequestAttendancePacket {}
            .packet_to_bytes(&mut request)
            .unwrap();
        let request = request.into_inner();
        assert_eq!(request, [0x25, 0x0A, 12, 0, 0, 0, 0xEF, 0x0A]);
    }

    fn map_handler(
        version: SupportedPacketVersion,
    ) -> ragnarok_packets::handler::PacketHandler<crate::event::NetworkEventList, NoPacketCallback> {
        NetworkingSystem::create_map_server_packet_handler(NoPacketCallback, version).unwrap()
    }

    fn process_one(version: SupportedPacketVersion, bytes: &[u8]) -> Vec<NetworkEvent> {
        let mut handler = map_handler(version);
        let mut reader = ByteReader::without_metadata(bytes);
        let HandlerResult::Ok(events) = handler.process_one(&mut reader) else {
            panic!("packet was not handled");
        };
        assert!(reader.remaining_bytes().is_empty());
        events.0
    }

    #[test]
    fn packetver_20250716_friend_online_status_is_typed() {
        use ragnarok_packets::{AccountId, CharacterId, FriendOnlineStatusPacket, OnlineState};

        for (state, online) in [(OnlineState::Online, true), (OnlineState::Offline, false)] {
            let packet = FriendOnlineStatusPacket {
                account_id: AccountId(7),
                character_id: CharacterId(9),
                state,
                name: "Nova".to_owned(),
            };
            let mut writer = ByteWriter::new();
            packet.packet_to_bytes(&mut writer).unwrap();

            let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
            assert!(matches!(events.as_slice(), [NetworkEvent::FriendOnlineStatus {
                account_id: AccountId(7),
                character_id: CharacterId(9),
                online: got,
                name,
            }] if *got == online && name == "Nova"));

            let older = process_one(SupportedPacketVersion::_20220406, writer.as_slice());
            assert!(older.is_empty());
        }
    }

    #[test]
    fn packetver_20250716_party_invite_is_typed() {
        use ragnarok_packets::{PartyId, PartyInvitePacket};

        let packet = PartyInvitePacket {
            party_id: PartyId(42),
            party_name: "Wolves".to_owned(),
        };
        let mut writer = ByteWriter::new();
        packet.packet_to_bytes(&mut writer).unwrap();

        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::PartyInvite {
            party_id: PartyId(42),
            party_name,
        }] if party_name == "Wolves"));

        assert!(process_one(SupportedPacketVersion::_20190530, writer.as_slice()).is_empty());
    }

    #[test]
    fn party_invite_reply_packet_is_seven_bytes() {
        use ragnarok_packets::{PacketExt, PartyId, PartyInviteReply, PartyInviteReplyPacket};

        let mut writer = ByteWriter::new();
        PartyInviteReplyPacket {
            party_id: PartyId(42),
            response: PartyInviteReply::Accept,
        }
        .packet_to_bytes(&mut writer)
        .unwrap();
        let bytes = writer.as_slice();
        assert_eq!(bytes.len(), 7);
        assert_eq!(u16::from_le_bytes([bytes[0], bytes[1]]), 0x02C7);
        assert_eq!(u32::from_le_bytes(bytes[2..6].try_into().unwrap()), 42);
        assert_eq!(bytes[6], 1);
    }

    #[test]
    fn packetver_20250716_party_invitation_state_is_typed() {
        use ragnarok_packets::UpdatePartyInvitationStatePacket;

        // Refuse flag: 0 allows invitations, 1 auto-denies them.
        let allowed = UpdatePartyInvitationStatePacket { allowed: 0 };
        let blocked = UpdatePartyInvitationStatePacket { allowed: 1 };

        let mut writer = ByteWriter::new();
        allowed.packet_to_bytes(&mut writer).unwrap();
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::PartyInvitationState {
            invitations_allowed: true
        }]));

        let mut writer = ByteWriter::new();
        blocked.packet_to_bytes(&mut writer).unwrap();
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::PartyInvitationState {
            invitations_allowed: false
        }]));
        assert!(process_one(SupportedPacketVersion::_20220406, writer.as_slice()).is_empty());
    }

    #[test]
    fn packetver_20250716_guild_window_packets_are_typed() {
        use ragnarok_packets::{
            AccountId, ChangeGuildNoticePacket, CharacterId, GUILD_MENU_NOTICE, GuildAffiliationPacket, GuildExpulsionPacket,
            GuildInfoPacket, GuildLeavePacket, GuildMemberInformation, GuildMemberListPacket, GuildMemberOnlinePacket, GuildNoticePacket,
            GuildPositionName, GuildPositionNamesPacket, PacketExt, RequestGuildMenuPacket,
        };

        let info = GuildInfoPacket {
            guild_id: 9,
            level: 3,
            online_members: 2,
            max_members: 16,
            average_level: 40,
            experience: 100,
            next_experience: 500,
            tax_points: 0,
            honor: 0,
            virtue: 0,
            emblem_version: 4,
            name: "Wolves".to_owned(),
            territory: "Prontera".to_owned(),
            zeny: 0,
            master_character_id: 77,
            master_name: "Alpha".to_owned(),
        };
        let mut writer = ByteWriter::new();
        info.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 118);
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::GuildInfo {
            guild_id: 9,
            name,
            master_name,
            territory,
            level: 3,
            ..
        }] if name == "Wolves" && master_name == "Alpha" && territory == "Prontera"));

        let members = GuildMemberListPacket {
            members: vec![GuildMemberInformation {
                account_id: AccountId(3),
                character_id: CharacterId(77),
                hair: 1,
                hair_color: 2,
                sex: 0,
                job: 7,
                level: 55,
                contribution_exp: 12,
                current_state: 1,
                position_id: 0,
                last_login_time: 0,
                name: "Alpha".to_owned(),
            }],
        };
        let mut writer = ByteWriter::new();
        members.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 62);
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::GuildMembers { members }]
            if members.len() == 1 && members[0].name == "Alpha" && members[0].online && members[0].level == 55));

        let positions = GuildPositionNamesPacket {
            positions: vec![GuildPositionName {
                position_id: 0,
                name: "Master".to_owned(),
            }],
        };
        let mut writer = ByteWriter::new();
        positions.packet_to_bytes(&mut writer).unwrap();
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::GuildPositions { positions }]
            if positions == &[(0, "Master".to_owned())]));

        let notice = GuildNoticePacket {
            subject: "Meet".to_owned(),
            notice: "Castle at 8".to_owned(),
        };
        let mut writer = ByteWriter::new();
        notice.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 182);
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::GuildNotice { subject, notice }]
            if subject == "Meet" && notice == "Castle at 8"));

        let affiliation = GuildAffiliationPacket {
            guild_id: 9,
            emblem_version: 4,
            mode: 1,
            is_master: 1,
            inter_sid: 0,
            guild_name: "Wolves".to_owned(),
            master_character_id: 77,
        };
        let mut writer = ByteWriter::new();
        affiliation.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 47);
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::GuildAffiliation {
            guild_id: 9,
            is_master: true,
            name,
            ..
        }] if name == "Wolves"));

        let online = GuildMemberOnlinePacket {
            account_id: AccountId(3),
            character_id: CharacterId(77),
            status: 0,
            gender: 1,
            hair: 1,
            hair_color: 2,
        };
        let mut writer = ByteWriter::new();
        online.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 20);
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::GuildMemberOnline {
            online: false,
            ..
        }]));

        let leave = GuildLeavePacket {
            character_id: CharacterId(77),
            reason: "bye".to_owned(),
        };
        let mut writer = ByteWriter::new();
        leave.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 46);
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::GuildMemberRemoved {
            character_id: CharacterId(77),
        }]));

        let expulsion = GuildExpulsionPacket {
            reason: "out".to_owned(),
            character_id: CharacterId(8),
        };
        let mut writer = ByteWriter::new();
        expulsion.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 46);
        assert_eq!(u32::from_le_bytes(writer.as_slice()[42..46].try_into().unwrap()), 8);

        let mut writer = ByteWriter::new();
        RequestGuildMenuPacket { menu: GUILD_MENU_NOTICE }
            .packet_to_bytes(&mut writer)
            .unwrap();
        assert_eq!(writer.as_slice().len(), 6);
        assert_eq!(u16::from_le_bytes([writer.as_slice()[0], writer.as_slice()[1]]), 0x014F);

        let mut writer = ByteWriter::new();
        ChangeGuildNoticePacket {
            guild_id: 9,
            subject: "A".to_owned(),
            notice: "B".to_owned(),
        }
        .packet_to_bytes(&mut writer)
        .unwrap();
        assert_eq!(writer.as_slice().len(), 186);
        assert_eq!(u16::from_le_bytes([writer.as_slice()[0], writer.as_slice()[1]]), 0x016E);
    }

    #[test]
    fn clan_info_and_online_packets_are_typed() {
        use ragnarok_packets::{Aliance, Antagonist, ClanInfoPacket, ClanOnlineCountPacket, PacketExt};

        let info = ClanInfoPacket {
            clan_id: 3,
            clan_name: "Swords".to_owned(),
            clan_master: "Lead".to_owned(),
            clan_map: "prontera".to_owned(),
            aliance_count: 1,
            antagonist_count: 1,
            aliances: vec![Aliance {
                name: "Archers".to_owned(),
            }],
            antagonists: vec![Antagonist {
                name: "Axes".to_owned(),
            }],
        };
        let mut writer = ByteWriter::new();
        info.packet_to_bytes(&mut writer).unwrap();
        for version in [
            SupportedPacketVersion::_20190530,
            SupportedPacketVersion::_20220406,
            SupportedPacketVersion::_20250716,
        ] {
            let events = process_one(version, writer.as_slice());
            assert!(matches!(events.as_slice(), [NetworkEvent::ClanInfo {
                clan_id: 3,
                name,
                master_name,
                map_name,
                alliances,
                antagonists,
            }] if name == "Swords"
                && master_name == "Lead"
                && map_name == "prontera"
                && alliances == &["Archers".to_owned()]
                && antagonists == &["Axes".to_owned()]));
        }

        let online = ClanOnlineCountPacket {
            online_members: 4,
            maximum_members: 20,
        };
        let mut writer = ByteWriter::new();
        online.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 6);
        assert_eq!(u16::from_le_bytes([writer.as_slice()[0], writer.as_slice()[1]]), 0x0988);
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::ClanOnlineCount {
            online_members: 4,
            maximum_members: 20,
        }]));

        let leave = [0x89u8, 0x09];
        for version in [
            SupportedPacketVersion::_20190530,
            SupportedPacketVersion::_20220406,
            SupportedPacketVersion::_20250716,
        ] {
            let events = process_one(version, &leave);
            assert!(matches!(events.as_slice(), [NetworkEvent::ClanLeft]));
        }
    }

    fn zero_trade_option() -> ragnarok_packets::ItemOptions {
        ragnarok_packets::ItemOptions {
            index: 0,
            value: 0,
            parameter: 0,
        }
    }

    #[test]
    fn enchant_ui_packets_round_trip_and_keep_the_group_id() {
        use ragnarok_packets::{
            CloseUiEnchantPacket, ENCHANT_UI_TYPE, EnchantResponsePacket, ItemId, RequestPerfectEnchantPacket, RequestRandomEnchantPacket,
            RequestResetEnchantPacket, RequestUpgradeEnchantPacket, UiOpenV3Packet,
        };

        let group = i64::from(u32::MAX) + 9;

        let open = UiOpenV3Packet {
            ui_type: ENCHANT_UI_TYPE,
            data: group,
        };
        let mut writer = ByteWriter::new();
        open.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 11);

        for version in [
            SupportedPacketVersion::_20250716,
            SupportedPacketVersion::_20220406,
            SupportedPacketVersion::_20190530,
        ] {
            let events = process_one(version, writer.as_slice());
            assert!(matches!(events.as_slice(), [NetworkEvent::OpenEnchant { enchant_group }] if *enchant_group == group));
        }

        let other_ui = UiOpenV3Packet { ui_type: 0, data: group };
        let mut writer = ByteWriter::new();
        other_ui.packet_to_bytes(&mut writer).unwrap();
        assert!(process_one(SupportedPacketVersion::_20250716, writer.as_slice()).is_empty());

        let result = EnchantResponsePacket {
            message_id: 3857,
            item_id: ItemId(4001),
        };
        let mut writer = ByteWriter::new();
        result.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 10);
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::EnchantResult {
            message_id: 3857,
            item_id: ItemId(4001)
        }]));

        let mut writer = ByteWriter::new();
        CloseUiEnchantPacket::new().packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice(), &[0xA0, 0x0B]);

        let mut writer = ByteWriter::new();
        RequestRandomEnchantPacket {
            enchant_group: group,
            inventory_index: 7,
        }
        .packet_to_bytes(&mut writer)
        .unwrap();
        assert_eq!(writer.as_slice().len(), 12);
        assert_eq!(&writer.as_slice()[2..10], &group.to_le_bytes());

        let mut writer = ByteWriter::new();
        RequestPerfectEnchantPacket {
            enchant_group: group,
            inventory_index: 7,
            item_id: ItemId(501),
        }
        .packet_to_bytes(&mut writer)
        .unwrap();
        assert_eq!(writer.as_slice().len(), 16);

        let mut writer = ByteWriter::new();
        RequestUpgradeEnchantPacket {
            enchant_group: group,
            inventory_index: 7,
            slot: 2,
        }
        .packet_to_bytes(&mut writer)
        .unwrap();
        assert_eq!(writer.as_slice().len(), 14);

        let mut writer = ByteWriter::new();
        RequestResetEnchantPacket {
            enchant_group: group,
            inventory_index: -1,
        }
        .packet_to_bytes(&mut writer)
        .unwrap();
        assert_eq!(writer.as_slice().len(), 12);
        assert_eq!(&writer.as_slice()[10..12], &(-1i16).to_le_bytes());
    }

    #[test]
    fn stable_board_packets_round_trip() {
        use ragnarok_packets::{
            RequestStableHomunculusPacket, RequestStableOpenPacket, RequestStablePetPacket, StableHomunculusEntry,
            StableHomunculusListPacket, StableOwnedEntry, StableOwnedListPacket, StablePetEntry, StablePetListPacket, StableResultPacket,
        };

        let mut writer = ByteWriter::new();
        RequestStableOpenPacket::new().packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice(), &[0xF0, 0x0E]);

        let mut writer = ByteWriter::new();
        RequestStableHomunculusPacket { homunculus_id: -7 }
            .packet_to_bytes(&mut writer)
            .unwrap();
        assert_eq!(writer.as_slice().len(), 6);
        assert_eq!(u16::from_le_bytes([writer.as_slice()[0], writer.as_slice()[1]]), 0x0EF1);
        assert_eq!(&writer.as_slice()[2..6], &(-7i32).to_le_bytes());

        let mut writer = ByteWriter::new();
        RequestStablePetPacket { action: 2, pet_id: 44 }
            .packet_to_bytes(&mut writer)
            .unwrap();
        assert_eq!(writer.as_slice().len(), 8);
        assert_eq!(u16::from_le_bytes([writer.as_slice()[0], writer.as_slice()[1]]), 0x0EF2);
        assert_eq!(&writer.as_slice()[2..4], &2i16.to_le_bytes());
        assert_eq!(&writer.as_slice()[4..8], &44i32.to_le_bytes());

        let homunculi = StableHomunculusListPacket {
            active_homunculus_id: 3,
            count: 1,
            homunculi: vec![StableHomunculusEntry {
                homunculus_id: 3,
                class_id: 6001,
                previous_class: 6000,
                level: 12,
                intimacy: 250,
                hunger: 40,
                vaporize: 1,
                rename_flag: 0,
                name: "Lif".to_owned(),
            }],
        };
        let mut writer = ByteWriter::new();
        homunculi.packet_to_bytes(&mut writer).unwrap();
        // opcode + length + active id + count + 42-byte row.
        assert_eq!(writer.as_slice().len(), 4 + 4 + 1 + 42);
        for version in [
            SupportedPacketVersion::_20250716,
            SupportedPacketVersion::_20220406,
            SupportedPacketVersion::_20190530,
        ] {
            let events = process_one(version, writer.as_slice());
            assert!(matches!(events.as_slice(), [NetworkEvent::StableHomunculusList { active_id: 3, summary }] if summary.contains("Lif")));
        }

        let pets = StablePetListPacket {
            active_pet_id: 9,
            count: 1,
            pets: vec![StablePetEntry {
                pet_id: 9,
                class_id: 1002,
                level: 1,
                intimacy: 100,
                hunger: 20,
                egg_item_id: 9001,
                equip_item_id: 0,
                character_id: 5,
                incubate: 0,
                rename_flag: 1,
                flags: 0,
                name: "Picky".to_owned(),
                owner_name: "Ada".to_owned(),
                attack: 3,
                attack2: 4,
                attack_speed: 150,
                experience: 10,
                next_experience: 50,
                food_item_id: 531,
                buff: "none".to_owned(),
            }],
        };
        let mut writer = ByteWriter::new();
        pets.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 4 + 4 + 1 + 191);
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::StablePetList { active_id: 9, summary }] if summary.contains("Picky")));

        let owned = StableOwnedListPacket {
            count: 1,
            items: vec![StableOwnedEntry { item_id: 714, owned: 3 }],
        };
        let mut writer = ByteWriter::new();
        owned.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 4 + 2 + 8);
        let events = process_one(SupportedPacketVersion::_20190530, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::StableOwnedList { summary }] if summary.contains("714")));

        let result = StableResultPacket { result: 0, id: 9 };
        let mut writer = ByteWriter::new();
        result.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 8);
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::StableResult {
            result: 0,
            id: 9,
            ..
        }]));
    }

    #[test]
    fn rodex_mail_packets_round_trip() {
        use ragnarok_packets::{
            CloseRodexPacket, DeleteRodexPacket, NewMailStatusPacket, OpenRodexPacket, ReadRodexPacket, RefreshRodexPacket,
            RodexDeleteAckPacket, RodexMailEntry, RodexMailListPacket, RodexReadPacket, RodexReadPacket2025, RodexWriteResultPacket,
            RodexZenyAckPacket,
        };

        let status = NewMailStatusPacket { new_available: 1 };
        let mut writer = ByteWriter::new();
        status.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 3);
        assert_eq!(u16::from_le_bytes([writer.as_slice()[0], writer.as_slice()[1]]), 0x09E7);

        for version in [
            SupportedPacketVersion::_20250716,
            SupportedPacketVersion::_20220406,
            SupportedPacketVersion::_20190530,
        ] {
            let events = process_one(version, writer.as_slice());
            assert!(matches!(events.as_slice(), [NetworkEvent::NewMailStatus { available: true }]));
        }

        let title = "Welcome";
        let list = RodexMailListPacket {
            is_end: 1,
            mails: vec![RodexMailEntry {
                open_type: 0,
                mail_id: 42,
                is_read: 0,
                mail_type: 0x04,
                sender_name: "Kafra".to_owned(),
                expire_time: 1_700_000_000,
                title_length: title.len() as i16,
                title: title.to_owned(),
            }],
        };
        let mut writer = ByteWriter::new();
        list.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(u16::from_le_bytes([writer.as_slice()[0], writer.as_slice()[1]]), 0x0AC2);
        // header, length, is_end, row without title, title bytes.
        assert_eq!(writer.as_slice().len(), 2 + 2 + 1 + 1 + 8 + 1 + 1 + 24 + 4 + 2 + title.len());
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::RodexMailList { is_end: true, summary }]
            if summary.contains("#42 from Kafra: Welcome [unread] (item)")));

        let empty = RodexMailListPacket {
            is_end: 1,
            mails: Vec::new(),
        };
        let mut writer = ByteWriter::new();
        empty.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 5);
        let events = process_one(SupportedPacketVersion::_20190530, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::RodexMailList { is_end: true, summary }] if summary == "Inbox is empty."));

        let write_result = RodexWriteResultPacket { result: 0 };
        let mut writer = ByteWriter::new();
        write_result.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 3);
        let events = process_one(SupportedPacketVersion::_20220406, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::RodexWriteResult { result: 0, .. }]));

        let deleted = RodexDeleteAckPacket {
            open_type: 0,
            mail_id: 42,
        };
        let mut writer = ByteWriter::new();
        deleted.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 11);
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::RodexDelete { open_type: 0, mail_id: 42 }]));

        let zeny = RodexZenyAckPacket {
            mail_id: 42,
            open_type: 0,
            result: 0,
        };
        let mut writer = ByteWriter::new();
        zeny.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 12);
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::RodexZeny { mail_id: 42, result: 0, summary }] if summary.contains("#42")));

        let mut writer = ByteWriter::new();
        OpenRodexPacket {
            mail_id: 0,
            unused: [0; 16],
        }
        .packet_to_bytes(&mut writer)
        .unwrap();
        assert_eq!(writer.as_slice().len(), 26);
        assert_eq!(u16::from_le_bytes([writer.as_slice()[0], writer.as_slice()[1]]), 0x0AC0);

        let mut writer = ByteWriter::new();
        RefreshRodexPacket {
            mail_id: 0,
            unused: [0; 16],
        }
        .packet_to_bytes(&mut writer)
        .unwrap();
        assert_eq!(writer.as_slice().len(), 26);
        assert_eq!(u16::from_le_bytes([writer.as_slice()[0], writer.as_slice()[1]]), 0x0AC1);

        let body = "Hello";
        let read = RodexReadPacket {
            open_type: 0,
            mail_id: 42,
            text_length: (body.len() + 1) as i16,
            zeny: 100,
            item_count: 1,
            body: body.to_owned(),
            items: vec![0x07],
        };
        let mut writer = ByteWriter::new();
        read.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(u16::from_le_bytes([writer.as_slice()[0], writer.as_slice()[1]]), 0x09EB);
        let events = process_one(SupportedPacketVersion::_20220406, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::RodexRead { mail_id: 42, zeny: 100, item_count: 1, summary }]
            if summary.contains("Hello") && summary.contains("zeny 100")));
        let events = process_one(SupportedPacketVersion::_20190530, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::RodexRead { mail_id: 42, .. }]));

        let read_2025 = RodexReadPacket2025 {
            open_type: 0,
            mail_id: 42,
            text_length: (body.len() + 1) as i16,
            zeny: 100,
            item_count: 1,
            body: body.to_owned(),
            items: vec![0x07],
        };
        let mut writer = ByteWriter::new();
        read_2025.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(u16::from_le_bytes([writer.as_slice()[0], writer.as_slice()[1]]), 0x0B63);
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::RodexRead { mail_id: 42, item_count: 1, .. }]));

        let mut writer = ByteWriter::new();
        CloseRodexPacket::new().packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 2);
        assert_eq!(u16::from_le_bytes([writer.as_slice()[0], writer.as_slice()[1]]), 0x09E9);

        let mut writer = ByteWriter::new();
        ReadRodexPacket {
            open_type: 0,
            mail_id: 42,
        }
        .packet_to_bytes(&mut writer)
        .unwrap();
        assert_eq!(writer.as_slice().len(), 11);
        assert_eq!(u16::from_le_bytes([writer.as_slice()[0], writer.as_slice()[1]]), 0x09EA);

        let mut writer = ByteWriter::new();
        DeleteRodexPacket {
            open_type: 0,
            mail_id: 42,
        }
        .packet_to_bytes(&mut writer)
        .unwrap();
        assert_eq!(writer.as_slice().len(), 11);
        assert_eq!(u16::from_le_bytes([writer.as_slice()[0], writer.as_slice()[1]]), 0x09F5);
    }

    #[test]
    fn mvp_minimap_packet_replaces_marker_list() {
        use ragnarok_packets::{EntityId, MvpMinimapEntry, MvpMinimapPacket, TilePosition};

        use crate::minimap_point;

        let packet = MvpMinimapPacket {
            flags: 0,
            count: 1,
            markers: vec![MvpMinimapEntry {
                entity_id: EntityId(42),
                class_id: 1039,
                position: TilePosition { x: 100, y: 50 },
            }],
        };
        let mut writer = ByteWriter::new();
        packet.packet_to_bytes(&mut writer).unwrap();
        // opcode + length + flags + count + 10-byte row.
        assert_eq!(writer.as_slice().len(), 2 + 2 + 1 + 2 + 10);
        assert_eq!(u16::from_le_bytes([writer.as_slice()[0], writer.as_slice()[1]]), 0x0EF5);

        for version in [
            SupportedPacketVersion::_20250716,
            SupportedPacketVersion::_20220406,
            SupportedPacketVersion::_20190530,
        ] {
            let events = process_one(version, writer.as_slice());
            assert!(matches!(events.as_slice(), [NetworkEvent::MvpMinimap { flags: 0, markers }]
                if markers.len() == 1
                    && markers[0].class_id == 1039
                    && markers[0].position == TilePosition { x: 100, y: 50 }
                    && markers[0].entity_id == EntityId(42)));
        }

        let empty = MvpMinimapPacket {
            flags: 1,
            count: 0,
            markers: Vec::new(),
        };
        let mut writer = ByteWriter::new();
        empty.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 7);
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::MvpMinimap { flags: 1, markers }] if markers.is_empty()));

        let point = minimap_point(200, 100, TilePosition { x: 100, y: 50 }, 100.0).unwrap();
        assert!((point.0 - 50.25).abs() < 0.01);
        assert!((point.1 - 49.5).abs() < 0.01);
        assert!(minimap_point(0, 100, TilePosition { x: 0, y: 0 }, 100.0).is_none());
        assert!(minimap_point(200, 100, TilePosition { x: 200, y: 0 }, 100.0).is_none());
    }

    #[test]
    fn eden_board_packets_round_trip() {
        use ragnarok_packets::{EdenQuestEntry, EdenQuestListPacket, EdenResultPacket, EdenShopEntry, EdenShopPacket, RequestEdenPacket};

        let mut writer = ByteWriter::new();
        RequestEdenPacket { action: 1, quest_id: 44 }.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 8);
        assert_eq!(u16::from_le_bytes([writer.as_slice()[0], writer.as_slice()[1]]), 0x0EF3);
        assert_eq!(&writer.as_slice()[2..4], &1i16.to_le_bytes());
        assert_eq!(&writer.as_slice()[4..8], &44i32.to_le_bytes());

        let list = EdenQuestListPacket {
            base_level: 50,
            count: 1,
            reserved: 0,
            merit: 7,
            quests: vec![EdenQuestEntry {
                quest_id: 44,
                state: 2,
                min_level: 41,
                max_level: 55,
                mob_id: 1002,
                hunt_count: 3,
                hunt_max: 10,
                item_id: 909,
                item_have: 1,
                item_need: 2,
                base_exp: 100,
                job_exp: 50,
                reward_item: 501,
                reward_item_amount: 1,
                merit: 2,
                map: "prt_fild08".to_owned(),
                spawn_count: 4,
                name: "Poring hunt".to_owned(),
                hint: "South of Prontera".to_owned(),
            }],
        };
        let mut writer = ByteWriter::new();
        list.packet_to_bytes(&mut writer).unwrap();
        // opcode + length + level + count + reserved + merit + 203-byte row.
        assert_eq!(writer.as_slice().len(), 4 + 2 + 1 + 1 + 4 + 203);
        for version in [
            SupportedPacketVersion::_20250716,
            SupportedPacketVersion::_20220406,
            SupportedPacketVersion::_20190530,
        ] {
            let events = process_one(version, writer.as_slice());
            assert!(matches!(
                events.as_slice(),
                [NetworkEvent::EdenQuestList { base_level: 50, merit: 7, summary }]
                    if summary.contains("Poring hunt") && summary.contains("ready")
            ));
        }

        let shop = EdenShopPacket {
            merit: 7,
            count: 1,
            items: vec![EdenShopEntry {
                slot: 3,
                category: 1,
                min_level: 11,
                item_id: 501,
                amount: 2,
                price_merit: 4,
                price_zeny: 0,
            }],
        };
        let mut writer = ByteWriter::new();
        shop.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 4 + 4 + 2 + 18);
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(
            events.as_slice(),
            [NetworkEvent::EdenShop { merit: 7, summary }] if summary.contains("slot 3") && summary.contains("gear")
        ));

        let result = EdenResultPacket { result: 9, quest_id: 44 };
        let mut writer = ByteWriter::new();
        result.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 8);
        assert_eq!(u16::from_le_bytes([writer.as_slice()[0], writer.as_slice()[1]]), 0x0EFC);
        let events = process_one(SupportedPacketVersion::_20190530, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::EdenResult { result: 9, quest_id: 44, summary }] if summary.contains("Merit")));
    }

    #[test]
    fn warpra_packets_round_trip_chunked_and_legacy() {
        use ragnarok_packets::{
            RequestWarpraPacket, WARPRA_ACTION_LIST, WarpraDestinationEntry, WarpraDestinationListPacket, WarpraResultPacket,
        };

        let mut writer = ByteWriter::new();
        RequestWarpraPacket {
            action: WARPRA_ACTION_LIST,
            kind: 0,
            location_id: 0,
        }
        .packet_to_bytes(&mut writer)
        .unwrap();
        assert_eq!(writer.as_slice().len(), 8);
        assert_eq!(u16::from_le_bytes([writer.as_slice()[0], writer.as_slice()[1]]), 0x0EF4);

        let entry = WarpraDestinationEntry {
            kind: 2,
            id: 7,
            unlocked: 0,
            x: 100,
            y: 40,
            group: 3,
            map: "pay_dun00".to_owned(),
            name: "Payon Dungeon (Lv 55+)".to_owned(),
            group_name: "Payon".to_owned(),
        };
        let list = WarpraDestinationListPacket {
            zeny: 12_000,
            price: 50_000_000,
            flags: 0,
            total: 1,
            destinations: vec![entry],
        };
        let mut writer = ByteWriter::new();
        list.packet_to_bytes(&mut writer).unwrap();
        // opcode + length + zeny + price + flags + total + count + 89-byte row.
        assert_eq!(writer.as_slice().len(), 4 + 8 + 5 + 89);
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(
            matches!(events.as_slice(), [NetworkEvent::WarpraList { zeny: 12_000, price: 50_000_000, destinations, .. }] if destinations.len() == 1 && destinations[0].name.contains("Payon"))
        );

        let mut legacy = vec![0xFE, 0x0E, 0, 0];
        legacy.extend_from_slice(&12_000i32.to_le_bytes());
        legacy.extend_from_slice(&50_000_000i32.to_le_bytes());
        legacy.extend_from_slice(&1i16.to_le_bytes());
        let row_start = writer.as_slice().len() - 89;
        legacy.extend_from_slice(&writer.as_slice()[row_start..]);
        let packet_len = legacy.len() as u16;
        legacy[2..4].copy_from_slice(&packet_len.to_le_bytes());
        let events = process_one(SupportedPacketVersion::_20190530, &legacy);
        assert!(
            matches!(events.as_slice(), [NetworkEvent::WarpraList { flags: 0, total: 1, destinations, .. }] if destinations[0].id == 7 && destinations[0].map == "pay_dun00")
        );

        let mut writer = ByteWriter::new();
        WarpraResultPacket {
            result: 2,
            kind: 2,
            location_id: 7,
        }
        .packet_to_bytes(&mut writer)
        .unwrap();
        assert_eq!(writer.as_slice().len(), 8);
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(
            matches!(events.as_slice(), [NetworkEvent::WarpraResult { result: 2, location_id: 7, summary, .. }] if summary.contains("locked"))
        );
    }

    #[test]
    fn companion_packets_match_20250716_lengths() {
        use ragnarok_packets::{
            ElementalInitPacket, ElementalParameterPacket, HomunculusCommandPacket, HomunculusPropertyPacket, MercenaryCommandPacket,
            MercenaryInitPacket, MercenaryParameterPacket, MercenaryPropertyPacket, PetCommandPacket, PetPropertyPacket, PetStatePacket,
        };

        let mut writer = ByteWriter::new();
        PetCommandPacket { action: 1 }.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 3);
        assert_eq!(u16::from_le_bytes([writer.as_slice()[0], writer.as_slice()[1]]), 0x01A1);

        let mut writer = ByteWriter::new();
        HomunculusCommandPacket {
            kind: 2,
            command: 2,
        }
        .packet_to_bytes(&mut writer)
        .unwrap();
        assert_eq!(writer.as_slice().len(), 5);
        assert_eq!(u16::from_le_bytes([writer.as_slice()[0], writer.as_slice()[1]]), 0x022D);

        let mut writer = ByteWriter::new();
        MercenaryCommandPacket { command: 1 }
            .packet_to_bytes(&mut writer)
            .unwrap();
        assert_eq!(writer.as_slice().len(), 3);

        let pet = PetPropertyPacket {
            name: "Poring".to_owned(),
            renamed: 1,
            level: 12,
            hunger: 80,
            intimacy: 910,
            accessory_id: 0,
            class_id: 1002,
        };
        let mut writer = ByteWriter::new();
        pet.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 37);
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(
            events.as_slice(),
            [NetworkEvent::PetInfo { class_id: 1002, intimacy: 910, summary, .. }] if summary.contains("Loyal") && summary.contains("Poring")
        ));

        let mut writer = ByteWriter::new();
        PetStatePacket {
            kind: 2,
            entity_id: 9,
            value: 40,
        }
        .packet_to_bytes(&mut writer)
        .unwrap();
        assert_eq!(writer.as_slice().len(), 11);
        let events = process_one(SupportedPacketVersion::_20220406, writer.as_slice());
        assert!(matches!(
            events.as_slice(),
            [NetworkEvent::PetState { kind: 2, value: 40, .. }]
        ));

        let hom = HomunculusPropertyPacket {
            name: "Lif".to_owned(),
            flags: 0x05,
            level: 50,
            hunger: 70,
            intimacy: 500,
            attack: 100,
            magic_attack: 20,
            hit: 30,
            critical: 4,
            defense: 40,
            magic_defense: 15,
            flee: 60,
            attack_motion: 150,
            health_points: 2000,
            maximum_health_points: 3000,
            spell_points: 100,
            maximum_spell_points: 200,
            experience: 10,
            next_experience: 1000,
            skill_points: 3,
            attack_range: 2,
        };
        let mut writer = ByteWriter::new();
        hom.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 85);
        assert_eq!(u16::from_le_bytes([writer.as_slice()[0], writer.as_slice()[1]]), 0x0BA4);
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(
            events.as_slice(),
            [NetworkEvent::HomunculusInfo { level: 50, health_points: 2000, summary, .. }]
                if summary.contains("Lif") && summary.contains("down") && summary.contains("Neutral")
        ));
        // 20220406 shares the 20190530 framing table and must accept 0x0BA4.
        let events = process_one(SupportedPacketVersion::_20220406, writer.as_slice());
        assert!(matches!(
            events.as_slice(),
            [NetworkEvent::HomunculusInfo { level: 50, .. }]
        ));

        let merc = MercenaryInitPacket {
            account_id: 4,
            attack: 10,
            magic_attack: 1,
            hit: 2,
            critical: 3,
            defense: 4,
            magic_defense: 5,
            flee: 6,
            attack_speed: 7,
            name: "Spear".to_owned(),
            level: 20,
            health_points: 500,
            maximum_health_points: 800,
            spell_points: 10,
            maximum_spell_points: 40,
            expire_date: 1_700_000_000,
            faith: 8,
            summons: 2,
            kills: 9,
            attack_range: 3,
        };
        let mut writer = ByteWriter::new();
        merc.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 80);
        let events = process_one(SupportedPacketVersion::_20190530, writer.as_slice());
        assert!(matches!(
            events.as_slice(),
            [NetworkEvent::MercenaryInfo { account_id: 4, kills: 9, summary, .. }] if summary.contains("Spear")
        ));

        let property = MercenaryPropertyPacket {
            attack: 11,
            magic_attack: 1,
            hit: 2,
            critical: 3,
            defense: 4,
            magic_defense: 5,
            flee: 6,
            attack_speed: 7,
            name: "Spear".to_owned(),
            level: 21,
            health_points: 400,
            maximum_health_points: 800,
            spell_points: 10,
            maximum_spell_points: 40,
            expire_date: 1_700_000_000,
            faith: 8,
            summons: 2,
            kills: 10,
        };
        let mut writer = ByteWriter::new();
        property.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 66);

        let mut writer = ByteWriter::new();
        MercenaryParameterPacket {
            variable: 8,
            value: 123,
        }
        .packet_to_bytes(&mut writer)
        .unwrap();
        assert_eq!(writer.as_slice().len(), 8);
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(
            events.as_slice(),
            [NetworkEvent::MercenaryParameter { variable: 8, value: 123 }]
        ));

        let mut writer = ByteWriter::new();
        ElementalInitPacket {
            class_id: 2114,
            health_points: 100,
            maximum_health_points: 200,
            spell_points: 50,
            maximum_spell_points: 80,
        }
        .packet_to_bytes(&mut writer)
        .unwrap();
        assert_eq!(writer.as_slice().len(), 22);
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(
            events.as_slice(),
            [NetworkEvent::ElementalInfo { class_id: 2114, summary, .. }] if summary.contains("Agni")
        ));

        let mut writer = ByteWriter::new();
        ElementalParameterPacket {
            variable: 2,
            value: 12,
        }
        .packet_to_bytes(&mut writer)
        .unwrap();
        assert_eq!(writer.as_slice().len(), 8);
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(
            events.as_slice(),
            [NetworkEvent::ElementalParameter { variable: 2, value: 12 }]
        ));
    }

    #[test]
    fn market_open_is_a_shop_distinct_from_player_vending() {
        use ragnarok_packets::{MarketItemInformation, OpenMarketPacket, Price};

        let packet = OpenMarketPacket {
            items: vec![MarketItemInformation {
                name_id: 501,
                item_type: 0,
                price: Price(50),
                quantity: 3,
                weight: 10,
                location: 0,
            }],
        };
        let mut writer = ByteWriter::new();
        packet.packet_to_bytes(&mut writer).unwrap();

        for version in [
            SupportedPacketVersion::_20250716,
            SupportedPacketVersion::_20220406,
            SupportedPacketVersion::_20190530,
        ] {
            let events = process_one(version, writer.as_slice());
            assert!(
                matches!(events.as_slice(), [NetworkEvent::OpenShop { items }] if items.len() == 1
                && items[0].item_id == ragnarok_packets::ItemId(501)
                && items[0].price == Price(50)
                && items[0].quantity == crate::ItemQuantity::Fixed(3)
                && items[0].weight == 10)
            );
        }
    }

    #[test]
    fn packetver_20250716_trade_packets_become_typed_events() {
        use ragnarok_packets::version_20250716::{
            TradeAddItemPacket, TradeAddItemResultPacket, TradeCancelPacket, TradeCompletePacket, TradeConfirmPacket, TradeRequestPacket,
            TradeStartPacket, TradeUnlockPacket,
        };
        use ragnarok_packets::{
            AccountId, AddTradeItemPacket, CancelTradePacket, CharacterId, CommitTradePacket, ConcludeTradePacket, ItemId,
            RequestTradePacket, TradeAckPacket,
        };

        let request = TradeRequestPacket {
            partner_name: "Alice".to_owned(),
            partner_character_id: CharacterId(99),
            level: 80,
        };
        let mut writer = ByteWriter::new();
        request.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 32);
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::TradeRequested {
            partner_name,
            partner_character_id: CharacterId(99),
            level: 80,
        }] if partner_name == "Alice"));

        let opened = TradeStartPacket {
            result: 3,
            partner_character_id: CharacterId(99),
            level: 80,
        };
        let mut writer = ByteWriter::new();
        opened.packet_to_bytes(&mut writer).unwrap();
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::TradeOpened {
            result: 3,
            partner_character_id: CharacterId(99),
            level: 80,
        }]));

        let item = TradeAddItemPacket {
            item_id: ItemId(501),
            item_type: 0,
            amount: 2,
            identified: 1,
            damaged: 0,
            cards: [0; 4],
            option_data: std::array::from_fn(|_| zero_trade_option()),
            location: 0,
            look: 0,
            forge_data: std::array::from_fn(|_| zero_trade_option()),
            refinement_level: 7,
            grade: 0,
        };
        let mut writer = ByteWriter::new();
        item.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 77);
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::TradeItemAdded {
            item_id: ItemId(501),
            amount: 2,
            identified: true,
            refinement_level: 7,
            ..
        }]));

        let zeny = TradeAddItemPacket {
            item_id: ItemId(0),
            item_type: 0,
            amount: 1_500,
            identified: 0,
            damaged: 0,
            cards: [0; 4],
            option_data: std::array::from_fn(|_| zero_trade_option()),
            location: 0,
            look: 0,
            forge_data: std::array::from_fn(|_| zero_trade_option()),
            refinement_level: 0,
            grade: 0,
        };
        let mut writer = ByteWriter::new();
        zeny.packet_to_bytes(&mut writer).unwrap();
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::TradeZenyAdded { amount: 1_500 }]));

        let added = TradeAddItemResultPacket { index: 4, result: 0 };
        let mut writer = ByteWriter::new();
        added.packet_to_bytes(&mut writer).unwrap();
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::TradeAddResult {
            index: 4,
            result: 0
        }]));

        let confirm = TradeConfirmPacket { which: 1 };
        let mut writer = ByteWriter::new();
        confirm.packet_to_bytes(&mut writer).unwrap();
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::TradeConfirmed { partner: true }]));

        let mut writer = ByteWriter::new();
        TradeCancelPacket::new().packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice(), &[0xEE, 0x00]);
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::TradeCancelled]));

        let mut writer = ByteWriter::new();
        TradeCompletePacket { result: 0 }.packet_to_bytes(&mut writer).unwrap();
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::TradeCompleted { result: 0 }]));

        let mut writer = ByteWriter::new();
        TradeUnlockPacket::new().packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice(), &[0xF1, 0x00]);
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::TradeLockRemoved]));

        let mut writer = ByteWriter::new();
        RequestTradePacket::new(AccountId(5)).packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice(), &[0xE4, 0x00, 5, 0, 0, 0]);
        let mut writer = ByteWriter::new();
        TradeAckPacket::new(3).packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice(), &[0xE6, 0x00, 3]);
        let mut writer = ByteWriter::new();
        AddTradeItemPacket::new(0, 1_500).packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice(), &[0xE8, 0x00, 0, 0, 0xDC, 0x05, 0, 0]);
        let mut writer = ByteWriter::new();
        ConcludeTradePacket::new().packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice(), &[0xEB, 0x00]);
        let mut writer = ByteWriter::new();
        CancelTradePacket::new().packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice(), &[0xED, 0x00]);
        let mut writer = ByteWriter::new();
        CommitTradePacket::new().packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice(), &[0xEF, 0x00]);

        let mut writer = ByteWriter::new();
        request.packet_to_bytes(&mut writer).unwrap();
        let mut handler = map_handler(SupportedPacketVersion::_20220406);
        let mut reader = ByteReader::without_metadata(writer.as_slice());
        assert!(matches!(handler.process_one(&mut reader), HandlerResult::UnhandledPacket));
    }

    #[test]
    fn vending_list_and_purchase_round_trip_on_20250716() {
        use ragnarok_bytes::FixedByteSize;
        use ragnarok_packets::{
            AccountId, BuyFromVendingPacket, ItemId, ItemOptions, OpenVendingItemInformation, OpenVendingShopPacket, Price,
            VendingPurchaseInformation, VendingShopItem, VendingShopListPacket,
        };

        assert_eq!(VendingShopItem::size_in_bytes(), 64);
        let empty_option = || ItemOptions {
            index: 0,
            value: 0,
            parameter: 0,
        };

        let list = VendingShopListPacket {
            account_id: AccountId(9),
            vender_id: 77,
            items: vec![VendingShopItem {
                price: Price(120),
                amount: 4,
                index: 3,
                item_type: 0,
                item_id: ItemId(501),
                identified: 1,
                damaged: 0,
                cards: [0; 4],
                options: std::array::from_fn(|_| empty_option()),
                location: 0,
                view_sprite: 0,
                refine: 0,
                grade: 0,
            }],
        };
        let mut writer = ByteWriter::new();
        list.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(u16::from_le_bytes([writer.as_slice()[0], writer.as_slice()[1]]), 0x0B3D);

        for version in [SupportedPacketVersion::_20250716, SupportedPacketVersion::_20220406] {
            let events = process_one(version, writer.as_slice());
            assert!(matches!(events.as_slice(), [NetworkEvent::PlayerShopOpened {
                account_id: AccountId(9),
                store_id: 77,
                buying_store: false,
                items,
                indexes,
            }] if items.len() == 1
                && items[0].item_id == ItemId(501)
                && items[0].price == Price(120)
                && indexes.as_slice() == [3]));
        }

        let mut reader_handler = map_handler(SupportedPacketVersion::_20190530);
        let mut reader = ByteReader::without_metadata(writer.as_slice());
        assert!(matches!(
            reader_handler.process_one(&mut reader),
            HandlerResult::UnhandledPacket
        ));

        let purchase = BuyFromVendingPacket {
            account_id: AccountId(9),
            vender_id: 77,
            items: vec![VendingPurchaseInformation { amount: 1, index: 3 }],
        };
        let mut writer = ByteWriter::new();
        purchase.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(&writer.as_slice()[..2], &[0x01, 0x08]);

        let opened = OpenVendingShopPacket {
            shop_name: "Potions".to_owned(),
            items: vec![OpenVendingItemInformation {
                inventory_index: 2,
                amount: 5,
                price: Price(40),
            }],
        };
        let mut writer = ByteWriter::new();
        opened.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(&writer.as_slice()[..2], &[0x2F, 0x01]);
        assert_eq!(writer.as_slice().len(), 2 + 2 + 80 + 8);

        // 0x0812 is open rejection (result + weight, 8 bytes). 0x081A is a finished
        // trade rejection (result only, 4 bytes). Building the structs would hide a
        // swap.
        let open_failed = [0x12, 0x08, 0x02, 0x00, 0x40, 0x1F, 0x00, 0x00];
        let trade_failed = [0x1A, 0x08, 0x03, 0x00];
        for version in [SupportedPacketVersion::_20250716, SupportedPacketVersion::_20220406] {
            let events = process_one(version, &open_failed);
            assert!(matches!(events.as_slice(), [NetworkEvent::BuyingStoreOpenFailed {
                result: 2,
                total_weight: 8000,
            }]));
            let events = process_one(version, &trade_failed);
            assert!(matches!(events.as_slice(), [NetworkEvent::BuyingStoreTradeFailed {
                result: 3
            }]));
        }
        assert!(matches!(
            map_handler(SupportedPacketVersion::_20190530).process_one(&mut ByteReader::without_metadata(&open_failed)),
            HandlerResult::UnhandledPacket
        ));
        assert!(matches!(
            map_handler(SupportedPacketVersion::_20190530).process_one(&mut ByteReader::without_metadata(&trade_failed)),
            HandlerResult::UnhandledPacket
        ));
    }

    #[test]
    fn search_store_result_is_typed() {
        use ragnarok_bytes::FixedByteSize;
        use ragnarok_packets::{AccountId, ItemId, ItemOptions, Price, SearchStoreHitInformation, SearchStoreResultPacket};

        assert_eq!(SearchStoreHitInformation::size_in_bytes(), 142);

        let packet = SearchStoreResultPacket {
            first_page: 1,
            next_page: 0,
            uses_remaining: 2,
            hits: vec![SearchStoreHitInformation {
                store_id: 15,
                account_id: AccountId(4),
                shop_name: "Arrow shop".to_owned(),
                item_id: ItemId(1750),
                item_type: 0,
                price: Price(10),
                amount: 99,
                cards: [0; 4],
                options: std::array::from_fn(|_| ItemOptions {
                    index: 0,
                    value: 0,
                    parameter: 0,
                }),
                refine: 0,
                grade: 0,
            }],
        };
        let mut writer = ByteWriter::new();
        packet.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(&writer.as_slice()[..2], &[0x64, 0x0B]);

        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::SearchStoreResults {
            first_page: true,
            next_page: false,
            uses_remaining: 2,
            hits,
        }] if hits.len() == 1
            && hits[0].store_id == 15
            && hits[0].account_id == AccountId(4)
            && hits[0].shop_name == "Arrow shop"
            && hits[0].item_id == ItemId(1750)
            && hits[0].amount == 99));
    }

    #[test]
    fn refine_open_materials_and_result_are_typed() {
        use ragnarok_packets::{
            InventoryIndex, ItemId, PacketExt, RefineClosePacket, RefineMaterialInformation, RefineMaterialListPacket, RefineOpenPacket,
            RefineRequestPacket, RefineSelectItemPacket,
        };

        let open = [0xA0, 0x0A];
        // Wire index 5 is stored index 3. Two ores: Bradium 70% / 20000z, Carnium 40% /
        // 50000z. Length includes the header: 2 + 2 + 2 + 1 + 9 + 9 = 25.
        let mut materials = vec![0xA2, 0x0A, 25, 0, 5, 0, 1];
        materials.extend_from_slice(&6223u32.to_le_bytes());
        materials.push(70);
        materials.extend_from_slice(&20_000u32.to_le_bytes());
        materials.extend_from_slice(&6224u32.to_le_bytes());
        materials.push(40);
        materials.extend_from_slice(&50_000u32.to_le_bytes());
        assert_eq!(materials.len(), 25);

        let success = [0x88, 0x01, 0x00, 0x00, 5, 0, 8, 0];

        for version in [
            SupportedPacketVersion::_20250716,
            SupportedPacketVersion::_20220406,
            SupportedPacketVersion::_20190530,
        ] {
            assert!(matches!(process_one(version, &open).as_slice(), [NetworkEvent::RefineOpened]));
            let events = process_one(version, &materials);
            assert!(matches!(events.as_slice(), [NetworkEvent::RefineMaterials {
                index: InventoryIndex(3),
                blacksmith_blessing: 1,
                materials,
            }] if materials.len() == 2
                && materials[0].item_id == ItemId(6223)
                && materials[0].chance_percent == 70
                && materials[0].zeny == 20_000
                && materials[1].item_id == ItemId(6224)
                && materials[1].chance_percent == 40));
            assert!(matches!(process_one(version, &success).as_slice(), [
                NetworkEvent::RefineResult {
                    result: 0,
                    index: InventoryIndex(3),
                    refine_level: 8,
                }
            ]));
        }

        let select = RefineSelectItemPacket { index: InventoryIndex(3) };
        let mut writer = ByteWriter::new();
        select.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice(), &[0xA1, 0x0A, 5, 0]);

        let request = RefineRequestPacket {
            index: InventoryIndex(3),
            item_id: ItemId(6223),
            blacksmith_blessing: 0,
        };
        let mut writer = ByteWriter::new();
        request.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 9);
        assert_eq!(&writer.as_slice()[..2], &[0xA3, 0x0A]);
        assert_eq!(&writer.as_slice()[2..4], &5u16.to_le_bytes());
        assert_eq!(&writer.as_slice()[4..8], &6223u32.to_le_bytes());

        let close = RefineClosePacket {};
        let mut writer = ByteWriter::new();
        close.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice(), &[0xA4, 0x0A]);

        let listed = RefineMaterialListPacket {
            index: InventoryIndex(3),
            blacksmith_blessing: 0,
            materials: vec![RefineMaterialInformation {
                item_id: ItemId(6224),
                chance_percent: 40,
                zeny: 50_000,
            }],
        };
        let mut writer = ByteWriter::new();
        listed.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 2 + 2 + 2 + 1 + 9);
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::RefineMaterials { materials, .. }] if materials[0].item_id == ItemId(6224)));

        let opened = RefineOpenPacket {};
        let mut writer = ByteWriter::new();
        opened.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice(), &[0xA0, 0x0A]);
    }

    #[test]
    fn item_workshop_packets_round_trip() {
        use ragnarok_packets::{
            GradeEnchantBlessing, GradeEnchantBroadcastPacket, GradeEnchantClosePacket, GradeEnchantMaterial,
            GradeEnchantMaterialListPacket, GradeEnchantRequestPacket, GradeEnchantResultPacket, GradeEnchantSelectPacket, InventoryIndex,
            ItemId, ItemReformClosePacket, ItemReformOpenPacket, ItemReformRequestPacket, ItemReformResultPacket,
            LaphineSynthesisClosePacket, LaphineSynthesisMaterial, LaphineSynthesisOpenPacket, LaphineSynthesisRequestPacket,
            LaphineSynthesisResultPacket, LaphineUpgradeClosePacket, LaphineUpgradeOpenPacket, LaphineUpgradeRequestPacket,
            LaphineUpgradeResultPacket, UiOpenPacket,
        };

        let open_grade = UiOpenPacket { ui_type: 8, data: 0 };
        let mut writer = ByteWriter::new();
        open_grade.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 7);
        for version in [SupportedPacketVersion::_20250716, SupportedPacketVersion::_20220406] {
            assert!(matches!(process_one(version, writer.as_slice()).as_slice(), [
                NetworkEvent::EnchantGradeOpened { data: 0 }
            ]));
        }

        let other_ui = UiOpenPacket { ui_type: 1, data: 4 };
        let mut writer = ByteWriter::new();
        other_ui.packet_to_bytes(&mut writer).unwrap();
        assert!(process_one(SupportedPacketVersion::_20250716, writer.as_slice()).is_empty());

        let mut list = GradeEnchantMaterialListPacket {
            index: -1,
            success_chance: 0,
            blessing: GradeEnchantBlessing {
                item_id: 0,
                amount: 0,
                max_blessing: 0,
                bonus: 0,
            },
            protect_item_id: ItemId(0),
            protect_amount: 0,
            materials: Vec::new(),
        };
        let mut writer = ByteWriter::new();
        list.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 34);
        assert!(matches!(
            process_one(SupportedPacketVersion::_20250716, writer.as_slice()).as_slice(),
            [NetworkEvent::EnchantGradeMaterials { index: None, materials, .. }] if materials.is_empty()
        ));

        list.index = 7;
        list.success_chance = 55;
        list.blessing.item_id = 6635;
        list.blessing.amount = 1;
        list.blessing.max_blessing = 3;
        list.blessing.bonus = 2;
        list.materials.push(GradeEnchantMaterial {
            item_id: ItemId(984),
            amount: 1,
            price: 1000,
            downgrade: 1,
            breakable: 0,
        });
        let mut writer = ByteWriter::new();
        list.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 34 + 17);
        let events = process_one(SupportedPacketVersion::_20250716, writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::EnchantGradeMaterials {
            index: Some(5),
            success_chance: 55,
            materials,
            ..
        }] if materials.len() == 1 && materials[0].item_id == ItemId(984) && materials[0].downgrade && !materials[0].breakable));

        let result = GradeEnchantResultPacket {
            index: InventoryIndex(5),
            grade: 2,
            result: 0,
        };
        let mut writer = ByteWriter::new();
        result.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 10);
        assert!(matches!(
            process_one(SupportedPacketVersion::_20250716, writer.as_slice()).as_slice(),
            [NetworkEvent::EnchantGradeResult {
                index: InventoryIndex(5),
                grade: 2,
                result: 0
            }]
        ));

        let broadcast = GradeEnchantBroadcastPacket {
            name: "Nova".to_owned(),
            item_id: ItemId(1101),
            grade: 1,
            status: 1,
        };
        let mut writer = ByteWriter::new();
        broadcast.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 33);
        assert!(matches!(
            process_one(SupportedPacketVersion::_20250716, writer.as_slice()).as_slice(),
            [NetworkEvent::EnchantGradeBroadcast {
                success: true,
                grade: 1,
                ..
            }]
        ));

        let select = GradeEnchantSelectPacket { index: InventoryIndex(5) };
        let mut writer = ByteWriter::new();
        select.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice(), &[0x59, 0x0B, 7, 0]);

        let request = GradeEnchantRequestPacket {
            index: InventoryIndex(5),
            material_index: 1,
            blessing_flag: 1,
            blessing_amount: 2,
            protect_flag: 0,
        };
        let mut writer = ByteWriter::new();
        request.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 14);
        assert_eq!(&writer.as_slice()[..2], &[0x5B, 0x0B]);

        let close_grade = GradeEnchantClosePacket {};
        let mut writer = ByteWriter::new();
        close_grade.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice(), &[0x5C, 0x0B]);

        let reform = ItemReformOpenPacket { item_id: ItemId(4001) };
        let mut writer = ByteWriter::new();
        reform.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 6);
        assert!(matches!(
            process_one(SupportedPacketVersion::_20250716, writer.as_slice()).as_slice(),
            [NetworkEvent::ItemReformOpened { item_id: ItemId(4001) }]
        ));

        let reform_result = ItemReformResultPacket {
            index: InventoryIndex(3),
            result: 0,
        };
        let mut writer = ByteWriter::new();
        reform_result.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 5);
        assert!(matches!(
            process_one(SupportedPacketVersion::_20220406, writer.as_slice()).as_slice(),
            [NetworkEvent::ItemReformResult {
                index: InventoryIndex(3),
                result: 0
            }]
        ));

        let reform_request = ItemReformRequestPacket {
            item_id: ItemId(4001),
            index: InventoryIndex(3),
        };
        let mut writer = ByteWriter::new();
        reform_request.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 8);
        assert_eq!(&writer.as_slice()[..2], &[0x91, 0x0B]);

        let reform_close = ItemReformClosePacket {};
        let mut writer = ByteWriter::new();
        reform_close.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice(), &[0x90, 0x0B]);

        let synthesis = LaphineSynthesisOpenPacket { item_id: ItemId(7210) };
        let mut writer = ByteWriter::new();
        synthesis.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 6);
        for version in [
            SupportedPacketVersion::_20250716,
            SupportedPacketVersion::_20220406,
            SupportedPacketVersion::_20190530,
        ] {
            assert!(matches!(process_one(version, writer.as_slice()).as_slice(), [
                NetworkEvent::LaphineSynthesisOpened { item_id: ItemId(7210) }
            ]));
        }

        let synthesis_request = LaphineSynthesisRequestPacket {
            item_id: ItemId(7210),
            materials: vec![LaphineSynthesisMaterial {
                index: InventoryIndex(4),
                count: 2,
            }],
        };
        let mut writer = ByteWriter::new();
        synthesis_request.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 2 + 2 + 4 + 4);
        assert_eq!(&writer.as_slice()[8..10], &6u16.to_le_bytes());

        let synthesis_result = LaphineSynthesisResultPacket { result: 0 };
        let mut writer = ByteWriter::new();
        synthesis_result.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 4);
        assert!(matches!(
            process_one(SupportedPacketVersion::_20190530, writer.as_slice()).as_slice(),
            [NetworkEvent::LaphineSynthesisResult { result: 0 }]
        ));

        let synthesis_close = LaphineSynthesisClosePacket {};
        let mut writer = ByteWriter::new();
        synthesis_close.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice(), &[0x70, 0x0A]);

        let upgrade = LaphineUpgradeOpenPacket { item_id: ItemId(7211) };
        let mut writer = ByteWriter::new();
        upgrade.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 6);
        assert!(matches!(
            process_one(SupportedPacketVersion::_20250716, writer.as_slice()).as_slice(),
            [NetworkEvent::LaphineUpgradeOpened { item_id: ItemId(7211) }]
        ));

        let upgrade_request = LaphineUpgradeRequestPacket {
            item_id: ItemId(7211),
            index: InventoryIndex(1),
        };
        let mut writer = ByteWriter::new();
        upgrade_request.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 8);

        let upgrade_result = LaphineUpgradeResultPacket { result: 1 };
        let mut writer = ByteWriter::new();
        upgrade_result.packet_to_bytes(&mut writer).unwrap();
        assert!(matches!(
            process_one(SupportedPacketVersion::_20250716, writer.as_slice()).as_slice(),
            [NetworkEvent::LaphineUpgradeResult { result: 1 }]
        ));

        let upgrade_close = LaphineUpgradeClosePacket {};
        let mut writer = ByteWriter::new();
        upgrade_close.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice(), &[0xB5, 0x0A]);
    }
}

#[cfg(test)]
mod transport_tests {
    use super::*;
    fn exercise(version: SupportedPacketVersion) {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let local = tokio::task::LocalSet::new();
        local.block_on(&runtime,async {
            let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address=listener.local_addr().unwrap();
            let server=tokio::spawn(async move {
                let (mut stream,_)=listener.accept().await.unwrap();
                // Each raw account-ID byte is delivered separately.
                for byte in 42u32.to_le_bytes() {
                    stream.write_all(&[byte]).await.unwrap();
                    tokio::time::sleep(Duration::from_millis(2)).await;
                }
                let mut bytes=Vec::new();
                if matches!(version,SupportedPacketVersion::_20190530) {
                    // Unsupported plugin frame larger than a socket read.
                    bytes.resize(20_000,0);
                    // 0x00AE is a framed variable-length opcode with no handler.
                    // 0x0EF5 is the alive-MVP list and would be parsed instead of skipped.
                    bytes[..4].copy_from_slice(&[0xae,0x00,0x20,0x4e]);
                }
                // A handled large frame, then another gameplay frame.
                let mut writer=ByteWriter::new();
                ServerMessagePacket::new("A".repeat(20_000)).packet_to_bytes(&mut writer).unwrap();
                DisplayEmotionPacket::new(EntityId(123),7).packet_to_bytes(&mut writer).unwrap();
                bytes.extend_from_slice(writer.as_slice());
                // Force a header-only initial fragment too.
                stream.write_all(&bytes[..1]).await.unwrap();
                tokio::time::sleep(Duration::from_millis(2)).await;
                stream.write_all(&bytes[1..]).await.unwrap();
            });
            let (actions,action_receiver)=tokio::sync::mpsc::unbounded_channel();
            let (event_sender,mut events)=tokio::sync::mpsc::unbounded_channel();
            let handler=NetworkingSystem::<NoPacketCallback>::create_map_server_packet_handler(NoPacketCallback,version).unwrap();
            let task=tokio::task::spawn_local(NetworkingSystem::<NoPacketCallback>::handle_server_connection(
                address,action_receiver,event_sender,handler, |_| RequestServerTickPacket::new(ClientTick(100)),
                Duration::from_secs(60),true,version,Arc::new(Mutex::new(TimeSynchronization::new())),
            ));
            assert!(matches!(tokio::time::timeout(Duration::from_secs(3),events.recv()).await.unwrap(),Some(NetworkEvent::AccountId{account_id:AccountId(42)})));
            assert!(matches!(tokio::time::timeout(Duration::from_secs(3),events.recv()).await.unwrap(),Some(NetworkEvent::ChatMessage{text,..}) if text.len()==20_000));
            assert!(matches!(tokio::time::timeout(Duration::from_secs(3),events.recv()).await.unwrap(),Some(NetworkEvent::DisplayEmotion{entity_id:EntityId(123),emotion:7})));
            drop(actions);server.await.unwrap();let _=task.await.unwrap();
        });
    }
    #[test]
    fn profile_20190530_retains_fragments_and_skips_only_exact_plugin_frames() {
        exercise(SupportedPacketVersion::_20190530);
    }
    #[test]
    fn profile_20220406_retains_fragmented_large_handled_frames() {
        exercise(SupportedPacketVersion::_20220406);
    }
    #[test]
    fn profile_20250716_retains_fragments_and_exact_plugin_boundaries() {
        exercise(SupportedPacketVersion::_20250716);
    }
}
