//! Client module - multi-transport distributed authority example.
//!
//! Connects via UDP, WebTransport, or WebSocket.
//! When the client gets authority over the ball, it sends updates back to the server.

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
    pub client_id: u64,
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
        // Input buffering
        app.add_systems(
            FixedPreUpdate,
            buffer_input.in_set(InputSystems::WriteClientInputs),
        );
        // Movement on predicted entities
        app.add_systems(FixedUpdate, player_movement);
        // Spawn handlers
        app.add_observer(handle_predicted_spawn);
        app.add_observer(handle_ball_spawn);
        app.add_systems(Update, (
            display_new_entities,
            log_authority_changes,
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

/// Connect to server via selected transport
fn startup(mut commands: Commands, config: Res<ClientConfig>) -> Result {
    let client_port = 4000 + (config.client_id % 100) as u16;
    let client_addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), client_port);

    match config.transport {
        Transport::Udp => {
            info!("📡 Connecting via UDP to {}", UDP_SERVER_ADDR);
            let auth = Authentication::Manual {
                server_addr: UDP_SERVER_ADDR,
                client_id: config.client_id,
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
                    ReplicationSender::new(  // Can send when we have authority
                        SERVER_REPLICATION_INTERVAL,
                        SendUpdatesMode::SinceLastAck,
                        false,
                    ),
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
                client_id: config.client_id,
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
                    ReplicationSender::new(
                        SERVER_REPLICATION_INTERVAL,
                        SendUpdatesMode::SinceLastAck,
                        false,
                    ),
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
                client_id: config.client_id,
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
                    ReplicationSender::new(
                        SERVER_REPLICATION_INTERVAL,
                        SendUpdatesMode::SinceLastAck,
                        false,
                    ),
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

/// Buffer keyboard input for sending to server
fn buffer_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut query: Query<&mut ActionState<Inputs>, With<InputMarker<Inputs>>>,
) {
    let direction = Direction {
        up: keyboard.pressed(KeyCode::KeyW) || keyboard.pressed(KeyCode::ArrowUp),
        down: keyboard.pressed(KeyCode::KeyS) || keyboard.pressed(KeyCode::ArrowDown),
        left: keyboard.pressed(KeyCode::KeyA) || keyboard.pressed(KeyCode::ArrowLeft),
        right: keyboard.pressed(KeyCode::KeyD) || keyboard.pressed(KeyCode::ArrowRight),
    };
    
    for mut action_state in query.iter_mut() {
        action_state.0 = Inputs::Direction(direction.clone());
    }
}

/// Move predicted player entities
fn player_movement(
    mut query: Query<(&mut Position, &ActionState<Inputs>), With<Predicted>>,
) {
    for (position, action_state) in query.iter_mut() {
        shared_movement_behaviour(position, &action_state.0);
    }
}

/// Handle when our player entity spawns as Predicted
fn handle_predicted_spawn(
    trigger: On<Add, Predicted>,
    mut query: Query<(Option<&PlayerId>, Option<&mut PlayerColor>)>,
    mut commands: Commands,
) {
    let entity = trigger.entity;
    if let Ok((player_id, color)) = query.get_mut(entity) {
        info!("🎯 Predicted spawn: entity {:?} (PlayerId: {:?})", entity, player_id);
        
        // Add InputMarker so inputs are sent for this entity
        commands.entity(entity).insert(InputMarker::<Inputs>::default());
        
        // Lower saturation for predicted entities to distinguish from confirmed
        if let Some(mut color) = color {
            let hsva = Hsva {
                saturation: 0.4,
                ..Hsva::from(color.0)
            };
            color.0 = Color::from(hsva);
        }
    }
}

/// Handle ball entity spawn - prepare for potential authority
fn handle_ball_spawn(trigger: On<Add, BallMarker>, mut commands: Commands) {
    let entity = trigger.entity;
    info!("⚽ Ball entity spawned: {:?}", entity);
    
    // Add Replicate::to_server() so that when we gain authority,
    // our ball updates get sent to the server
    // Also disable PlayerColor replication from client -> server
    // (we don't want the client's color to override the server's authority-based coloring)
    let mut color_override = ComponentReplicationOverrides::<PlayerColor>::default();
    color_override.global_override(ComponentReplicationOverride {
        disable: true,
        ..default()
    });
    
    commands.entity(entity).insert((
        Replicate::to_server(),
        color_override,
    ));
}

/// Display new entities as they appear
fn display_new_entities(
    players: Query<(Entity, &PlayerId, &Position, Has<Predicted>, Has<Interpolated>), Added<PlayerId>>,
    balls: Query<(Entity, &Position), Added<BallMarker>>,
) {
    for (entity, player_id, pos, is_predicted, is_interpolated) in players.iter() {
        let entity_type = if is_predicted { "Predicted" } else if is_interpolated { "Interpolated" } else { "Regular" };
        info!("🎮 {} player: entity {:?}, ID {:?}, pos ({:.0}, {:.0})", 
            entity_type, entity, player_id.0, pos.0.x, pos.0.y);
    }
    
    for (entity, pos) in balls.iter() {
        info!("⚽ Ball spawned: entity {:?} at ({:.0}, {:.0})", entity, pos.0.x, pos.0.y);
    }
}

/// Log when ball authority changes (visible through color changes)
fn log_authority_changes(
    balls: Query<(&PlayerColor, Has<HasAuthority>), (With<BallMarker>, Changed<PlayerColor>)>,
) {
    for (color, has_authority) in balls.iter() {
        let color_info = match color.0 {
            Color::Srgba(c) => format!("({:.1}, {:.1}, {:.1})", c.red, c.green, c.blue),
            _ => "unknown".to_string(),
        };
        let authority_status = if has_authority { "WE HAVE AUTHORITY" } else { "remote authority" };
        info!("🔄 Ball color changed to {} - {}", color_info, authority_status);
    }
}
