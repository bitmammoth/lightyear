//! Server module - demonstrates entity replication with multiple transports.
//!
//! This server runs both UDP and WebTransport, allowing clients to connect
//! via either transport and see replicated player entities.
//!
//! Architecture:
//! - ONE Server entity (the logical server, relationship target for all LinkOfs)
//! - UDP transport entity with TransportOf pointing to Server
//! - WebTransport transport entity with TransportOf pointing to Server
//! - All client LinkOfs point to the ONE Server entity
//!
//! Keyboard Commands:
//! - 1: Send message to UDP client (player 0)
//! - 2: Send message to WebTransport client (player 1)
//! - B: Broadcast message to ALL clients
//! - C: Show connected clients

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
        app.init_resource::<MessageTestTimer>();
        app.add_systems(Startup, startup);
        app.add_observer(handle_new_client);
        app.add_observer(handle_connected);
        app.add_observer(handle_client_disconnected);
        app.add_systems(Update, (
            log_server_role,
            log_connected_clients,
            auto_send_messages,
            receive_client_messages,
        ));
    }
}

/// Timer for automatic message testing
#[derive(Resource)]
struct MessageTestTimer {
    timer: Timer,
    phase: u32,
}

impl Default for MessageTestTimer {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(3.0, TimerMode::Repeating),
            phase: 0,
        }
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

    // 1. Spawn ONE logical server - all clients will have LinkOf pointing here
    let server = commands
        .spawn((
            Server::default(),
            Name::new("GameServer"),
        ))
        .id();
    info!("🎯 Spawned logical Server entity: {:?}", server);

    // 2. UDP Transport - feeds connections to the server
    let udp_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), UDP_PORT);
    let udp_transport = commands
        .spawn((
            NetcodeServer::new(NetcodeConfig::default()),
            LocalAddr(udp_addr),
            ServerUdpIo::default(),
            TransportOf::new(server),  // Points to our server
            Name::new("UdpTransport"),
        ))
        .id();
    commands.trigger(Start { entity: udp_transport });
    info!("📡 UDP transport starting on port {} -> Server {:?}", UDP_PORT, server);

    // 3. WebTransport - also feeds connections to the SAME server
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
            TransportOf::new(server),  // Same server!
            Name::new("WebTransportTransport"),
        ))
        .id();
    commands.trigger(Start { entity: wt_transport });
    info!("🌐 WebTransport transport starting on port {} -> Server {:?}", WEBTRANSPORT_PORT, server);

    info!("\n✅ Server initialized with 2 transports. All clients connect to Server {:?}\n", server);
    Ok(())
}

/// Log ServerRole state changes
fn log_server_role(server_role: Res<ServerRole>) {
    if server_role.is_changed() {
        info!("📊 ServerRole state: {:?}", server_role.state);
    }
}

/// Log connected clients - demonstrates that all clients are on ONE server
fn log_connected_clients(
    server_query: Query<(Entity, &Server)>,
    input: Res<ButtonInput<KeyCode>>,
) {
    if input.just_pressed(KeyCode::KeyC) {
        for (entity, server) in &server_query {
            info!("🖥️ Server {:?} has {} connected clients", entity, server.collection().len());
            for (i, client) in server.collection().iter().enumerate() {
                info!("   Client {}: {:?}", i, client);
            }
        }
    }
}

/// Automatically send test messages on a timer
fn auto_send_messages(
    time: Res<Time>,
    mut timer: ResMut<MessageTestTimer>,
    registry: Res<PlayerRegistry>,
    mut client_sender_query: Query<(Entity, &mut MessageSender<ServerToClientMessage>), With<ClientOf>>,
    mut broadcast_sender_query: Query<&mut MessageSender<BroadcastMessage>, With<ClientOf>>,
) {
    timer.timer.tick(time.delta());
    
    if !timer.timer.just_finished() {
        return;
    }
    
    // Only test when we have 2 clients connected
    if registry.client_to_player.len() < 2 {
        return;
    }
    
    timer.phase = (timer.phase + 1) % 3;
    
    match timer.phase {
        0 => {
            // Send to UDP client (Player 0)
            if let Some((&link_entity, &(player_id, _))) = registry.client_to_player.iter().find(|(_, (pid, _))| pid.0 == 0) {
                info!("📤 SERVER -> UDP Client (Player {}): Sending targeted message", player_id.0);
                if let Ok((_, mut sender)) = client_sender_query.get_mut(link_entity) {
                    sender.send::<DefaultChannel>(ServerToClientMessage {
                        content: format!("Hello UDP client! (Player {})", player_id.0),
                    });
                }
            }
        }
        1 => {
            // Send to WebTransport client (Player 1)
            if let Some((&link_entity, &(player_id, _))) = registry.client_to_player.iter().find(|(_, (pid, _))| pid.0 == 1) {
                info!("📤 SERVER -> WebTransport Client (Player {}): Sending targeted message", player_id.0);
                if let Ok((_, mut sender)) = client_sender_query.get_mut(link_entity) {
                    sender.send::<DefaultChannel>(ServerToClientMessage {
                        content: format!("Hello WebTransport client! (Player {})", player_id.0),
                    });
                }
            }
        }
        2 => {
            // Broadcast to ALL clients
            let client_count = registry.client_to_player.len();
            info!("📢 SERVER -> ALL ({} clients): Broadcasting message", client_count);
            for mut sender in broadcast_sender_query.iter_mut() {
                sender.send::<DefaultChannel>(BroadcastMessage {
                    content: format!("Broadcast to all {} clients!", client_count),
                });
            }
        }
        _ => {}
    }
}

/// Receive messages from clients
fn receive_client_messages(
    mut client_msg_query: Query<(Entity, &mut MessageReceiver<ClientToServerMessage>), With<ClientOf>>,
    mut forward_query: Query<(Entity, &mut MessageReceiver<ForwardMessage>), With<ClientOf>>,
    registry: Res<PlayerRegistry>,
    mut sender_query: Query<&mut MessageSender<ServerToClientMessage>, With<ClientOf>>,
) {
    // Handle regular client messages
    for (entity, mut receiver) in client_msg_query.iter_mut() {
        for msg in receiver.receive() {
            let from_player = registry.client_to_player.get(&entity)
                .map(|(pid, _)| pid.0)
                .unwrap_or(999);
            info!("📥 SERVER <- Client (Player {}): {}", from_player, msg.content);
        }
    }

    // Handle forward requests
    for (entity, mut receiver) in forward_query.iter_mut() {
        for msg in receiver.receive() {
            let from_player = registry.client_to_player.get(&entity)
                .map(|(pid, _)| pid.0)
                .unwrap_or(999);
            let target_player_id = msg.target_player_id;
            
            info!("🔀 SERVER: Forwarding from Player {} to Player {}: {}", 
                  from_player, target_player_id, msg.content);
            
            // Find target client
            if let Some((&target_link, _)) = registry.client_to_player.iter()
                .find(|(_, (pid, _))| pid.0 == target_player_id) 
            {
                if let Ok(mut sender) = sender_query.get_mut(target_link) {
                    sender.send::<DefaultChannel>(ServerToClientMessage {
                        content: format!("[Forwarded from Player {}]: {}", from_player, msg.content),
                    });
                    info!("   ✅ Forwarded to Player {}", target_player_id);
                }
            } else {
                info!("   ⚠️ Target Player {} not found", target_player_id);
            }
        }
    }
}