//! Server for projectiles with multi-transport support

use crate::protocol::*;
use crate::shared::*;
use avian2d::prelude::*;
use bevy::prelude::*;
use bevy_enhanced_input::prelude::Complete;
use bevy_enhanced_input::EnhancedInputSystems;
use core::net::{Ipv4Addr, SocketAddr};
use core::time::Duration;
use lightyear::connection::client::Connected;
use lightyear::connection::server::Started;
use lightyear::link::prelude::ViaTransport;
use lightyear::prelude::*;
use lightyear::prelude::server::*;
use lightyear_avian2d::prelude::LagCompensationPlugin;
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

    // Shared protocol + physics
    app.add_plugins(SharedPlugin);

    // Lag compensation for hit detection
    app.add_plugins(LagCompensationPlugin);

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
        app.add_observer(cycle_weapon_type);
        app.add_observer(handle_hits);
        
        // Disable BEI input reading on server (we receive inputs from clients)
        app.configure_sets(PreUpdate, EnhancedInputSystems::Prepare.run_if(|| false));
    }
}

#[derive(Resource, Default)]
struct PlayerRegistry {
    next_id: u64,
    client_to_player: HashMap<Entity, Entity>,
}

fn startup(mut commands: Commands) {
    info!("\n=== Multi-Transport Projectiles Server Starting ===\n");

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

    // Mark server as started
    commands.entity(server).insert(Started);

    // Spawn global context for weapon type
    commands.spawn((
        ClientContext,
        Replicate::to_clients(NetworkTarget::All),
        WeaponType::default(),
        Name::new("ClientContext"),
    ));

    info!("\n=== Projectiles Server Ready ===\n");
}

fn handle_new_client(trigger: On<Add, LinkOf>, mut commands: Commands) {
    commands.entity(trigger.entity).insert((
        ReplicationSender::new(SERVER_REPLICATION_INTERVAL, SendUpdatesMode::SinceLastAck, false),
        ReplicationReceiver::default(),
        Name::from("ClientLink"),
    ));
    info!("🔗 New client link: {:?}", trigger.entity);
}

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
    
    let transport_name = transport_names.get(via_transport.transport)
        .map(|n| n.as_str())
        .unwrap_or("Unknown");

    let player_entity = commands
        .spawn((
            player_bundle(client_id),
            Replicate::to_clients(NetworkTarget::All),
            PredictionTarget::to_clients(NetworkTarget::All),
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

/// Handle weapon cycling
fn cycle_weapon_type(
    trigger: On<Complete<CycleWeapon>>,
    mut global: Query<&mut WeaponType, With<ClientContext>>,
) {
    if let Ok(mut weapon_type) = global.single_mut() {
        *weapon_type = weapon_type.next();
        info!("Switched to weapon: {}", weapon_type.name());
    }
}

/// Handle hit detection events from clients
fn handle_hits(
    trigger: On<RemoteEvent<HitDetected>>,
    mut scores: Query<&mut Score>,
) {
    if let Ok(mut score) = scores.get_mut(trigger.trigger.shooter) {
        info!(?trigger, "Server received hit from client!");
        score.0 += 1;
    }
}
