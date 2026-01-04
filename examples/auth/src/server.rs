//! Server module - multi-transport server with TCP auth backend.
//!
//! The server:
//! 1. Starts TCP listener for auth token requests
//! 2. Generates ConnectTokens for clients
//! 3. Runs game server on UDP, WebTransport, and WebSocket

extern crate alloc;
use alloc::sync::Arc;
use async_compat::Compat;
use std::sync::RwLock;
use std::collections::HashSet;

use bevy::prelude::*;
use bevy::tasks::IoTaskPool;
use core::net::Ipv4Addr;
use std::net::SocketAddr;
use lightyear::netcode::ConnectToken;
use lightyear::prelude::server::*;
use lightyear::prelude::*;
use lightyear::connection::server::Started;
use tokio::io::AsyncWriteExt;

use crate::shared::*;

pub struct ExampleServerPlugin;

impl Plugin for ExampleServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, startup);
        app.add_observer(handle_new_client);
        app.add_observer(handle_connected);
        app.add_observer(handle_disconnected);
        
        // Start the auth backend task
        let client_ids = Arc::new(RwLock::new(HashSet::default()));
        start_auth_backend(client_ids.clone());
        app.insert_resource(ClientIds(client_ids));
    }
}

/// Track connected client IDs to prevent duplicates
#[derive(Resource)]
struct ClientIds(Arc<RwLock<HashSet<u64>>>);

/// Start the server with multiple transports
fn startup(mut commands: Commands) -> Result {
    info!("\n=== Multi-Transport Auth Server Starting ===\n");
    info!("🔐 Auth backend listening on TCP port {}", AUTH_BACKEND_PORT);

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
    info!("📡 UDP transport starting on port {}", UDP_PORT);

    // 3. WebTransport
    let wt_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), WEBTRANSPORT_PORT);
    let sans = vec!["localhost".to_string(), "127.0.0.1".to_string(), "::1".to_string()];
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
    info!("🌐 WebTransport transport starting on port {}", WEBTRANSPORT_PORT);

    // 4. WebSocket
    let ws_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), WEBSOCKET_PORT);
    let sans = vec!["localhost".to_string(), "127.0.0.1".to_string(), "::1".to_string()];
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
    info!("🔌 WebSocket transport starting on port {}", WEBSOCKET_PORT);
    
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

/// Track client connection
fn handle_connected(
    trigger: On<Add, Connected>,
    query: Query<&RemoteId, With<ClientOf>>,
    client_ids: Res<ClientIds>,
) {
    let Ok(remote_id) = query.get(trigger.entity) else { return };
    if let PeerId::Netcode(client_id) = remote_id.0 {
        info!("✅ Client {} connected (authenticated via token)", client_id);
        client_ids.0.write().unwrap().insert(client_id);
    }
}

/// Track client disconnection
fn handle_disconnected(
    trigger: On<Add, Disconnected>,
    query: Query<&RemoteId, With<ClientOf>>,
    client_ids: Res<ClientIds>,
) {
    let Ok(remote_id) = query.get(trigger.entity) else { return };
    if let PeerId::Netcode(client_id) = remote_id.0 {
        info!("👋 Client {} disconnected", client_id);
        client_ids.0.write().unwrap().remove(&client_id);
    }
}

/// Start the TCP auth backend that generates ConnectTokens
fn start_auth_backend(client_ids: Arc<RwLock<HashSet<u64>>>) {
    IoTaskPool::get()
        .spawn(Compat::new(async move {
            info!("🔐 Starting auth backend on {}", AUTH_BACKEND_ADDR);
            let listener = tokio::net::TcpListener::bind(AUTH_BACKEND_ADDR)
                .await
                .expect("Failed to bind auth backend");
            
            loop {
                let (mut stream, peer_addr) = listener.accept().await.unwrap();
                info!("📥 Auth request from {}", peer_addr);

                // Generate unique client ID
                let client_id = loop {
                    let id = rand::random();
                    if !client_ids.read().unwrap().contains(&id) {
                        break id;
                    }
                };

                // Generate ConnectToken for UDP server (client will connect to appropriate transport)
                let token = ConnectToken::build(
                    UDP_SERVER_ADDR,
                    PROTOCOL_ID,
                    client_id,
                    PRIVATE_KEY,
                )
                .generate()
                .expect("Failed to generate token");

                let serialized = token.try_into_bytes().expect("Failed to serialize token");
                info!("📤 Sending token for client {} ({} bytes)", client_id, serialized.len());
                
                stream.write_all(&serialized).await.expect("Failed to send token");
            }
        }))
        .detach();
}
