//! Client module - connects to server and receives replicated player entities.
//!

use crate::shared::*;
use bevy::prelude::*;
use core::net::Ipv4Addr;
use std::net::SocketAddr;
use lightyear::connection::client::{Connected, Disconnected, Connecting};
use lightyear::netcode::Key;
use lightyear::prelude::client::*;
use lightyear::prelude::client::input::*;
use lightyear::prelude::input::native::*;
use lightyear::prelude::*;
use lightyear::input::input_buffer::InputBuffer;
use lightyear::websocket::prelude::client::ClientConfig as WebSocketClientConfig;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Transport {
    #[default]
    Udp,
    WebTransport,
    WebSocket,
    /// Local transport for host-client mode (uses in-memory channels)
    Local,
}

#[derive(Resource)]
pub struct ClientConfig {
    pub client_id: u64,
    pub transport: Transport,
    pub cert_digest: Option<String>,
    /// Optional server address override. If None, uses the default addresses from shared.rs
    pub server_addr: Option<SocketAddr>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            client_id: 0,
            transport: Transport::Udp,
            cert_digest: None,
            server_addr: None,
        }
    }
}

pub struct ExampleClientPlugin;

impl Plugin for ExampleClientPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ClientMessageTimer>();
        // Don't add startup system here - it will be added by main.rs with proper ordering
        // for host-client mode, or run standalone for client-only mode
        app.add_observer(on_connecting);
        app.add_observer(on_connected);
        app.add_observer(on_disconnected);
        // Input buffering must be in WriteClientInputs set
        app.add_systems(
            FixedPreUpdate,
            buffer_input.in_set(InputSystems::WriteClientInputs),
        );
        // Movement in FixedUpdate for determinism (on predicted entities)
        app.add_systems(FixedUpdate, player_movement);
        // Predicted/Interpolated spawn handlers
        app.add_observer(handle_predicted_spawn);
        app.add_observer(handle_interpolated_spawn);
        app.add_systems(Update, (
            display_new_players,
            count_replicated_entities,
            auto_send_messages,
            receive_server_messages,
            debug_input_sync_status,
        ));
    }
}

/// Timer for automatic message testing from client
#[derive(Resource)]
struct ClientMessageTimer {
    timer: Timer,
    phase: u32,
}

impl Default for ClientMessageTimer {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(5.0, TimerMode::Repeating),
            phase: 0,
        }
    }
}

fn on_connecting(trigger: On<Add, Connecting>, names: Query<&Name>) {
    if let Ok(name) = names.get(trigger.entity) {
        info!("🔄 {} is connecting...", name);
    }
}

fn on_connected(trigger: On<Add, Connected>, names: Query<&Name>) {
    if let Ok(name) = names.get(trigger.entity) {
        info!("✅ {} connected to server!", name);
    }
}

fn on_disconnected(trigger: On<Add, Disconnected>, names: Query<&Name>) {
    if let Ok(name) = names.get(trigger.entity) {
        info!("❌ {} disconnected from server", name);
    }
}

/// Connect to server via UDP, WebTransport, WebSocket, or Local channels
pub fn startup(mut commands: Commands, config: Res<ClientConfig>, server_query: Query<Entity, With<lightyear::prelude::server::Server>>) -> Result {
    // For Local transport (host-client mode), we connect to the server entity directly
    if config.transport == Transport::Local {
        // Find the server entity
        let server = server_query.single()?;
        
        info!("🏠 Connecting via Local channels to server {:?}", server);
        let client = commands
            .spawn((
                Client::default(),
                Name::new("HostClient"),
                LinkOf { server },
            ))
            .id();
        
        // Trigger Connect to establish the connection
        commands.trigger(Connect { entity: client });
        info!("🔗 Host-client connection initiated...\n");
        return Ok(());
    }
    
    let client_port = 4000 + (config.client_id % 100) as u16;
    let client_addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), client_port);

    // Use provided server_addr or fall back to defaults
    let server_addr = config.server_addr.unwrap_or_else(|| {
        match config.transport {
            Transport::Udp => UDP_SERVER_ADDR,
            Transport::WebTransport => WEBTRANSPORT_SERVER_ADDR,
            Transport::WebSocket => WEBSOCKET_SERVER_ADDR,
            Transport::Local => unreachable!(), // Handled above
        }
    });

    match config.transport {
        Transport::Udp => {
            info!("📡 Connecting via UDP to {}", server_addr);
            let auth = Authentication::Manual {
                server_addr,
                client_id: config.client_id,
                private_key: Key::default(),
                protocol_id: 0,
            };
            let client = commands
                .spawn((
                    Client::default(),
                    LocalAddr(client_addr),
                    PeerAddr(server_addr),
                    Link::new(None),
                    ReplicationReceiver::default(),
                    NetcodeClient::new(auth, NetcodeConfig::default())?,
                    UdpIo::default(),
                    Name::new("UdpClient"),
                ))
                .id();
            commands.trigger(Connect { entity: client });
        }
        Transport::WebTransport => {
            info!("🌐 Connecting via WebTransport to {}", server_addr);
            let auth = Authentication::Manual {
                server_addr,
                client_id: config.client_id,
                private_key: Key::default(),
                protocol_id: 0,
            };
            let cert_digest_raw = config.cert_digest.clone().expect("WebTransport requires cert_digest");
            // Remove colons if present (server outputs colon-separated format)
            let cert_digest = cert_digest_raw.replace(":", "");
            let client = commands
                .spawn((
                    Client::default(),
                    LocalAddr(client_addr),
                    PeerAddr(server_addr),
                    Link::new(None),
                    ReplicationReceiver::default(),
                    NetcodeClient::new(auth, NetcodeConfig::default())?,
                    WebTransportClientIo {
                        certificate_digest: cert_digest,
                    },
                    Name::new("WebTransportClient"),
                ))
                .id();
            commands.trigger(Connect { entity: client });
        }
        Transport::WebSocket => {
            info!("🔌 Connecting via WebSocket to {}", server_addr);
            let auth = Authentication::Manual {
                server_addr,
                client_id: config.client_id,
                private_key: Key::default(),
                protocol_id: 0,
            };
            let client = commands
                .spawn((
                    Client::default(),
                    LocalAddr(client_addr),
                    PeerAddr(server_addr),
                    Link::new(None),
                    ReplicationReceiver::default(),
                    NetcodeClient::new(auth, NetcodeConfig::default())?,
                    WebSocketClientIo {
                        config: WebSocketClientConfig::builder()
                            .with_no_cert_validation(),
                        scheme: WebSocketScheme::Secure,
                    },
                    Name::new("WebSocketClient"),
                ))
                .id();
            commands.trigger(Connect { entity: client });
        }
        Transport::Local => {
            // Local transport is handled at the start of this function
            unreachable!("Local transport should have returned early");
        }
    }
    
    info!("🔗 Connection initiated...\n");
    Ok(())
}

/// Display information about newly replicated player entities
fn display_new_players(
    players: Query<(Entity, &PlayerId, &PlayerPosition, &PlayerColor, &Name, Has<Predicted>, Has<Interpolated>), Added<PlayerId>>,
) {
    for (entity, player_id, pos, color, name, is_predicted, is_interpolated) in players.iter() {
        let color_rgb = match color.0 {
            Color::Srgba(c) => format!("({:.1}, {:.1}, {:.1})", c.red, c.green, c.blue),
            _ => "unknown".to_string(),
        };
        let entity_type = if is_predicted { "Predicted" } else if is_interpolated { "Interpolated" } else { "Regular" };
        info!(
            "🎮 {} player spawned: {} entity {:?} (ID: {:?}) at ({:.1}, {:.1}) color: {}",
            entity_type, name, entity, player_id.0, pos.0.x, pos.0.y, color_rgb
        );
    }
}

/// Log the count of replicated entities when it changes
fn count_replicated_entities(
    players: Query<Entity, With<PlayerId>>,
    mut last_count: Local<usize>,
) {
    let count = players.iter().count();
    if count != *last_count {
        info!("📊 Total replicated players: {}", count);
        *last_count = count;
    }
}

/// Automatically send test messages on a timer
fn auto_send_messages(
    time: Res<Time>,
    mut timer: ResMut<ClientMessageTimer>,
    config: Res<ClientConfig>,
    mut client_sender_query: Query<&mut MessageSender<ClientToServerMessage>, (With<Client>, With<Connected>)>,
    mut forward_query: Query<&mut MessageSender<ForwardMessage>, (With<Client>, With<Connected>)>,
    players: Query<(), With<PlayerId>>,
) {
    timer.timer.tick(time.delta());

    if !timer.timer.just_finished() {
        return;
    }

    // Only test when we have 3 players replicated (all transports connected)
    if players.iter().count() < 3 {
        return;
    }
    
    let (my_name, targets) = match config.transport {
        Transport::Udp => ("UDP", vec!["WT", "WS"]),
        Transport::WebTransport => ("WT", vec!["UDP", "WS"]),
        Transport::WebSocket => ("WS", vec!["UDP", "WT"]),
        Transport::Local => ("Host", vec!["UDP", "WT", "WS"]),
    };

    timer.phase = (timer.phase + 1) % 2;
    
    match timer.phase {
        0 => {
            // Send message to server
            for mut sender in client_sender_query.iter_mut() {
                sender.send::<DefaultChannel>(ClientToServerMessage {
                    content: format!("Hello Server from {}!", my_name),
                });
            }
        }
        1 => {
            // Forward message to ALL other clients via server
            for target in &targets {
                info!("📤 {} -> {} (via SERVER): Hello!", my_name, target);
                for mut sender in forward_query.iter_mut() {
                    sender.send::<DefaultChannel>(ForwardMessage {
                        from_name: my_name.to_string(),
                        target_name: target.to_string(),
                        content: format!("Hello from {}!", my_name),
                        is_reply: false,
                    });
                }
            }
        }
        _ => {}
    }
}

/// Receive messages from server
fn receive_server_messages(
    mut direct_receiver: Query<&mut MessageReceiver<ServerToClientMessage>, With<Client>>,
    mut broadcast_receiver: Query<&mut MessageReceiver<BroadcastMessage>, With<Client>>,
    mut forwarded_receiver: Query<&mut MessageReceiver<ForwardedMessage>, With<Client>>,
    mut forward_sender: Query<&mut MessageSender<ForwardMessage>, (With<Client>, With<Connected>)>,
    config: Res<ClientConfig>,
) {
    let my_name = match config.transport {
        Transport::Udp => "UDP",
        Transport::WebTransport => "WT",
        Transport::WebSocket => "WS",
        Transport::Local => "Host",
    };

    for mut receiver in direct_receiver.iter_mut() {
        for msg in receiver.receive() {
            info!("📥 {} received: {}", my_name, msg.content);
        }
    }

    for mut receiver in broadcast_receiver.iter_mut() {
        for msg in receiver.receive() {
            info!("📢 {} received broadcast: {}", my_name, msg.content);
        }
    }

    // Handle forwarded messages and send replies (only to non-replies)
    for mut receiver in forwarded_receiver.iter_mut() {
        for msg in receiver.receive() {
            if msg.is_reply {
                info!("↩️  {} got reply from {}: {}", my_name, msg.from_name, msg.content);
            } else {
                info!("📨 {} received from {}: {}", my_name, msg.from_name, msg.content);
                
                // Send a reply back to the sender
                info!("↩️  {} replying to {}", my_name, msg.from_name);
                for mut sender in forward_sender.iter_mut() {
                    sender.send::<DefaultChannel>(ForwardMessage {
                        from_name: my_name.to_string(),
                        target_name: msg.from_name.clone(),
                        content: format!("Reply from {} - got your message!", my_name),
                        is_reply: true,
                    });
                }
            }
        }
    }
}

/// System that reads from peripherals and adds inputs to the buffer
/// Must run in InputSystems::WriteClientInputs set in FixedPreUpdate
fn buffer_input(
    mut query: Query<(Entity, &mut ActionState<Inputs>), With<InputMarker<Inputs>>>,
    keypress: Res<ButtonInput<KeyCode>>,
    mut logged: Local<bool>,
) {
    // Log once when we find entities
    if !*logged {
        let count = query.iter().count();
        if count > 0 {
            info!("🔍 Client buffer_input found {} entities with InputMarker + ActionState", count);
            *logged = true;
        }
    }

    for (entity, mut action_state) in query.iter_mut() {
        let mut direction = Direction {
            up: false,
            down: false,
            left: false,
            right: false,
        };
        if keypress.pressed(KeyCode::KeyW) || keypress.pressed(KeyCode::ArrowUp) {
            direction.up = true;
        }
        if keypress.pressed(KeyCode::KeyS) || keypress.pressed(KeyCode::ArrowDown) {
            direction.down = true;
        }
        if keypress.pressed(KeyCode::KeyA) || keypress.pressed(KeyCode::ArrowLeft) {
            direction.left = true;
        }
        if keypress.pressed(KeyCode::KeyD) || keypress.pressed(KeyCode::ArrowRight) {
            direction.right = true;
        }
        // Always set the value - None means missing input, not "no keys pressed"
        action_state.0 = Inputs::Direction(direction.clone());
        if !direction.is_none() {
            info!("📝 Client buffering input for entity {:?}: {:?}", entity, direction);
        }
    }
}

/// Apply movement to predicted entities we own
fn player_movement(
    mut position_query: Query<(&mut PlayerPosition, &ActionState<Inputs>), With<Predicted>>,
) {
    for (position, input) in position_query.iter_mut() {
        // Note: pass Mut<PlayerPosition> directly, getting &mut triggers change detection
        shared_movement_behaviour(position, input);
    }
}

/// When predicted copy of client-owned entity spawns:
/// - Change saturation to differentiate predicted from server
/// - Add InputMarker (ActionState and InputBuffer are added automatically via required components)
///
/// Note: We don't need to check for `Controlled` because only OUR player is Predicted
/// (other players are Interpolated via InterpolationTarget). This matches simple_box pattern.
fn handle_predicted_spawn(
    trigger: On<Add, PlayerId>,
    mut predicted: Query<&mut PlayerColor, With<Predicted>>,
    mut commands: Commands,
) {
    let entity = trigger.entity;
    if let Ok(mut color) = predicted.get_mut(entity) {
        let hsva = Hsva {
            saturation: 0.4,
            ..Hsva::from(color.0)
        };
        color.0 = Color::from(hsva);
        info!("🎮 Predicted entity spawned: {:?}, adding InputMarker", entity);
        commands.entity(entity).insert(InputMarker::<Inputs>::default());
    }
}

/// When interpolated copy of other players' entities spawns:
/// - Change saturation to differentiate interpolated from server
fn handle_interpolated_spawn(
    trigger: On<Add, PlayerColor>,
    mut interpolated: Query<&mut PlayerColor, With<Interpolated>>,
) {
    if let Ok(mut color) = interpolated.get_mut(trigger.entity) {
        let hsva = Hsva {
            saturation: 0.1,
            ..Hsva::from(color.0)
        };
        color.0 = Color::from(hsva);
        info!("👤 Interpolated entity spawned: {:?}", trigger.entity);
    }
}

/// Debug system to monitor input sync status
fn debug_input_sync_status(
    client_query: Query<(
        Entity,
        Has<InputTimeline>,
        Has<IsSynced<InputTimeline>>,
        Has<Connected>,
    ), With<Client>>,
    input_entities: Query<(
        Entity,
        Has<InputMarker<Inputs>>,
        Has<ActionState<Inputs>>,
        Option<&InputBuffer<ActionState<Inputs>, Inputs>>,
    ), With<Predicted>>,
    mut timer: Local<Option<Timer>>,
    time: Res<Time>,
) {
    let timer = timer.get_or_insert_with(|| Timer::from_seconds(2.0, TimerMode::Repeating));
    timer.tick(time.delta());

    if timer.just_finished() {
        for (entity, has_input_timeline, has_is_synced, has_connected) in client_query.iter() {
            info!(
                "🔄 Client {:?} - InputTimeline: {}, IsSynced: {}, Connected: {}",
                entity, has_input_timeline, has_is_synced, has_connected
            );
        }

        for (entity, has_marker, has_state, input_buffer) in input_entities.iter() {
            let buffer_info = input_buffer.map(|b| format!("start={:?}, len={}", b.start_tick, b.len())).unwrap_or("None".to_string());
            info!(
                "🎮 Predicted {:?} - InputMarker: {}, ActionState: {}, InputBuffer: {}",
                entity, has_marker, has_state, buffer_info
            );
        }
    }
}
