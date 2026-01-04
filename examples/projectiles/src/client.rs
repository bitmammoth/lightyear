//! Client for projectiles with multi-transport support

use crate::protocol::*;
use crate::shared::*;
use crate::TransportArg;
use avian2d::prelude::*;
use bevy::prelude::*;
use bevy_enhanced_input::action::ActionMock;
use bevy_enhanced_input::prelude::*;
use core::net::{Ipv4Addr, SocketAddr};
use core::time::Duration;
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
        app.add_systems(Startup, (setup_camera, spawn_connection));
        app.add_observer(on_connecting);
        app.add_observer(on_connected);
        app.add_observer(on_disconnected);
        app.add_observer(handle_predicted_spawn);
        app.add_observer(add_global_actions);
        app.add_systems(Update, (render_entities, update_cursor_action));
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

/// When predicted player spawns, add input bindings
fn handle_predicted_spawn(
    trigger: On<Add, (PlayerMarker, Predicted)>,
    client: Single<&LocalId, With<Client>>,
    mut commands: Commands,
    player_query: Query<(&PlayerId, Has<Controlled>), With<Predicted>>,
) {
    let client_id = client.into_inner().0;
    if let Ok((player_id, controlled)) = player_query.get(trigger.entity) {
        if player_id.0 != client_id {
            return;
        }
        
        info!("🎮 Adding input bindings to predicted player {:?}", trigger.entity);
        add_player_actions(&mut commands, trigger.entity);
    }
}

fn add_player_actions(commands: &mut Commands, player: Entity) {
    commands.entity(player).insert(PlayerContext);
    
    // Movement (WASD)
    commands.spawn((
        ActionOf::<PlayerContext>::new(player),
        Action::<MovePlayer>::new(),
        Bindings::spawn(Cardinal::wasd_keys()),
    ));
    
    // Cursor tracking (mouse position)
    commands.spawn((
        ActionOf::<PlayerContext>::new(player),
        Action::<MoveCursor>::new(),
        ActionMock::new(
            ActionState::Fired,
            ActionValue::zero(ActionValueDim::Axis2D),
            MockSpan::Manual,
        ),
    ));
    
    // Shooting (Space)
    commands.spawn((
        ActionOf::<PlayerContext>::new(player),
        Action::<Shoot>::new(),
        Bindings::spawn_one((Binding::from(KeyCode::Space), Name::from("ShootBinding"))),
    ));
}

/// Add global actions for weapon switching
fn add_global_actions(trigger: On<Add, ClientContext>, mut commands: Commands) {
    commands.spawn((
        ActionOf::<ClientContext>::new(trigger.entity),
        Action::<CycleWeapon>::new(),
        Bindings::spawn_one((Binding::from(KeyCode::KeyQ), Name::from("CycleWeaponBinding"))),
    ));
}

/// Update cursor action with mouse position
fn update_cursor_action(
    window: Query<&Window>,
    camera: Query<(&Camera, &GlobalTransform)>,
    mut cursor_actions: Query<&mut ActionMock, With<Action<MoveCursor>>>,
) {
    let Ok(window) = window.single() else { return };
    let Ok((camera, camera_transform)) = camera.single() else { return };
    
    if let Some(cursor_pos) = window.cursor_position()
        .and_then(|p| camera.viewport_to_world_2d(camera_transform, p).ok())
    {
        for mut mock in cursor_actions.iter_mut() {
            mock.value = ActionValue::Axis2D(cursor_pos);
        }
    }
}

/// Render entities as colored sprites
fn render_entities(
    mut commands: Commands,
    players: Query<(Entity, &ColorComponent), (With<PlayerId>, Without<Sprite>)>,
    bullets: Query<(Entity, &ColorComponent), (With<BulletMarker>, Without<Sprite>)>,
    hitscans: Query<(Entity, &HitscanVisual, &ColorComponent), Without<Sprite>>,
) {
    // Render players
    for (entity, color) in players.iter() {
        commands.entity(entity).insert(Sprite {
            color: color.0,
            custom_size: Some(Vec2::splat(PLAYER_SIZE)),
            ..default()
        });
    }
    
    // Render bullets
    for (entity, color) in bullets.iter() {
        commands.entity(entity).insert(Sprite {
            color: color.0,
            custom_size: Some(Vec2::splat(BULLET_SIZE * 2.0)),
            ..default()
        });
    }
    
    // Render hitscan visuals as lines (simplified as sprites)
    for (entity, visual, color) in hitscans.iter() {
        let length = (visual.end - visual.start).length();
        let angle = (visual.end - visual.start).to_angle();
        let center = (visual.start + visual.end) / 2.0;
        
        commands.entity(entity).insert((
            Sprite {
                color: color.0.with_alpha(visual.lifetime / visual.max_lifetime),
                custom_size: Some(Vec2::new(2.0, length)),
                ..default()
            },
            Transform::from_xyz(center.x, center.y, 0.0)
                .with_rotation(Quat::from_rotation_z(angle - std::f32::consts::FRAC_PI_2)),
        ));
    }
}
