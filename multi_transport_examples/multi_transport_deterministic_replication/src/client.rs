//! Client for deterministic replication with multi-transport support
//!
//! All clients simulate all players deterministically.
//! Rollbacks only happen when input predictions don't match.

use crate::protocol::*;
use crate::shared::*;
use crate::TransportArg;
use avian2d::prelude::*;
use bevy::prelude::*;
use core::net::{Ipv4Addr, SocketAddr};
use core::time::Duration;
use leafwing_input_manager::prelude::*;
use lightyear::connection::client::{Connected, Connecting, Disconnected};
use lightyear::netcode::Key;
use lightyear::prediction::rollback::DeterministicPredicted;
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
    
    // Shared protocol + physics
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
        app.add_systems(Startup, (setup_camera, spawn_connection, spawn_walls_local));
        app.add_observer(on_connecting);
        app.add_observer(on_connected);
        app.add_observer(on_disconnected);
        app.add_observer(handle_player_spawn);
        app.add_observer(handle_ball_spawn);
        app.add_systems(FixedUpdate, player_movement);
        app.add_systems(Update, render_entities);
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

/// Spawn local copies of walls (they're not replicated)
fn spawn_walls_local(mut commands: Commands) {
    spawn_walls(&mut commands);
}

fn on_connecting(trigger: On<Add, Connecting>, names: Query<&Name>) {
    if let Ok(name) = names.get(trigger.entity) {
        info!("🔄 {} is connecting...", name);
    }
}

fn on_connected(
    trigger: On<Add, Connected>,
    names: Query<&Name>,
    mut commands: Commands,
    client_query: Query<Entity, With<Client>>,
) {
    if let Ok(name) = names.get(trigger.entity) {
        info!("✅ {} connected to server!", name);
    }
    
    // Configure deterministic prediction on connect
    if let Ok(client) = client_query.single() {
        commands
            .entity(client)
            .insert(PredictionManager {
                rollback_policy: RollbackPolicy {
                    // we only replicate inputs, so state-based rollback is disabled
                    state: RollbackMode::Disabled,
                    // we rollback only when remote inputs don't match what we were predicting
                    input: RollbackMode::Check,
                    // do not limit the max number of rollback ticks
                    max_rollback_ticks: 100,
                },
                ..default()
            })
            .insert(
                InputTimelineConfig::default()
                    // Control the input delay - higher values = less rollback, more latency
                    .with_input_delay(InputDelayConfig::fixed_input_delay(INPUT_DELAY_TICKS)),
            );
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

/// When we receive a new player entity, set up deterministic prediction
fn handle_player_spawn(
    trigger: On<Add, PlayerId>,
    mut commands: Commands,
    player_query: Query<(&PlayerId, Has<Controlled>)>,
) {
    let Ok((player_id, controlled)) = player_query.get(trigger.entity) else {
        return;
    };
    let peer_id = player_id.0;
    info!("Received player spawn for player {peer_id:?}");

    let mut entity_mut = commands.entity(trigger.entity);
    
    // Add physics bundle
    entity_mut.insert((
        player_bundle(peer_id),
        // This indicates that the entity will only do rollbacks from input updates, not state updates!
        // REQUIRED for deterministic replication mode.
        DeterministicPredicted {
            // Prevents instant despawn if entity was spawned after rollback tick
            skip_despawn: true,
            ..default()
        },
    ));

    // Add input map for the local (controlled) player
    if controlled {
        info!("🎮 Adding input mappings to controlled player {:?}", trigger.entity);
        entity_mut.insert(InputMap::new([
            (PlayerActions::Up, KeyCode::KeyW),
            (PlayerActions::Down, KeyCode::KeyS),
            (PlayerActions::Left, KeyCode::KeyA),
            (PlayerActions::Right, KeyCode::KeyD),
        ]));
    }
}

/// Add physics and deterministic prediction to ball
fn handle_ball_spawn(
    trigger: On<Add, BallMarker>,
    mut commands: Commands,
) {
    commands.entity(trigger.entity).insert((
        PhysicsBundle::ball(),
        DeterministicPredicted {
            skip_despawn: true,
            ..default()
        },
    ));
}

/// In deterministic replication, all clients simulate all players
fn player_movement(
    timeline: Res<LocalTimeline>,
    mut velocity_query: Query<(
        Entity,
        &PlayerId,
        &Position,
        &mut LinearVelocity,
        &ActionState<PlayerActions>,
    )>,
) {
    let tick = timeline.tick();
    for (entity, player_id, position, velocity, action_state) in velocity_query.iter_mut() {
        if !action_state.get_pressed().is_empty() {
            trace!(?entity, ?tick, ?position, actions = ?action_state.get_pressed(), "applying movement to player");
            // note that we apply input to ALL players, even remote ones
            shared_movement_behaviour(velocity, action_state);
        }
    }
}

/// Render entities as colored sprites
fn render_entities(
    mut commands: Commands,
    players: Query<(Entity, &ColorComponent), (With<PlayerId>, Without<Sprite>)>,
    balls: Query<(Entity, &ColorComponent), (With<BallMarker>, Without<Sprite>)>,
    walls: Query<(Entity, &ColorComponent, &Collider), (Without<PlayerId>, Without<BallMarker>, Without<Sprite>)>,
) {
    // Render players
    for (entity, color) in players.iter() {
        commands.entity(entity).insert(Sprite {
            color: color.0,
            custom_size: Some(Vec2::splat(PLAYER_SIZE)),
            ..default()
        });
    }
    
    // Render balls
    for (entity, color) in balls.iter() {
        commands.entity(entity).insert(Sprite {
            color: color.0,
            custom_size: Some(Vec2::splat(BALL_SIZE * 2.0)),
            ..default()
        });
    }
    
    // Render walls
    for (entity, color, _collider) in walls.iter() {
        commands.entity(entity).insert(Sprite {
            color: color.0,
            custom_size: Some(Vec2::new(10.0, 700.0)),
            ..default()
        });
    }
}
