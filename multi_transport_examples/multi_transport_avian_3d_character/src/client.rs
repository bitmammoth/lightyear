//! Client implementation with transport selection and avian3d physics

use crate::protocol::*;
use crate::shared::*;
use crate::TransportArg;
use avian3d::prelude::*;
use bevy::prelude::*;
use core::net::{Ipv4Addr, SocketAddr};
use core::time::Duration;
use leafwing_input_manager::prelude::*;
use lightyear::connection::client::{Connected, Connecting, Disconnected};
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
        app.add_systems(Startup, (setup_scene, spawn_connection, spawn_floor_local));
        app.add_observer(on_connecting);
        app.add_observer(on_connected);
        app.add_observer(on_disconnected);
        app.add_observer(handle_predicted_spawn);
        app.add_systems(FixedUpdate, player_movement);
        app.add_systems(Update, render_entities);
    }
}

fn setup_scene(mut commands: Commands) {
    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 15.0, 25.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Light
    commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(10.0, 20.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

/// Spawn local floor (not replicated)
fn spawn_floor_local(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(FLOOR_WIDTH, FLOOR_HEIGHT, FLOOR_WIDTH))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.3, 0.3, 0.3),
            ..default()
        })),
        Transform::from_xyz(0.0, -FLOOR_HEIGHT / 2.0, 0.0),
        FloorMarker,
        FloorPhysicsBundle::default(),
    ));
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

/// When predicted character spawns, add physics and input mappings
fn handle_predicted_spawn(
    trigger: On<Add, (CharacterMarker, Predicted)>,
    mut commands: Commands,
    player_query: Query<(&ColorComponent, Has<Controlled>), With<Predicted>>,
) {
    if let Ok((_color, controlled)) = player_query.get(trigger.entity) {
        let mut entity_mut = commands.entity(trigger.entity);
        entity_mut.insert(CharacterPhysicsBundle::default());
        
        if controlled {
            info!("🎮 Adding input mappings to controlled character {:?}", trigger.entity);
            entity_mut.insert(InputMap::new([
                (CharacterAction::Jump, KeyCode::Space),
            ]).with_dual_axis(
                CharacterAction::Move,
                VirtualDPad::wasd(),
            ));
        }
    }
}

/// Client-side movement - only on predicted entities we own
fn player_movement(
    spatial_query: SpatialQuery,
    mut query: Query<
        (Entity, &Position, &mut LinearVelocity, &ActionState<CharacterAction>),
        With<Predicted>,
    >,
) {
    for (entity, position, mut velocity, action) in query.iter_mut() {
        apply_character_action(entity, &mut velocity, &spatial_query, action, position);
    }
}

/// Render characters as 3D capsules
fn render_entities(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    characters: Query<(Entity, &ColorComponent), (With<CharacterMarker>, Without<Mesh3d>)>,
) {
    for (entity, color) in characters.iter() {
        commands.entity(entity).insert((
            Mesh3d(meshes.add(Capsule3d::new(CHARACTER_CAPSULE_RADIUS, CHARACTER_CAPSULE_HEIGHT))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: color.0,
                ..default()
            })),
        ));
    }
}
