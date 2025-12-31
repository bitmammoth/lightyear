//! Server - multi-transport with player spawning and movement.
//!
//! This server runs BOTH UDP and WebTransport transports simultaneously.
//! Clients can connect via either transport and see the same unified game world.

use crate::protocol::*;
use crate::shared;
use bevy::prelude::*;
use core::net::Ipv4Addr;
use core::time::Duration;
use std::net::SocketAddr;
use lightyear::connection::client::{Connected, Disconnected};
use lightyear::prelude::server::*;
use lightyear::prelude::input::native::*;
use lightyear::prelude::{*, Replicating};

pub const UDP_PORT: u16 = 5000;
pub const WEBTRANSPORT_PORT: u16 = 5001;
pub const SEND_INTERVAL: Duration = Duration::from_millis(100);

pub struct ServerPlugin;

impl Plugin for ServerPlugin {
    fn build(&self, app: &mut App) {
        // Server systems
        app.add_systems(Startup, startup_server);
        app.add_systems(FixedUpdate, movement);
        app.add_observer(handle_new_client);
        app.add_observer(handle_connected);
        app.add_observer(handle_disconnected);
        app.add_systems(Update, log_server_state);
    }
}

/// Start BOTH UDP and WebTransport servers simultaneously
fn startup_server(mut commands: Commands) -> Result {
    info!("\n=== Simple Box Multi-Transport Server ===\n");

    // UDP Server on port 5000
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

    // WebTransport Server on port 5001 with self-signed cert
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

    info!("\nWaiting for clients...\n");
    info!("Connect with:");
    info!("  cargo run -p simple_box_multi_transport -- client --transport udp");
    info!("  cargo run -p simple_box_multi_transport -- client --transport webtransport --cert <DIGEST>\n");
    info!("Use WASD or arrow keys to move!\n");
    
    Ok(())
}

/// When a new client link is created, add ReplicationSender
fn handle_new_client(trigger: On<Add, LinkOf>, mut commands: Commands) {
    commands.entity(trigger.entity).insert((
        ReplicationSender::new(SEND_INTERVAL, SendUpdatesMode::SinceLastAck, false),
        Name::from("ClientLink"),
    ));
    info!("🔗 New client link: {:?}", trigger.entity);
}

/// When client is fully connected, spawn their player entity
fn handle_connected(
    trigger: On<Add, Connected>,
    query: Query<(&RemoteId, &Name), With<ClientOf>>,
    mut commands: Commands,
) {
    let Ok((remote_id, link_name)) = query.get(trigger.entity) else {
        warn!("⚠️ Connected triggered but no RemoteId found on {:?}", trigger.entity);
        return;
    };
    let client_id = remote_id.0;
    
    // Spawn position based on client ID (spread them out)
    let id_bits = client_id.to_bits();
    let spawn_x = ((id_bits % 5) as f32 - 2.0) * 100.0;
    let spawn_y = ((id_bits / 5 % 5) as f32 - 2.0) * 100.0;
    
    info!("🎮 Client {:?} connected via {} (link: {:?})", client_id, link_name, trigger.entity);
    info!("   Spawning player at ({:.1}, {:.1})", spawn_x, spawn_y);
    
    let player_entity = commands
        .spawn((
            PlayerBundle::new(client_id, Vec2::new(spawn_x, spawn_y)),
            // KEY: Use Replicate::to_all() for multi-transport unified world
            // This replicates to ALL clients regardless of which transport they use
            Replicate::to_all(NetworkTarget::All),
            // This client predicts their own entity (works across transports)
            PredictionTarget::to_all(NetworkTarget::Single(client_id)),
            // Other clients interpolate this entity (works across transports)
            InterpolationTarget::to_all(NetworkTarget::AllExceptSingle(client_id)),
            // Link input to this player - the client owning this can send inputs
            ControlledBy {
                owner: trigger.entity,
                lifetime: Default::default(),
            },
            // Name for debugging
            Name::new(format!("Player_{:?}", client_id)),
        ))
        .id();
    
    info!("   ✅ Spawned player entity {:?}", player_entity);
}

/// Server-side movement - read client inputs and move players
fn movement(
    mut position_query: Query<
        (&mut PlayerPosition, &ActionState<Inputs>),
        // In host-server mode, don't apply to local client's predicted entities
        Without<Predicted>,
    >,
) {
    for (position, inputs) in position_query.iter_mut() {
        shared::shared_movement_behaviour(position, inputs);
    }
}

/// Handle disconnections - despawn player
fn handle_disconnected(
    trigger: On<Add, Disconnected>,
    players: Query<(Entity, &PlayerId)>,
    links: Query<&RemoteId>,
    mut commands: Commands,
) {
    // Get the client ID that disconnected
    let Ok(remote_id) = links.get(trigger.entity) else {
        return;
    };
    let client_id = remote_id.0;
    
    // Find and despawn their player entity
    for (entity, player_id) in players.iter() {
        if player_id.0 == client_id {
            info!("👋 Client {:?} disconnected, despawning player {:?}", client_id, entity);
            commands.entity(entity).despawn();
        }
    }
}

/// Log server state periodically
fn log_server_state(
    server_role: Res<ServerRole>,
    players: Query<(Entity, &PlayerId, &PlayerPosition, Option<&Replicating>)>,
    time: Res<Time>,
    mut last_log: Local<f32>,
) {
    if server_role.is_changed() {
        info!("📊 ServerRole: {:?}", server_role.state);
    }
    
    // Log player count every 5 seconds
    let now = time.elapsed_secs();
    if now - *last_log > 5.0 {
        *last_log = now;
        let count = players.iter().count();
        if count > 0 {
            info!("📊 Server has {} player entities:", count);
            for (entity, id, pos, replicating) in players.iter() {
                let repl_status = if replicating.is_some() { "replicating" } else { "NOT replicating" };
                info!("   {:?} (ID: {:?}) at ({:.1}, {:.1}) - {}", entity, id.0, pos.0.x, pos.0.y, repl_status);
            }
        }
    }
}
