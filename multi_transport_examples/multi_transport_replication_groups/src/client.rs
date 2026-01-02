//! Client module - connects to server and receives replicated player + trail groups.

use crate::protocol::*;
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
use lightyear::websocket::prelude::client::ClientConfig as WebSocketClientConfig;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Transport {
    #[default]
    Udp,
    WebTransport,
    WebSocket,
}

#[derive(Resource)]
pub struct ClientConfig {
    pub transport: Transport,
    pub cert_digest: Option<String>,
}

pub struct ExampleClientPlugin;

impl Plugin for ExampleClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, startup);
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
            display_new_trails,
        ));
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

/// Connect to server via UDP, WebTransport, or WebSocket
fn startup(mut commands: Commands, config: Res<ClientConfig>) -> Result {
    // Generate a random client ID
    let client_id: u64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64 % 100000;
    
    let client_port = 4000 + (client_id % 100) as u16;
    let client_addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), client_port);

    match config.transport {
        Transport::Udp => {
            info!("📡 Connecting via UDP to {}", UDP_SERVER_ADDR);
            let auth = Authentication::Manual {
                server_addr: UDP_SERVER_ADDR,
                client_id,
                private_key: Key::default(),
                protocol_id: 0,
            };
            let client = commands
                .spawn((
                    Client::default(),
                    LocalAddr(client_addr),
                    PeerAddr(UDP_SERVER_ADDR),
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
            info!("🌐 Connecting via WebTransport to {}", WEBTRANSPORT_SERVER_ADDR);
            let auth = Authentication::Manual {
                server_addr: WEBTRANSPORT_SERVER_ADDR,
                client_id,
                private_key: Key::default(),
                protocol_id: 0,
            };
            let cert_digest_raw = config.cert_digest.clone().expect("WebTransport requires cert_digest");
            let cert_digest = cert_digest_raw.replace(":", "");
            let client = commands
                .spawn((
                    Client::default(),
                    LocalAddr(client_addr),
                    PeerAddr(WEBTRANSPORT_SERVER_ADDR),
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
            info!("🔌 Connecting via WebSocket to {}", WEBSOCKET_SERVER_ADDR);
            let auth = Authentication::Manual {
                server_addr: WEBSOCKET_SERVER_ADDR,
                client_id,
                private_key: Key::default(),
                protocol_id: 0,
            };
            let client = commands
                .spawn((
                    Client::default(),
                    LocalAddr(client_addr),
                    PeerAddr(WEBSOCKET_SERVER_ADDR),
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
    }
    
    info!("🔗 Connection initiated...\n");
    Ok(())
}

/// Display information about newly replicated player entities
fn display_new_players(
    players: Query<(Entity, &PlayerId, &PlayerPosition, &PlayerColor, Has<Predicted>, Has<Interpolated>), Added<PlayerId>>,
) {
    for (entity, player_id, pos, _color, is_predicted, is_interpolated) in players.iter() {
        let entity_type = if is_predicted { "Predicted" } else if is_interpolated { "Interpolated" } else { "Regular" };
        info!(
            "🎮 {} player head spawned: entity {:?} (ID: {:?}) at ({:.1}, {:.1})",
            entity_type, entity, player_id.0, pos.0.x, pos.0.y
        );
    }
}

/// Display information about newly replicated trail entities
fn display_new_trails(
    trails: Query<(Entity, &TrailPosition, &PlayerParent, Has<Predicted>, Has<Interpolated>), Added<TrailPosition>>,
) {
    for (entity, pos, parent, is_predicted, is_interpolated) in trails.iter() {
        let entity_type = if is_predicted { "Predicted" } else if is_interpolated { "Interpolated" } else { "Regular" };
        info!(
            "📍 {} trail spawned: entity {:?} at ({:.1}, {:.1}) following {:?}",
            entity_type, entity, pos.0.x, pos.0.y, parent.0
        );
    }
}

/// System that reads from peripherals and adds inputs to the buffer
fn buffer_input(
    mut query: Query<(Entity, &mut ActionState<Inputs>), With<InputMarker<Inputs>>>,
    keypress: Res<ButtonInput<KeyCode>>,
) {
    for (_entity, mut action_state) in query.iter_mut() {
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
        action_state.0 = Inputs::Direction(direction);
    }
}

/// Apply movement to predicted entities we own
fn player_movement(
    mut position_query: Query<(&mut PlayerPosition, &ActionState<Inputs>), With<Predicted>>,
) {
    for (position, input) in position_query.iter_mut() {
        shared_movement_behaviour(position, input);
    }
}

/// When predicted copy of player spawns, add InputMarker and adjust color
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
        info!("🎮 Predicted player head spawned: {:?}, adding InputMarker", entity);
        commands.entity(entity).insert(InputMarker::<Inputs>::default());
    }
}

/// When interpolated copy of other players' entities spawns, adjust color
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
