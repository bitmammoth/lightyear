//! Server module - multi-transport server setup.
//!
//! Demonstrates starting a server with UDP, WebTransport, and WebSocket transports.

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
        app.add_observer(on_server_started);
    }
}

fn on_server_started(trigger: On<Add, Started>, names: Query<&Name>) {
    if let Ok(name) = names.get(trigger.entity) {
        info!("✅ {} started", name);
    }
}

/// Handle new client connections - add ReplicationSender to enable replication
fn handle_new_client(trigger: On<Add, Connected>, mut commands: Commands) {
    info!("New client connected: {:?}", trigger.entity);
    commands
        .entity(trigger.entity)
        .insert(ReplicationSender::new(
            SERVER_REPLICATION_INTERVAL,
            SendUpdatesMode::SinceLastAck,
            false,
        ));
}

/// Start the multi-transport server
fn startup(mut commands: Commands) -> Result {
    info!("Starting multi-transport server...");
    
    // 1. Spawn the logical Server entity (single server for all transports)
    let server = commands
        .spawn((
            Server::default(),
            Name::new("Server"),
        ))
        .id();
    info!("Created logical server entity: {:?}", server);

    // 2. UDP Transport (port 5000)
    let udp_addr = UDP_SERVER_ADDR;
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
    info!("UDP transport on {}", udp_addr);

    // 3. WebTransport (port 5001) - requires TLS certificate
    let wt_addr = WT_SERVER_ADDR;
    let identity = lightyear::prelude::Identity::self_signed(vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
    ])
    .expect("Failed to create WebTransport identity");
    
    // Print certificate digest for clients
    let cert_digest = identity.certificate_chain()
        .as_slice()
        .first()
        .expect("No certificate in chain")
        .hash();
    info!("📋 WebTransport certificate digest: {}", cert_digest);
    
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
    info!("WebTransport on {}", wt_addr);

    // 4. WebSocket (port 5002) - requires TLS certificate
    let ws_addr = WS_SERVER_ADDR;
    let ws_sans = vec!["localhost".to_string(), "127.0.0.1".to_string()];
    let ws_config = lightyear::websocket::server::ServerConfig::builder()
        .with_bind_address(ws_addr)
        .with_identity(
            lightyear::websocket::server::Identity::self_signed(ws_sans)
                .expect("Failed to create WebSocket identity"),
        );
    
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
    info!("WebSocket on {}", ws_addr);

    info!("🚀 Multi-transport server ready!");
    info!("   UDP: {}", udp_addr);
    info!("   WebTransport: {} (use cert digest above)", wt_addr);
    info!("   WebSocket: {}", ws_addr);

    Ok(())
}
