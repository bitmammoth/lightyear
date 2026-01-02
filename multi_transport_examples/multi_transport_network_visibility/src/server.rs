//! Server module - multi-transport network visibility example.
//!
//! Runs UDP, WebTransport, and WebSocket transports.
//! Spawns circles in a grid and manages visibility based on player distance.

use crate::protocol::*;
use crate::shared::*;
use bevy::prelude::*;
use core::net::Ipv4Addr;
use std::collections::HashMap;
use std::net::SocketAddr;
use lightyear::prelude::server::*;
use lightyear::prelude::*;
use lightyear::prelude::input::native::ActionState;
use lightyear::link::prelude::ViaTransport;
use lightyear::connection::server::Started;
use lightyear::connection::client::PeerMetadata;

pub struct ExampleServerPlugin;

impl Plugin for ExampleServerPlugin {
    fn build(&self, app: &mut App) {
        // Add Room plugin for static room-based visibility
        app.add_plugins(RoomPlugin);
        app.init_resource::<PlayerRegistry>();
        app.add_systems(Startup, (startup, spawn_circles));
        app.add_observer(handle_new_client);
        app.add_observer(handle_connected);
        app.add_observer(handle_disconnected);
        app.add_systems(FixedUpdate, player_movement);
        app.add_systems(Update, (interest_management, log_server_status));
        
        // Spawn a room for player entities (static visibility)
        app.world_mut().spawn(Room::default());
    }
}

/// Track connected players
#[derive(Resource, Default)]
struct PlayerRegistry {
    /// Maps client link entity -> (PeerId, player entity)
    client_to_player: HashMap<Entity, (PeerId, Entity)>,
}

/// When a client link is created, add replication sender
fn handle_new_client(trigger: On<Add, LinkOf>, mut commands: Commands) {
    commands.entity(trigger.entity).insert((
        ReplicationSender::new(
            SERVER_REPLICATION_INTERVAL,
            SendUpdatesMode::SinceLastAck,
            false,
        ),
        Name::from("Client"),
    ));
    info!("🔗 New client link created: {:?}", trigger.entity);
}

/// When client fully connects, spawn their player entity
fn handle_connected(
    trigger: On<Add, Connected>,
    client_query: Query<(&RemoteId, &LinkOf, &ViaTransport), With<ClientOf>>,
    transport_names: Query<&Name>,
    room: Single<Entity, With<Room>>,
    mut commands: Commands,
    mut registry: ResMut<PlayerRegistry>,
) {
    let Ok((remote_id, link_of, via_transport)) = client_query.get(trigger.entity) else {
        warn!("⚠️ Connected but couldn't get client components for {:?}", trigger.entity);
        return;
    };
    
    let client_id = remote_id.0;
    let server_entity = link_of.server;
    let transport_name = transport_names.get(via_transport.transport)
        .map(|n| n.as_str())
        .unwrap_or("Unknown");
    
    let color = color_from_id(client_id);
    
    // Position players at origin
    let player_entity = commands
        .spawn((
            PlayerId(client_id),
            Position(Vec2::ZERO),
            PlayerColor(color),
            Replicate::to_clients(NetworkTarget::All),
            PredictionTarget::to_clients(NetworkTarget::Single(client_id)),
            InterpolationTarget::to_clients(NetworkTarget::AllExceptSingle(client_id)),
            ControlledBy {
                owner: trigger.entity,
                lifetime: Default::default(),
            },
            // Enable network visibility for this entity
            NetworkVisibility,
            Name::new(format!("Player_{}", client_id.to_bits())),
        ))
        .id();

    registry.client_to_player.insert(trigger.entity, (client_id, player_entity));
    
    // Add client and player to the room for static visibility
    // This ensures all players can see all other players
    let room = room.into_inner();
    commands.trigger(RoomEvent {
        target: RoomTarget::AddSender(trigger.entity),
        room,
    });
    commands.trigger(RoomEvent {
        target: RoomTarget::AddEntity(player_entity),
        room,
    });
    
    info!("🎮 Client {:?} connected via {} (Server {:?})", 
        client_id, transport_name, server_entity);
    info!("   ✅ Spawned player entity {:?}", player_entity);
}

/// Handle client disconnection
fn handle_disconnected(
    trigger: On<Add, Disconnected>,
    mut commands: Commands,
    mut registry: ResMut<PlayerRegistry>,
) {
    if let Some((peer_id, player_entity)) = registry.client_to_player.remove(&trigger.entity) {
        info!("👋 Client {:?} disconnected, despawning player {:?}", peer_id, player_entity);
        commands.entity(player_entity).despawn();
    }
}

/// Server startup - spawn transports
fn startup(mut commands: Commands) -> Result {
    info!("\n=== Multi-Transport Network Visibility Server ===\n");

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

    // Mark server as started
    commands.entity(server).insert(Started);

    info!("\n✅ Server ready with network visibility!\n");
    Ok(())
}

/// Spawn circles in a grid pattern
fn spawn_circles(mut commands: Commands) {
    info!("🔵 Spawning circles in {}x{} grid (spacing: {})", 
        NUM_CIRCLES * 2 + 1, NUM_CIRCLES * 2 + 1, GRID_SIZE);
    
    for x in -NUM_CIRCLES..=NUM_CIRCLES {
        for y in -NUM_CIRCLES..=NUM_CIRCLES {
            commands.spawn((
                Position(Vec2::new(x as f32 * GRID_SIZE, y as f32 * GRID_SIZE)),
                CircleMarker,
                Replicate::to_clients(NetworkTarget::All),
                // Enable network visibility - circles are only visible to nearby players
                NetworkVisibility,
            ));
        }
    }
    
    let total = (NUM_CIRCLES * 2 + 1) * (NUM_CIRCLES * 2 + 1);
    info!("   ✅ Spawned {} circles", total);
}

/// Process player movement inputs on server
fn player_movement(
    mut query: Query<(&mut Position, &ActionState<Inputs>), Without<CircleMarker>>,
) {
    for (position, action_state) in query.iter_mut() {
        shared_movement_behaviour(position, &action_state.0);
    }
}

/// Interest management - circles become visible when players are close
fn interest_management(
    peer_metadata: Res<PeerMetadata>,
    player_query: Query<(&PlayerId, Ref<Position>), (Without<CircleMarker>, With<Replicate>)>,
    mut circle_query: Query<
        (Entity, &Position, &mut ReplicationState),
        (With<CircleMarker>, With<Replicate>, With<NetworkVisibility>),
    >,
) {
    for (player_id, position) in player_query.iter() {
        // Get the sender entity for this client
        let Some(sender_entity) = peer_metadata.mapping.get(&player_id.0) else {
            continue;
        };
        
        // Only update visibility when position changes
        if position.is_changed() {
            for (circle, circle_position, mut state) in circle_query.iter_mut() {
                let distance = position.distance(**circle_position);
                if distance < INTEREST_RADIUS {
                    state.gain_visibility(*sender_entity);
                } else {
                    state.lose_visibility(*sender_entity);
                }
            }
        }
    }
}

/// Log server status periodically
fn log_server_status(
    server_query: Query<&Server>,
    circles: Query<(), With<CircleMarker>>,
    players: Query<(), With<PlayerId>>,
    mut timer: Local<Option<Timer>>,
    time: Res<Time>,
) {
    let timer = timer.get_or_insert_with(|| Timer::from_seconds(5.0, TimerMode::Repeating));
    timer.tick(time.delta());
    
    if timer.just_finished() {
        for server in &server_query {
            let client_count = server.collection().len();
            info!("📊 Server: {} clients, {} players, {} circles (visibility radius: {})", 
                client_count, players.iter().count(), circles.iter().count(), INTEREST_RADIUS);
        }
    }
}
