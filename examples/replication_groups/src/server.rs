//! Server module - demonstrates replication groups with multiple transports.
//!
//! Spawns a player (head) + trail as a replication group when clients connect.
//! The trail uses `ReplicateLike { root: player }` to replicate atomically with the player.

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

pub struct ExampleServerPlugin;

impl Plugin for ExampleServerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerRegistry>();
        app.add_systems(Startup, startup);
        app.add_observer(handle_new_client);
        app.add_observer(handle_connected);
        app.add_observer(handle_client_disconnected);
        // Movement in FixedUpdate for determinism
        app.add_systems(FixedUpdate, movement);
        app.add_systems(Update, log_server_role);
    }
}

/// Track connected players and their entities by name
#[derive(Resource, Default)]
struct PlayerRegistry {
    next_id: u64,
    /// Maps client link entity -> (player_name, player entity, trail entity)
    client_to_entities: HashMap<Entity, (String, Entity, Entity)>,
}

impl PlayerRegistry {
    fn next_player_id(&mut self) -> PlayerId {
        let id = PlayerId(self.next_id);
        self.next_id += 1;
        id
    }
}

/// When a client first connects (link created), add replication sender
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

/// When client fully connects, spawn their player entity + trail as a replication group
fn handle_connected(
    trigger: On<Add, Connected>,
    client_query: Query<(&RemoteId, &LinkOf, &ViaTransport), With<ClientOf>>,
    transport_names: Query<&Name>,
    mut commands: Commands,
    mut registry: ResMut<PlayerRegistry>,
) {
    let Ok((remote_id, link_of, via_transport)) = client_query.get(trigger.entity) else {
        warn!("⚠️ Connected trigger but couldn't get client components for {:?}", trigger.entity);
        return;
    };
    let client_id = remote_id.0;
    let server_entity = link_of.server;
    let player_id = registry.next_player_id();

    info!("📋 Server connecting client {:?} (RemoteId: {:?})", trigger.entity, client_id);
    
    // Determine player name from transport (UDP, WT, WS)
    let player_name = transport_names.get(via_transport.transport)
        .map(|n| {
            let name = n.as_str();
            if name.contains("Udp") { "UDP".to_string() }
            else if name.contains("WebTransport") { "WT".to_string() }
            else if name.contains("WebSocket") { "WS".to_string() }
            else { format!("Player_{}", player_id.0) }
        })
        .unwrap_or_else(|_| format!("Player_{}", player_id.0));
    
    info!("🎮 Client '{}' connected via server {:?}", player_name, server_entity);
    
    // Choose a color based on transport
    let color = match player_name.as_str() {
        "UDP" => Color::srgb(1.0, 0.2, 0.2),   // Red
        "WT" => Color::srgb(0.2, 0.8, 0.2),    // Green
        "WS" => Color::srgb(0.2, 0.4, 1.0),    // Blue
        _ => Color::srgb(0.8, 0.8, 0.8),       // Gray
    };
    
    let start_x = match player_name.as_str() {
        "UDP" => -100.0,
        "WT" => 0.0,
        "WS" => 100.0,
        _ => 0.0,
    };
    
    // Spawn the player entity (head of replication group)
    let player_entity = commands
        .spawn((
            player_id,
            PlayerPosition(Vec2::new(start_x, 0.0)),
            PlayerColor(color),
            // Replicate to all clients across all transports
            Replicate::to_clients(NetworkTarget::All),
            // The owning client will predict this entity
            PredictionTarget::to_clients(NetworkTarget::Single(client_id)),
            // Other clients will interpolate this entity
            InterpolationTarget::to_clients(NetworkTarget::AllExceptSingle(client_id)),
            // Track which client controls this entity (for input processing)
            ControlledBy {
                owner: trigger.entity,
                lifetime: Default::default(),
            },
            Name::new(format!("{} Head", player_name)),
        ))
        .id();
    
    // Spawn the trail entity - linked to the player via ReplicateLike
    // This means the trail will be replicated atomically with the player
    let trail_entity = commands
        .spawn((
            TrailPosition(Vec2::new(start_x - TRAIL_OFFSET, -TRAIL_OFFSET)),
            TrailColor(color.with_alpha(0.5)),
            PlayerParent(player_entity),
            // ReplicateLike makes this entity replicate with the same settings as the player
            ReplicateLike { root: player_entity },
            Name::new(format!("{} Trail", player_name)),
        ))
        .id();

    registry.client_to_entities.insert(trigger.entity, (player_name.clone(), player_entity, trail_entity));
    info!("   ✅ Spawned player '{}' head {:?} + trail {:?} as replication group", player_name, player_entity, trail_entity);
}

/// Handle client disconnections
fn handle_client_disconnected(
    trigger: On<Add, Disconnected>,
    mut commands: Commands,
    mut registry: ResMut<PlayerRegistry>,
) {
    let entity = trigger.entity;
    
    // Remove player and trail entities if they exist
    if let Some((player_name, player_entity, trail_entity)) = registry.client_to_entities.remove(&entity) {
        info!("👋 Client '{}' disconnected", player_name);
        commands.entity(trail_entity).despawn();
        commands.entity(player_entity).despawn();
    }
}

/// Start UDP, WebTransport, and WebSocket servers
fn startup(mut commands: Commands) -> Result {
    info!("\n=== Multi-Transport Replication Groups Server Starting ===\n");

    // 1. Spawn ONE logical server - all clients will have LinkOf pointing here
    let server = commands
        .spawn((
            Server::default(),
            Name::new("GameServer"),
        ))
        .id();
    info!("🎯 Spawned logical Server entity: {:?}", server);

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
    info!("📡 UDP transport starting on port {} -> Server {:?}", UDP_PORT, server);

    // 3. WebTransport
    let wt_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), WEBTRANSPORT_PORT);
    let sans = vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "::1".to_string(),
    ];
    let identity = Identity::self_signed(sans).unwrap();
    let digest = identity.certificate_chain().as_slice()[0].hash();
    info!("🔐 WebTransport certificate digest: {}", digest);
    
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
    info!("🌐 WebTransport transport starting on port {} -> Server {:?}", WEBTRANSPORT_PORT, server);

    // 4. WebSocket
    let ws_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), WEBSOCKET_PORT);
    let sans = vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "::1".to_string(),
    ];
    let ws_config = lightyear::websocket::server::ServerConfig::builder()
        .with_bind_address(ws_addr)
        .with_identity(lightyear::websocket::server::Identity::self_signed(sans).unwrap());
    
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
    info!("🔌 WebSocket transport starting on port {} -> Server {:?}", WEBSOCKET_PORT, server);
    
    // Manually add Started to the Server entity
    commands.entity(server).insert(Started);

    info!("\n=== Server Ready ===\n");
    Ok(())
}

/// Apply movement from client inputs
fn movement(
    mut query: Query<(&mut PlayerPosition, &ControlledBy), With<Replicate>>,
    inputs: Query<&ActionState<Inputs>>,
) {
    for (position, controlled_by) in query.iter_mut() {
        if let Ok(input) = inputs.get(controlled_by.owner) {
            shared_movement_behaviour(position, input);
        }
    }
}

/// Log server role info periodically
fn log_server_role(
    server_query: Query<(Entity, Has<Started>), With<Server>>,
    mut timer: Local<Option<Timer>>,
    time: Res<Time>,
) {
    let timer = timer.get_or_insert_with(|| Timer::from_seconds(5.0, TimerMode::Once));
    timer.tick(time.delta());
    
    if timer.just_finished() {
        for (entity, started) in server_query.iter() {
            info!("🖥️  Server {:?} - Started: {}", entity, started);
        }
    }
}
