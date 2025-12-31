//! Server module - demonstrates entity replication with multiple transports.
//!
//! This server runs both UDP and WebTransport, allowing clients to connect
//! via either transport and see replicated player entities.

use crate::shared::*;
use bevy::prelude::*;
use core::net::Ipv4Addr;
use std::collections::HashMap;
use std::net::SocketAddr;
use lightyear::prelude::server::*;
use lightyear::prelude::*;

pub struct ExampleServerPlugin;

impl Plugin for ExampleServerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerRegistry>();
        app.add_systems(Startup, startup);
        app.add_observer(handle_new_client);
        app.add_observer(handle_connected);
        app.add_observer(handle_client_disconnected);
        app.add_systems(Update, log_server_role);
    }
}

/// Track connected players and their entities
#[derive(Resource, Default)]
struct PlayerRegistry {
    next_id: u64,
    /// Maps client link entity -> (player_id, player entity)
    client_to_player: HashMap<Entity, (PlayerId, Entity)>,
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

/// When client fully connects, spawn their player entity
/// Uses Target replication mode to replicate to ALL clients across ALL transports
fn handle_connected(
    trigger: On<Add, Connected>,
    client_query: Query<(&RemoteId, &LinkOf), With<ClientOf>>,
    mut commands: Commands,
    mut registry: ResMut<PlayerRegistry>,
) {
    let Ok((remote_id, link_of)) = client_query.get(trigger.entity) else {
        return;
    };
    let client_id = remote_id.0;
    let server_entity = link_of.server;
    let player_id = registry.next_player_id();
    
    info!("🎮 Client connected: {:?} via server {:?}", client_id, server_entity);
    info!("   Assigned player ID: {:?}", player_id);
    
    // Choose a color based on player id
    let colors = [
        Color::srgb(1.0, 0.2, 0.2),  // Red
        Color::srgb(0.2, 0.8, 0.2),  // Green
        Color::srgb(0.2, 0.4, 1.0),  // Blue
        Color::srgb(1.0, 0.8, 0.2),  // Yellow
        Color::srgb(0.8, 0.2, 0.8),  // Purple
        Color::srgb(0.2, 0.8, 0.8),  // Cyan
    ];
    let color = colors[player_id.0 as usize % colors.len()];
    
    // Spawn the player entity with replication to ALL clients across ALL transports
    // Using to_all() for unified world replication across UDP + WebTransport
    let player_entity = commands
        .spawn((
            Player { id: player_id },
            PlayerPosition(Vec2::new(
                (player_id.0 as f32 % 5.0) * 100.0 - 200.0,
                (player_id.0 as f32 / 5.0).floor() * 100.0 - 200.0,
            )),
            PlayerColor(color),
            // Replicate to all clients across all transports (unified world)
            Replicate::to_all(NetworkTarget::All),
            Name::new(format!("Player_{}", player_id.0)),
        ))
        .id();
    
    registry.client_to_player.insert(trigger.entity, (player_id, player_entity));
    info!("   Spawned player entity {:?}", player_entity);
}

/// Handle client disconnections
fn handle_client_disconnected(
    trigger: On<Add, Disconnected>,
    mut commands: Commands,
    mut registry: ResMut<PlayerRegistry>,
) {
    let entity = trigger.entity;
    info!("👋 Client disconnected: {:?}", entity);
    
    // Remove player entity if it exists
    if let Some((player_id, player_entity)) = registry.client_to_player.remove(&entity) {
        info!("   Despawning player {:?}", player_id);
        commands.entity(player_entity).despawn();
    }
}

/// Start both UDP and WebTransport servers
fn startup(mut commands: Commands) -> Result {
    info!("\n=== Multi-Transport Replication Server Starting ===\n");

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
