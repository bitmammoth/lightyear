//! Server module - multi-transport client replication example.
//!
//! Receives cursor entities from clients and replicates them to all other clients.

use crate::protocol::*;
use crate::shared::*;
use bevy::prelude::*;
use core::net::Ipv4Addr;
use std::net::SocketAddr;
use lightyear::prelude::server::*;
use lightyear::prelude::*;
use lightyear::connection::server::Started;

pub struct ExampleServerPlugin;

impl Plugin for ExampleServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, startup);
        app.add_observer(handle_new_client);
        // When we receive a cursor from a client, replicate it to all other clients
        app.add_observer(replicate_cursors);
        app.add_systems(Update, log_server_status);
    }
}

/// When a client link is created, add replication receiver AND sender
fn handle_new_client(trigger: On<Add, LinkOf>, mut commands: Commands) {
    commands.entity(trigger.entity).insert((
        // Receive replication FROM clients (client-authoritative cursors)
        ReplicationReceiver::default(),
        // Send replication TO clients (relay cursors to other clients)
        ReplicationSender::new(
            SERVER_REPLICATION_INTERVAL,
            SendUpdatesMode::SinceLastAck,
            false,
        ),
        Name::from("Client"),
    ));
    info!("🔗 New client link created: {:?}", trigger.entity);
}

/// When we receive a cursor entity from a client, set it up for replication to others
fn replicate_cursors(
    trigger: On<Add, (CursorPosition, Replicated)>,
    mut commands: Commands,
    cursor_query: Query<&Replicated, With<CursorPosition>>,
    client_query: Query<&RemoteId, With<ClientOf>>,
) {
    let entity = trigger.entity;
    let Ok(replicated) = cursor_query.get(entity) else {
        return;
    };
    
    // Get the client ID of the owner
    let Ok(remote_id) = client_query.get(replicated.receiver) else {
        return;
    };
    let client_id = remote_id.0;
    
    info!("📨 Received cursor from client {:?}, relaying to others", client_id);
    
    // Add replication to ALL OTHER clients (not back to sender)
    if let Ok(mut e) = commands.get_entity(entity) {
        e.insert((
            // Replicate to all clients except the one who owns this cursor
            Replicate::to_clients(NetworkTarget::AllExceptSingle(client_id)),
            // Other clients will interpolate this cursor
            InterpolationTarget::to_clients(NetworkTarget::AllExceptSingle(client_id)),
            // Track who owns this cursor
            ControlledBy {
                owner: replicated.receiver,
                lifetime: Lifetime::SessionBased,
            },
        ));
    }
}

/// Server startup - spawn transports
fn startup(mut commands: Commands) -> Result {
    info!("\n=== Multi-Transport Client Replication Server ===\n");

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

    info!("\n✅ Server ready for client replication!\n");
    Ok(())
}

/// Log server status periodically
fn log_server_status(
    server_query: Query<&Server>,
    cursors: Query<(), With<CursorPosition>>,
    mut timer: Local<Option<Timer>>,
    time: Res<Time>,
) {
    let timer = timer.get_or_insert_with(|| Timer::from_seconds(5.0, TimerMode::Repeating));
    timer.tick(time.delta());
    
    if timer.just_finished() {
        for server in &server_query {
            let client_count = server.collection().len();
            info!("📊 Server: {} clients, {} cursors", client_count, cursors.iter().count());
        }
    }
}
