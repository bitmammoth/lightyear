//! Server implementation with multi-transport support
//!
//! Runs UDP, WebTransport, and WebSocket transports simultaneously.
//! All clients connect to the same logical server regardless of transport.

use crate::protocol::*;
use crate::shared::{self, *};
use bevy::prelude::*;
use bevy_enhanced_input::prelude::Fire;
use core::net::{Ipv4Addr, SocketAddr};
use core::time::Duration;
use lightyear::connection::client::Connected;
use lightyear::connection::server::Started;
use lightyear::link::prelude::ViaTransport;
use lightyear::prelude::server::*;
use lightyear::prelude::*;
use std::collections::HashMap;

pub fn run_server() {
    let mut app = App::new();
    
    app.add_plugins(MinimalPlugins.set(bevy::app::ScheduleRunnerPlugin::run_loop(
        Duration::from_secs_f64(1.0 / FIXED_TIMESTEP_HZ),
    )));
    app.add_plugins(bevy::log::LogPlugin::default());
    
    // Lightyear server plugin
    app.add_plugins(lightyear::prelude::server::ServerPlugins {
        tick_duration: Duration::from_secs_f64(1.0 / FIXED_TIMESTEP_HZ),
    });
    
    // Shared protocol
    app.add_plugins(SharedPlugin);
    
    // Server-specific plugin
    app.add_plugins(ExampleServerPlugin);
    
    app.run();
}

pub struct ExampleServerPlugin;

impl Plugin for ExampleServerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerRegistry>();
        app.add_systems(Startup, startup);
        app.add_observer(handle_new_client);
        app.add_observer(handle_connected);
        app.add_observer(handle_disconnected);
        app.add_observer(movement);
    }
}

#[derive(Resource, Default)]
struct PlayerRegistry {
    next_id: u64,
    client_to_player: HashMap<Entity, Entity>,
}

fn startup(mut commands: Commands) {
    info!("\n=== Multi-Transport BEI Server Starting ===\n");

    // 1. Spawn ONE logical server
    let server = commands
        .spawn((Server::default(), Name::new("Server")))
        .id();

    // 2. UDP transport
    let udp_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), SERVER_UDP_PORT);
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
    info!("📡 UDP transport on port {}", SERVER_UDP_PORT);

    // 3. WebTransport transport
    let wt_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), SERVER_WEBTRANSPORT_PORT);
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
    info!("🌐 WebTransport transport on port {}", SERVER_WEBTRANSPORT_PORT);

    // 4. WebSocket transport
    let ws_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), SERVER_WEBSOCKET_PORT);
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
    info!("🔌 WebSocket transport on port {}", SERVER_WEBSOCKET_PORT);

    // Mark server as started (multi-transport requires manual Started)
    commands.entity(server).insert(Started);

    info!("\n=== Server Ready ===\n");
}

/// Add ReplicationSender/Receiver when a new client link is created
fn handle_new_client(trigger: On<Add, LinkOf>, mut commands: Commands) {
    commands.entity(trigger.entity).insert((
        ReplicationSender::new(SERVER_REPLICATION_INTERVAL, SendUpdatesMode::SinceLastAck, false),
        // Need ReplicationReceiver because Action entities are spawned on client and replicated to server
        ReplicationReceiver::default(),
        Name::from("ClientLink"),
    ));
    info!("🔗 New client link: {:?}", trigger.entity);
}

/// Spawn player entity when client fully connects
fn handle_connected(
    trigger: On<Add, Connected>,
    client_query: Query<(&RemoteId, &ViaTransport), With<ClientOf>>,
    transport_names: Query<&Name>,
    mut commands: Commands,
    mut registry: ResMut<PlayerRegistry>,
) {
    let Ok((remote_id, via_transport)) = client_query.get(trigger.entity) else {
        return;
    };
    let client_id = remote_id.0;
    
    // Determine transport name for visual distinction
    let transport_name = transport_names.get(via_transport.transport)
        .map(|n| n.as_str())
        .unwrap_or("Unknown");
    
    let (color, x_offset) = if transport_name.contains("Udp") {
        (Color::srgb(1.0, 0.2, 0.2), -100.0)   // Red, left
    } else if transport_name.contains("WebTransport") {
        (Color::srgb(0.2, 0.8, 0.2), 0.0)      // Green, center
    } else if transport_name.contains("WebSocket") {
        (Color::srgb(0.2, 0.4, 1.0), 100.0)    // Blue, right
    } else {
        (Color::srgb(0.8, 0.8, 0.8), 0.0)      // Gray, center
    };

    let player_entity = commands
        .spawn((
            Player,  // The BEI context component
            PlayerId(client_id),
            PlayerPosition(Vec2::new(x_offset, 0.0)),
            PlayerColor(color),
            Replicate::to_clients(NetworkTarget::All),
            // Owner predicts, others interpolate
            PredictionTarget::to_clients(NetworkTarget::Single(client_id)),
            InterpolationTarget::to_clients(NetworkTarget::AllExceptSingle(client_id)),
            ControlledBy {
                owner: trigger.entity,
                lifetime: Default::default(),
            },
            Name::new(format!("Player_{}", registry.next_id)),
        ))
        .id();

    registry.next_id += 1;
    registry.client_to_player.insert(trigger.entity, player_entity);
    
    info!("🎮 Client {:?} connected via {} - spawned player {:?}", 
          client_id, transport_name, player_entity);
}

/// Clean up when client disconnects
fn handle_disconnected(
    trigger: On<Add, Disconnected>,
    mut commands: Commands,
    mut registry: ResMut<PlayerRegistry>,
) {
    if let Some(player_entity) = registry.client_to_player.remove(&trigger.entity) {
        info!("👋 Client disconnected, despawning player {:?}", player_entity);
        commands.entity(player_entity).despawn();
    }
}

/// Server-side movement handler - applies inputs from BEI
fn movement(
    trigger: On<Fire<Movement>>,
    mut position_query: Query<&mut PlayerPosition, Without<Predicted>>,
) {
    if let Ok(position) = position_query.get_mut(trigger.context) {
        shared::shared_movement_behaviour(position, trigger.value);
    }
}
