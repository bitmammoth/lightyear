use avian2d::prelude::*;
use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;
use bevy::time::common_conditions::on_timer;
use core::net::Ipv4Addr;
use core::time::Duration;
use leafwing_input_manager::prelude::*;
use lightyear::core::timeline::is_in_rollback;
use lightyear::input::input_buffer::InputBuffer;
use lightyear::netcode::Key;
use lightyear::prelude::client::*;
use lightyear::prelude::*;
use lightyear::prediction::manager::PredictionManager;
use lightyear::websocket::prelude::client::ClientConfig as WebSocketClientConfig;
use std::net::SocketAddr;

use crate::shared::FIXED_TIMESTEP_HZ;
use crate::protocol::*;
use crate::server::{UDP_PORT, WEBTRANSPORT_PORT, WEBSOCKET_PORT};
use crate::shared::*;

// Server addresses
pub const UDP_SERVER_ADDR: SocketAddr = SocketAddr::new(
    std::net::IpAddr::V4(Ipv4Addr::LOCALHOST),
    UDP_PORT,
);
pub const WEBTRANSPORT_SERVER_ADDR: SocketAddr = SocketAddr::new(
    std::net::IpAddr::V4(Ipv4Addr::LOCALHOST),
    WEBTRANSPORT_PORT,
);
pub const WEBSOCKET_SERVER_ADDR: SocketAddr = SocketAddr::new(
    std::net::IpAddr::V4(Ipv4Addr::LOCALHOST),
    WEBSOCKET_PORT,
);

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Transport {
    #[default]
    Udp,
    WebTransport,
    WebSocket,
    Local,
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
        // Multi-transport client startup
        app.add_systems(Startup, startup);
        
        // Debug logging
        app.add_systems(Update, debug_client_positions.run_if(on_timer(Duration::from_secs(2))));
        
        // Original spaceships client systems (unchanged)
        app.add_observer(add_ball_physics);
        app.add_observer(add_bullet_physics);
        app.add_observer(handle_new_player);

        app.add_systems(
            FixedUpdate,
            handle_hit_event
                .run_if(on_message::<BulletHitMessage>)
                .after(process_collisions),
        );
    }
}

/// Debug system to log all player positions on the client
fn debug_client_positions(
    players: Query<(Entity, &Player, &Position, Option<&Controlled>, Option<&Predicted>)>,
    balls: Query<(Entity, &BallMarker, &Position), With<Predicted>>,
    client_q: Query<&Client>,
) {
    let client_info = client_q.single()
        .map(|_| "connected".to_string())
        .unwrap_or("no client".to_string());
    
    info!("=== CLIENT POSITIONS ({}) ===", client_info);
    for (entity, player, pos, controlled, predicted) in players.iter() {
        let ctrl = controlled.map(|_| "CONTROLLED").unwrap_or("remote");
        let pred = predicted.map(|_| "Predicted").unwrap_or("not-predicted");
        info!("  Player {:?} '{}' pos=({:.1}, {:.1}) {} {}", 
            entity, player.nickname, pos.0.x, pos.0.y, ctrl, pred);
    }
    
    // Collect and sort balls by entity index for consistent ordering
    let mut ball_list: Vec<_> = balls.iter().collect();
    ball_list.sort_by_key(|(e, _, _)| e.index());
    
    info!("  Balls: {} total", ball_list.len());
    for (entity, _ball, pos) in ball_list.iter() {
        info!("    Ball {:?} pos=({:.1}, {:.1})", entity, pos.0.x, pos.0.y);
    }
}

/// Connect to server via UDP, WebTransport, or WebSocket
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
                    PredictionManager::default(),
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
            // Remove colons if present (server outputs colon-separated format)
            let cert_digest = cert_digest_raw.replace(":", "");
            let client = commands
                .spawn((
                    Client::default(),
                    LocalAddr(client_addr),
                    PeerAddr(WEBTRANSPORT_SERVER_ADDR),
                    Link::new(None),
                    ReplicationReceiver::default(),
                    PredictionManager::default(),
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
                    PredictionManager::default(),
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
            info!("🏠 Using Local connection (host-client mode)");
            // For local transport in host-client mode, we need crossbeam channels
            // This is a placeholder - local transport setup is more complex
            // and typically handled by the lightyear_examples_common::cli
            todo!("Local transport not yet implemented for multi-transport demo");
        }
    }
    
    info!("🔗 Connection initiated...\n");
    Ok(())
}

/// When the ball gets replicated from the server, add all the components
/// that we need that are not replicated.
/// (for example physical properties that are constant, so they don't need to be networked)
///
/// We only add the physical properties on the ball that is displayed on screen (i.e the Predicted ball)
/// We want the ball to be rigid so that when players collide with it, they bounce off.
fn add_ball_physics(
    trigger: On<Add, BallMarker>,
    ball_query: Query<&BallMarker, With<Predicted>>,
    mut commands: Commands,
) {
    let entity = trigger.entity;
    if let Ok(ball) = ball_query.get(entity) {
        info!("Adding physics to a replicated ball {entity:?}");
        commands.entity(entity).insert(ball.physics_bundle());
    }
}

/// Similar blueprint scenario as balls, except sometimes clients prespawn bullets ahead of server
/// replication, which means they will already have the physics components.
/// So, we filter the query using `Without<Collider>`.
fn add_bullet_physics(
    trigger: On<Add, BulletMarker>,
    mut commands: Commands,
    bullet_query: Query<(), (With<Predicted>, Without<Collider>)>,
) {
    let entity = trigger.entity;
    if let Ok(()) = bullet_query.get(entity) {
        info!("Adding physics to a replicated bullet: {entity:?}");
        commands.entity(entity).insert(PhysicsBundle::bullet());
    }
}

/// Decorate newly connecting players with physics components
/// ..and if it's our own player, set up input stuff
fn handle_new_player(
    trigger: On<Add, (Player, Predicted)>,
    mut commands: Commands,
    player_query: Query<(&Player, Has<Controlled>), With<Predicted>>,
) {
    let entity = trigger.entity;
    if let Ok((player, is_controlled)) = player_query.get(entity) {
        info!("handle_new_player, entity = {entity:?} is_controlled = {is_controlled}");
        // is this our own entity?
        if is_controlled {
            info!("Own player replicated to us, adding inputmap {entity:?} {player:?}");
            commands.entity(entity).insert(InputMap::new([
                (PlayerActions::Up, KeyCode::ArrowUp),
                (PlayerActions::Down, KeyCode::ArrowDown),
                (PlayerActions::Left, KeyCode::ArrowLeft),
                (PlayerActions::Right, KeyCode::ArrowRight),
                (PlayerActions::Up, KeyCode::KeyW),
                (PlayerActions::Down, KeyCode::KeyS),
                (PlayerActions::Left, KeyCode::KeyA),
                (PlayerActions::Right, KeyCode::KeyD),
                (PlayerActions::Fire, KeyCode::Space),
            ]));
        } else {
            info!("Remote player replicated to us: {entity:?} {player:?}");
        }
        commands.entity(entity).insert(PhysicsBundle::player_ship());
    }
}

// Generate an explosion effect for bullet collisions
fn handle_hit_event(
    time: Res<Time>,
    mut events: MessageReader<BulletHitMessage>,
    mut commands: Commands,
) {
    for ev in events.read() {
        commands.spawn((
            Transform::from_xyz(ev.position.x, ev.position.y, 0.0),
            Visibility::default(),
            crate::renderer::Explosion::new(time.elapsed(), ev.bullet_color),
        ));
    }
}
