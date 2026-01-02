//! Client implementation with transport selection
//!
//! Connects to the server using the specified transport and sets up BEI inputs.

use crate::protocol::*;
use crate::shared::{self, *};
use crate::TransportArg;
use bevy::prelude::*;
use core::net::{Ipv4Addr, SocketAddr};
use core::time::Duration;
use lightyear::connection::client::{Connected, Connecting, Disconnected};
use lightyear::input::bei::prelude::{Action, ActionOf, Bindings, Cardinal, Fire};
use lightyear::netcode::Key;
use lightyear::prelude::client::*;
use lightyear::prelude::*;
use lightyear::websocket::prelude::client::ClientConfig as WebSocketClientConfig;

pub fn run_client(transport: TransportArg, cert: Option<String>) {
    let mut app = App::new();
    
    app.add_plugins(DefaultPlugins);
    
    // Lightyear client plugin
    app.add_plugins(lightyear::prelude::client::ClientPlugins {
        tick_duration: Duration::from_secs_f64(1.0 / FIXED_TIMESTEP_HZ),
    });
    
    // Shared protocol
    app.add_plugins(SharedPlugin);
    
    // Client-specific plugin
    app.add_plugins(ExampleClientPlugin);
    
    // Store config for startup
    app.insert_resource(ClientConfig {
        client_id: rand::random::<u64>(),
        transport,
        cert_digest: cert,
    });
    
    app.run();
}

#[derive(Resource)]
pub struct ClientConfig {
    pub client_id: u64,
    pub transport: TransportArg,
    pub cert_digest: Option<String>,
}

pub struct ExampleClientPlugin;

impl Plugin for ExampleClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (setup_camera, spawn_connection));
        app.add_observer(on_connecting);
        app.add_observer(on_connected);
        app.add_observer(on_disconnected);
        app.add_observer(handle_predicted_spawn);
        app.add_observer(handle_interpolated_spawn);
        app.add_observer(player_movement);
        app.add_systems(Update, (render_players, update_transforms));
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
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

fn spawn_connection(mut commands: Commands, config: Res<ClientConfig>) {
    let client_port = 4000 + (config.client_id % 100) as u16;
    let client_addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), client_port);
    
    match config.transport {
        TransportArg::Udp => {
            let server_addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), SERVER_UDP_PORT);
            info!("📡 Connecting via UDP to {}", server_addr);
            let auth = Authentication::Manual {
                server_addr,
                client_id: config.client_id,
                private_key: Key::default(),
                protocol_id: PROTOCOL_ID,
            };
            let client = commands
                .spawn((
                    Client::default(),
                    LocalAddr(client_addr),
                    PeerAddr(server_addr),
                    Link::new(None),
                    ReplicationReceiver::default(),
                    ReplicationSender::new(SERVER_REPLICATION_INTERVAL, SendUpdatesMode::SinceLastAck, false),
                    NetcodeClient::new(auth, NetcodeConfig::default()).unwrap(),
                    UdpIo::default(),
                    Name::new("UdpClient"),
                ))
                .id();
            commands.trigger(Connect { entity: client });
        }
        TransportArg::Webtransport => {
            let server_addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), SERVER_WEBTRANSPORT_PORT);
            info!("🌐 Connecting via WebTransport to {}", server_addr);
            let auth = Authentication::Manual {
                server_addr,
                client_id: config.client_id,
                private_key: Key::default(),
                protocol_id: PROTOCOL_ID,
            };
            let cert_digest_raw = config.cert_digest.clone().expect("WebTransport requires --cert <digest>");
            let cert_digest = cert_digest_raw.replace(":", "");
            let client = commands
                .spawn((
                    Client::default(),
                    LocalAddr(client_addr),
                    PeerAddr(server_addr),
                    Link::new(None),
                    ReplicationReceiver::default(),
                    ReplicationSender::new(SERVER_REPLICATION_INTERVAL, SendUpdatesMode::SinceLastAck, false),
                    NetcodeClient::new(auth, NetcodeConfig::default()).unwrap(),
                    WebTransportClientIo {
                        certificate_digest: cert_digest,
                    },
                    Name::new("WebTransportClient"),
                ))
                .id();
            commands.trigger(Connect { entity: client });
        }
        TransportArg::Websocket => {
            let server_addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), SERVER_WEBSOCKET_PORT);
            info!("🔌 Connecting via WebSocket to {}", server_addr);
            let auth = Authentication::Manual {
                server_addr,
                client_id: config.client_id,
                private_key: Key::default(),
                protocol_id: PROTOCOL_ID,
            };
            let client = commands
                .spawn((
                    Client::default(),
                    LocalAddr(client_addr),
                    PeerAddr(server_addr),
                    Link::new(None),
                    ReplicationReceiver::default(),
                    ReplicationSender::new(SERVER_REPLICATION_INTERVAL, SendUpdatesMode::SinceLastAck, false),
                    NetcodeClient::new(auth, NetcodeConfig::default()).unwrap(),
                    WebSocketClientIo {
                        config: WebSocketClientConfig::builder().with_no_cert_validation(),
                        scheme: WebSocketScheme::Secure,
                    },
                    Name::new("WebSocketClient"),
                ))
                .id();
            commands.trigger(Connect { entity: client });
        }
    }
    
    info!("🔗 Connection initiated...\n");
}

/// When predicted player spawns, add BEI input bindings if we control it
fn handle_predicted_spawn(
    trigger: On<Add, (PlayerId, Predicted)>,
    mut predicted: Query<(&mut PlayerColor, Has<Controlled>), With<Predicted>>,
    mut commands: Commands,
) {
    let entity = trigger.entity;
    if let Ok((mut color, controlled)) = predicted.get_mut(entity) {
        // Make predicted entities slightly desaturated
        let hsva = Hsva {
            saturation: 0.4,
            ..Hsva::from(color.0)
        };
        color.0 = Color::from(hsva);
        
        if controlled {
            info!("🎮 Adding BEI input bindings to controlled player {:?}", entity);
            // Add Action entities for BEI input
            commands.spawn((
                ActionOf::<Player>::new(entity),
                Action::<Movement>::new(),
                Bindings::spawn(Cardinal::wasd_keys()),
            ));
        }
    }
}

/// Adjust color for interpolated entities
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
    }
}

/// Client-side movement - only applies to predicted entities we own
fn player_movement(
    trigger: On<Fire<Movement>>,
    mut position_query: Query<&mut PlayerPosition, With<Predicted>>,
) {
    if let Ok(position) = position_query.get_mut(trigger.context) {
        shared::shared_movement_behaviour(position, trigger.value);
    }
}

/// Render players as colored sprites
fn render_players(
    mut commands: Commands,
    players: Query<(Entity, &PlayerPosition, &PlayerColor), (Without<Transform>,)>,
) {
    for (entity, position, color) in players.iter() {
        commands.entity(entity).insert((
            Sprite {
                color: color.0,
                custom_size: Some(Vec2::splat(40.0)),
                ..default()
            },
            Transform::from_translation(position.0.extend(0.0)),
        ));
    }
}

/// Update transforms based on position
fn update_transforms(
    mut players: Query<(&PlayerPosition, &mut Transform), Changed<PlayerPosition>>,
) {
    for (position, mut transform) in players.iter_mut() {
        transform.translation = position.0.extend(0.0);
    }
}
