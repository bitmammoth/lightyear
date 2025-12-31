//! Server module - runs both UDP and WebTransport transports.
//!
//! This demonstrates ServerRole tracking multiple active transports.

use crate::shared::*;
use bevy::prelude::*;
use core::net::Ipv4Addr;
use std::net::SocketAddr;
use lightyear::prelude::server::*;
use lightyear::prelude::*;

pub struct ExampleServerPlugin;

impl Plugin for ExampleServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, startup);
        app.add_observer(handle_new_client);
        app.add_systems(Update, (log_server_role, handle_pings));
    }
}

/// Handle new client connections
fn handle_new_client(trigger: On<Add, Connected>, mut commands: Commands) {
    info!("🎮 New client connected: {:?}", trigger.entity);
    commands
        .entity(trigger.entity)
        .insert(ReplicationSender::new(
            SERVER_REPLICATION_INTERVAL,
            SendUpdatesMode::SinceLastAck,
            false,
        ));
}

/// Start both UDP and WebTransport servers
fn startup(mut commands: Commands) -> Result {
    info!("\n=== Multi-Transport Server Starting ===\n");

    // UDP Server
    let udp_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), UDP_PORT);
    let udp_server = commands
        .spawn((
            NetcodeServer::new(NetcodeConfig::default()),
            LocalAddr(udp_addr),
            ServerUdpIo::default(),
            Name::new("UdpServer"),
        ))
        .id();
    commands.trigger(Start { entity: udp_server });
    info!("📡 UDP server starting on port {}", UDP_PORT);

    // WebTransport Server with self-signed cert
    let wt_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), WEBTRANSPORT_PORT);
    let sans = vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "::1".to_string(),
    ];
    let identity = Identity::self_signed(sans).unwrap();
    let digest = identity.certificate_chain().as_slice()[0].hash();
    info!("🔐 WebTransport certificate digest: {}", digest);
    
    let wt_server = commands
        .spawn((
            NetcodeServer::new(NetcodeConfig::default()),
            LocalAddr(wt_addr),
            WebTransportServerIo { certificate: identity },
            Name::new("WebTransportServer"),
        ))
        .id();
    commands.trigger(Start { entity: wt_server });
    info!("🌐 WebTransport server starting on port {}", WEBTRANSPORT_PORT);

    info!("\nServers initialized. Waiting for clients...\n");
    Ok(())
}

/// Log ServerRole state changes
fn log_server_role(server_role: Res<ServerRole>) {
    if server_role.is_changed() {
        info!("📊 ServerRole state: {:?}", server_role.state);
    }
}

/// Handle ping messages from clients
fn handle_pings(
    mut query: Query<(Entity, &mut MessageReceiver<PingMessage>, &mut MessageSender<PongMessage>)>,
) {
    for (entity, mut receiver, mut sender) in query.iter_mut() {
        for ping in receiver.receive() {
            info!("📨 Received ping from client {} (seq: {})", ping.client_id, ping.sequence);
            
            let pong = PongMessage {
                sequence: ping.sequence,
            };
            sender.send::<ReliableChannel>(pong);
            info!("📤 Sent pong (seq: {})", ping.sequence);
        }
    }
}
