//! Server module - demonstrates delta compression with multiple transports.

use crate::protocol::*;
use crate::shared::*;
use bevy::prelude::*;
use core::net::Ipv4Addr;
use std::net::SocketAddr;
use lightyear::prelude::server::*;
use lightyear::prelude::*;
use lightyear::prelude::input::native::ActionState;
use lightyear::link::prelude::ViaTransport;
use lightyear::connection::server::Started;

pub struct ExampleServerPlugin;

impl Plugin for ExampleServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, startup);
        app.add_observer(handle_new_client);
        app.add_observer(handle_connected);
        app.add_observer(handle_disconnected);
        // Movement in FixedUpdate for determinism
        app.add_systems(FixedUpdate, movement);
    }
}

/// Start the server with multiple transports
fn startup(mut commands: Commands) -> Result {
    info!("\n=== Multi-Transport Delta Compression Server Starting ===\n");
    info!("📦 Delta compression enabled: Vec2 (8 bytes) -> (i8, i8) (2 bytes) = 75% reduction");

    // 1. Spawn ONE logical server
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

/// Add ReplicationSender to new clients
fn handle_new_client(trigger: On<Add, LinkOf>, mut commands: Commands) {
    info!("🔗 New client link created: {:?}", trigger.entity);
    commands.entity(trigger.entity).insert((
        ReplicationSender::new(SERVER_REPLICATION_INTERVAL, SendUpdatesMode::SinceLastAck, false),
        Name::from("Client"),
    ));
}

/// Spawn player entity when client connects
fn handle_connected(
    trigger: On<Add, Connected>,
    client_query: Query<(&RemoteId, &ViaTransport), With<ClientOf>>,
    transport_names: Query<&Name>,
    mut commands: Commands,
    mut player_count: Local<u64>,
) {
    let Ok((remote_id, via_transport)) = client_query.get(trigger.entity) else {
        warn!("⚠️ Connected trigger but couldn't get client components");
        return;
    };
    let client_id = remote_id.0;
    *player_count += 1;
    
    // Determine player name from transport
    let player_name = transport_names.get(via_transport.transport)
        .map(|n| {
            let name = n.as_str();
            if name.contains("Udp") { "UDP".to_string() }
            else if name.contains("WebTransport") { "WT".to_string() }
            else if name.contains("WebSocket") { "WS".to_string() }
            else { format!("Player_{}", *player_count) }
        })
        .unwrap_or_else(|_| format!("Player_{}", *player_count));
    
    // Choose color based on transport
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
    
    let player_entity = commands
        .spawn((
            PlayerId(*player_count),
            PlayerPosition(Vec2::new(start_x, 0.0)),
            PlayerColor(color),
            Replicate::to_clients(NetworkTarget::All),
            PredictionTarget::to_clients(NetworkTarget::Single(client_id)),
            InterpolationTarget::to_clients(NetworkTarget::AllExceptSingle(client_id)),
            ControlledBy {
                owner: trigger.entity,
                lifetime: Default::default(),
            },
            Name::new(player_name.clone()),
        ))
        .id();

    info!("🎮 Client '{}' connected, spawned player {:?} (with delta compression)", player_name, player_entity);
}

/// Handle client disconnection
fn handle_disconnected(
    trigger: On<Add, Disconnected>,
    players: Query<(Entity, &ControlledBy), With<PlayerId>>,
    mut commands: Commands,
) {
    for (entity, controlled_by) in players.iter() {
        if controlled_by.owner == trigger.entity {
            commands.entity(entity).despawn();
            info!("👋 Player {:?} disconnected", entity);
        }
    }
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
