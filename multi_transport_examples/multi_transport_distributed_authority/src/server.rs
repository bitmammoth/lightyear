//! Server module - multi-transport distributed authority example.
//!
//! Runs UDP, WebTransport, and WebSocket transports.
//! Spawns a ball entity with authority transfer - when a player gets close,
//! authority transfers to them, and they control the ball.
//!
//! Architecture:
//! - ONE Server entity (logical server, relationship target)
//! - Three transport entities (UDP, WebTransport, WebSocket) with TransportOf
//! - Ball entity with dynamic authority (can be Server, or any Client)
//! - When authority transfers to a client, that client's ball updates are replicated back to server

use crate::protocol::*;
use crate::shared::*;
use bevy::prelude::*;
use core::net::Ipv4Addr;
use core::time::Duration;
use std::collections::HashMap;
use std::net::SocketAddr;
use lightyear::prelude::server::*;
use lightyear::prelude::*;
use lightyear::prelude::input::native::ActionState;
use lightyear::link::prelude::ViaTransport;
use lightyear::connection::server::Started;

pub struct ExampleServerPlugin;

impl Plugin for ExampleServerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerRegistry>();
        app.add_systems(Startup, (startup, spawn_ball));
        app.add_observer(handle_new_client);
        app.add_observer(handle_connected);
        app.add_observer(handle_disconnected);
        // Movement and authority transfer in FixedUpdate
        app.add_systems(FixedUpdate, player_movement);
        app.add_systems(Update, (
            transfer_authority,
            update_ball_color,
            log_server_status,
        ));
    }
}

/// Track connected players
#[derive(Resource, Default)]
struct PlayerRegistry {
    /// Maps client link entity -> (PeerId, player entity)
    client_to_player: HashMap<Entity, (PeerId, Entity)>,
    /// Maps PeerId -> client link entity
    peer_to_client: HashMap<PeerId, Entity>,
}

/// Color for server-owned entities (neutral white)
const SERVER_COLOR: Color = Color::WHITE;

/// When a client link is created, add replication sender
fn handle_new_client(trigger: On<Add, LinkOf>, mut commands: Commands) {
    commands.entity(trigger.entity).insert((
        ReplicationSender::new(
            SERVER_REPLICATION_INTERVAL,
            SendUpdatesMode::SinceLastAck,
            false,
        ),
        ReplicationReceiver::default(),  // Also receive replication from clients with authority
        Name::from("Client"),
    ));
    info!("🔗 New client link created: {:?}", trigger.entity);
}

/// When client fully connects, spawn their player entity
fn handle_connected(
    trigger: On<Add, Connected>,
    client_query: Query<(&RemoteId, &LinkOf, &ViaTransport), With<ClientOf>>,
    transport_names: Query<&Name>,
    mut commands: Commands,
    mut registry: ResMut<PlayerRegistry>,
) {
    let Ok((remote_id, link_of, via_transport)) = client_query.get(trigger.entity) else {
        warn!("⚠️ Connected but couldn't get client components for {:?}", trigger.entity);
        return;
    };
    
    let client_id = remote_id.0;
    let server_entity = link_of.server;

    // Determine transport name for logging
    let transport_name = transport_names.get(via_transport.transport)
        .map(|n| n.as_str())
        .unwrap_or("Unknown");
    
    let color = color_from_id(client_id);
    
    // Position players based on their PeerId (distributed horizontally)
    let x_pos = (client_id.to_bits() % 400) as f32 - 200.0;
    
    // Spawn player entity with replication to all clients
    let player_entity = commands
        .spawn((
            PlayerId(client_id),
            Position(Vec2::new(x_pos, 0.0)),
            PlayerColor(color),
            // Replicate to all clients
            Replicate::to_clients(NetworkTarget::All),
            // Owner predicts this entity
            PredictionTarget::to_clients(NetworkTarget::Single(client_id)),
            // Others interpolate
            InterpolationTarget::to_clients(NetworkTarget::AllExceptSingle(client_id)),
            // Track owner for input processing
            ControlledBy {
                owner: trigger.entity,
                lifetime: Default::default(),
            },
            Name::new(format!("Player_{}", client_id.to_bits())),
        ))
        .id();

    registry.client_to_player.insert(trigger.entity, (client_id, player_entity));
    registry.peer_to_client.insert(client_id, trigger.entity);
    
    info!("🎮 Client {:?} connected via {} (Server {:?})", 
        client_id, transport_name, server_entity);
    info!("   ✅ Spawned player entity {:?} at ({:.0}, 0)", player_entity, x_pos);
}

/// Handle client disconnection
fn handle_disconnected(
    trigger: On<Add, Disconnected>,
    mut commands: Commands,
    mut registry: ResMut<PlayerRegistry>,
) {
    if let Some((peer_id, player_entity)) = registry.client_to_player.remove(&trigger.entity) {
        info!("👋 Client {:?} disconnected, despawning player {:?}", peer_id, player_entity);
        registry.peer_to_client.remove(&peer_id);
        commands.entity(player_entity).despawn();
    }
}

/// Server startup - spawn transports
fn startup(mut commands: Commands) -> Result {
    info!("\n=== Multi-Transport Distributed Authority Server ===\n");

    // 1. Spawn the logical server entity
    let server = commands
        .spawn((
            Server::default(),
            Name::new("GameServer"),
        ))
        .id();
    info!("🎯 Spawned Server entity: {:?}", server);

    // 2. UDP Transport
    let udp_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), UDP_PORT);
    let udp_transport = commands
        .spawn((
            NetcodeServer::new(NetcodeConfig::default()),
            LocalAddr(udp_addr),
            ServerUdpIo::default(),
            TransportOf::new(server),
            Name::new("UdpTransport"),
        ))
        .id();
    commands.trigger(Start { entity: udp_transport });
    info!("📡 UDP transport on port {} -> Server {:?}", UDP_PORT, server);

    // 3. WebTransport
    let wt_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), WEBTRANSPORT_PORT);
    let sans = vec!["localhost".to_string(), "127.0.0.1".to_string(), "::1".to_string()];
    let identity = Identity::self_signed(sans).unwrap();
    let digest = identity.certificate_chain().as_slice()[0].hash();
    info!("🔐 WebTransport cert digest: {}", digest);
    
    let wt_transport = commands
        .spawn((
            NetcodeServer::new(NetcodeConfig::default()),
            LocalAddr(wt_addr),
            WebTransportServerIo { certificate: identity },
            TransportOf::new(server),
            Name::new("WebTransportTransport"),
        ))
        .id();
    commands.trigger(Start { entity: wt_transport });
    info!("🌐 WebTransport on port {} -> Server {:?}", WEBTRANSPORT_PORT, server);

    // 4. WebSocket
    let ws_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), WEBSOCKET_PORT);
    let ws_sans = vec!["localhost".to_string(), "127.0.0.1".to_string(), "::1".to_string()];
    let ws_config = lightyear::websocket::server::ServerConfig::builder()
        .with_bind_address(ws_addr)
        .with_identity(lightyear::websocket::server::Identity::self_signed(ws_sans).unwrap());
    let ws_transport = commands
        .spawn((
            NetcodeServer::new(NetcodeConfig::default()),
            LocalAddr(ws_addr),
            WebSocketServerIo { config: ws_config },
            TransportOf::new(server),
            Name::new("WebSocketTransport"),
        ))
        .id();
    commands.trigger(Start { entity: ws_transport });
    info!("🔌 WebSocket on port {} -> Server {:?}", WEBSOCKET_PORT, server);

    // Mark server as started for input processing
    commands.entity(server).insert(Started);

    info!("\n✅ Server ready with distributed authority ball!\n");
    Ok(())
}

/// Spawn the ball entity - separate system so server is ready
fn spawn_ball(mut commands: Commands) {
    // Ball entity - starts with server authority
    commands.spawn((
        BallMarker,
        Position(Vec2::new(300.0, 0.0)),
        Speed(Vec2::new(0.0, 1.0)),  // Moves up/down
        PlayerColor(SERVER_COLOR),    // White = server owns it
        // Replicate to all clients
        Replicate::to_clients(NetworkTarget::All),
        // All clients interpolate (server has authority initially)
        InterpolationTarget::to_clients(NetworkTarget::All),
        Name::new("Ball"),
    ));
    info!("⚽ Spawned ball entity with Server authority");
}

/// Process player movement inputs on server
fn player_movement(
    mut query: Query<(&mut Position, &ActionState<Inputs>), Without<BallMarker>>,
) {
    for (position, action_state) in query.iter_mut() {
        shared_movement_behaviour(position, &action_state.0);
    }
}

/// Transfer ball authority to nearest player within threshold
fn transfer_authority(
    mut timer: Local<Timer>,
    time: Res<Time>,
    mut commands: Commands,
    ball_query: Query<(Entity, &Position), With<BallMarker>>,
    players: Query<(&PlayerId, &Position), Without<BallMarker>>,
) {
    // Only check every 0.3 seconds
    if !timer.tick(time.delta()).is_finished() {
        return;
    }
    *timer = Timer::new(Duration::from_secs_f32(0.3), TimerMode::Once);
    
    for (ball_entity, ball_pos) in ball_query.iter() {
        // Find closest player
        let mut closest: Option<PeerId> = None;
        let mut closest_dist = f32::MAX;
        
        for (player_id, player_pos) in players.iter() {
            let dist = player_pos.0.distance(ball_pos.0);
            if dist < AUTHORITY_TRANSFER_DISTANCE && dist < closest_dist {
                closest_dist = dist;
                closest = Some(player_id.0);
            }
        }
        
        // Transfer authority - if no player close, give to server
        let new_authority = Some(closest.unwrap_or(PeerId::Server));
        commands.trigger(GiveAuthority {
            entity: ball_entity,
            peer: new_authority,
        });
    }
}

/// Update ball color to match current authority owner
fn update_ball_color(
    broker: Query<&AuthorityBroker, (With<Server>, Changed<AuthorityBroker>)>,
    mut balls: Query<&mut PlayerColor, With<BallMarker>>,
    players: Query<(&PlayerId, &PlayerColor), Without<BallMarker>>,
) {
    let Ok(broker) = broker.single() else {
        return;
    };
    
    for (entity, current_authority) in broker.owners.iter() {
        if let Ok(mut color) = balls.get_mut(*entity) {
            match current_authority {
                None => {
                    color.0 = Color::BLACK;
                }
                Some(PeerId::Server) => {
                    color.0 = Color::WHITE;
                }
                Some(peer_id) => {
                    // Find player with this PeerId and use their color
                    if let Some((_, player_color)) = players.iter()
                        .find(|(id, _)| id.0 == *peer_id)
                    {
                        color.0 = player_color.0;
                    } else {
                        color.0 = color_from_id(*peer_id);
                    }
                }
            }
        }
    }
}

/// Log server status periodically
fn log_server_status(
    server_query: Query<&Server>,
    broker_query: Query<&AuthorityBroker, With<Server>>,
    ball_query: Query<&Position, With<BallMarker>>,
    mut timer: Local<Option<Timer>>,
    time: Res<Time>,
) {
    let timer = timer.get_or_insert_with(|| Timer::from_seconds(5.0, TimerMode::Repeating));
    timer.tick(time.delta());
    
    if timer.just_finished() {
        for server in &server_query {
            let client_count = server.collection().len();
            
            let authority_info = broker_query.iter().next()
                .and_then(|broker| {
                    ball_query.iter().next().map(|pos| {
                        // Find ball's authority from broker
                        let auth = broker.owners.values().next()
                            .map(|a| match a {
                                Some(PeerId::Server) => "Server".to_string(),
                                Some(id) => format!("Client {:?}", id),
                                None => "None".to_string(),
                            })
                            .unwrap_or("Unknown".to_string());
                        (pos.0, auth)
                    })
                });
            
            match authority_info {
                Some((pos, auth)) => {
                    info!("📊 Server: {} clients, ball at ({:.0}, {:.0}) authority: {}", 
                        client_count, pos.x, pos.y, auth);
                }
                None => {
                    info!("📊 Server: {} clients (no ball)", client_count);
                }
            }
        }
    }
}
