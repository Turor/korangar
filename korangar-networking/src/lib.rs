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
pub use self::event::{DisconnectReason, NetworkEvent, SearchStoreHit, StatusChangeState};
pub use self::hotkey::HotkeyState;
pub use self::items::{InventoryItem, InventoryItemDetails, ItemQuantity, NoMetadata, SellItem, ShopItem};
pub use self::message::MessageColor;
pub use self::packet_versions::SupportedPacketVersion;
pub use self::packet_versions::enchant::{format_enchant_result, parse_enchant_index, parse_enchant_item_id, parse_enchant_slot};
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
                    bytes[..4].copy_from_slice(&[0xf5,0x0e,0x20,0x4e]);
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
