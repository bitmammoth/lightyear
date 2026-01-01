//! Server module - demonstrates entity replication with multiple transports.
//!
//! This server runs UDP, WebTransport, and WebSocket transports, allowing clients
//! to connect via any transport and see replicated player entities.
//!
//! Architecture:
//! - ONE Server entity (the logical server, relationship target for all LinkOfs)
//! - UDP transport entity with TransportOf pointing to Server
//! - WebTransport transport entity with TransportOf pointing to Server
//! - WebSocket transport entity with TransportOf pointing to Server
//! - All client LinkOfs point to the ONE Server entity
//!
//! Keyboard Commands:
//! - 1: Send message to UDP client (player 0)
//! - 2: Send message to WebTransport client (player 1)
//! - 3: Send message to WebSocket client (player 2)
//! - B: Broadcast message to ALL clients
//! - C: Show connected clients

use crate::shared::*;
use bevy::prelude::*;
use core::net::Ipv4Addr;
use std::collections::HashMap;
use std::net::SocketAddr;
use lightyear::prelude::server::*;
use lightyear::prelude::*;
use lightyear::prelude::input::native::ActionState;
use lightyear::input::input_buffer::InputBuffer;
use lightyear::link::prelude::ViaTransport;
use lightyear::connection::server::Started;

pub struct ExampleServerPlugin;

impl Plugin for ExampleServerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerRegistry>();
        app.init_resource::<MessageTestTimer>();
        app.add_systems(Startup, startup);
        app.add_observer(handle_new_client);
        app.add_observer(handle_connected);
        app.add_observer(handle_client_disconnected);
        // Movement in FixedUpdate for determinism
        app.add_systems(FixedUpdate, movement);
        app.add_systems(Update, (
            log_server_role,
            log_connected_clients,
            auto_send_messages,
            receive_client_messages,
            debug_action_states,
            debug_input_messages,
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

/// Track connected players and their entities by name
#[derive(Resource, Default)]
struct PlayerRegistry {
    next_id: u64,
    /// Maps client link entity -> (player_name, player entity)
    client_to_player: HashMap<Entity, (String, Entity)>,
    /// Maps player name -> client link entity (for forwarding by name)
    name_to_client: HashMap<String, Entity>,
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
    client_query: Query<(&RemoteId, &LinkOf, &ViaTransport), With<ClientOf>>,
    transport_names: Query<&Name>,
    mut commands: Commands,
    mut registry: ResMut<PlayerRegistry>,
) {
    let Ok((remote_id, link_of, via_transport)) = client_query.get(trigger.entity) else {
        warn!("⚠️ Connected trigger but couldn't get client components for {:?}", trigger.entity);
        return;
    };
    let client_id = remote_id.0;
    let server_entity = link_of.server;
    let player_id = registry.next_player_id();

    info!("📋 Server connecting client {:?} (RemoteId: {:?})", trigger.entity, client_id);
    
    // Determine player name from transport (UDP, WT, WS)
    let player_name = transport_names.get(via_transport.transport)
        .map(|n| {
            let name = n.as_str();
            if name.contains("Udp") { "UDP".to_string() }
            else if name.contains("WebTransport") { "WT".to_string() }
            else if name.contains("WebSocket") { "WS".to_string() }
            else { format!("Player_{}", player_id.0) }
        })
        .unwrap_or_else(|_| format!("Player_{}", player_id.0));
    
    info!("🎮 Client '{}' connected via server {:?}", player_name, server_entity);
    
    // Choose a color based on transport
    let color = match player_name.as_str() {
        "UDP" => Color::srgb(1.0, 0.2, 0.2),   // Red
        "WT" => Color::srgb(0.2, 0.8, 0.2),    // Green
        "WS" => Color::srgb(0.2, 0.4, 1.0),    // Blue
        _ => Color::srgb(0.8, 0.8, 0.8),       // Gray
    };
    
    // Spawn the player entity with replication to ALL clients
    // Also add PredictionTarget (owner predicts), InterpolationTarget (others interpolate), and ControlledBy
    // NOTE: Don't add ActionState here - it will be added automatically when the first input message is received
    let player_entity = commands
        .spawn((
            player_id,  // Flat PlayerId - matches simple_box pattern
            PlayerPosition(Vec2::new(
                match player_name.as_str() {
                    "UDP" => -100.0,
                    "WT" => 0.0,
                    "WS" => 100.0,
                    _ => 0.0,
                },
                0.0,
            )),
            PlayerColor(color),
            // Replicate to all clients across all transports
            Replicate::to_all(NetworkTarget::All),
            // The owning client will predict this entity
            PredictionTarget::to_all(NetworkTarget::Single(client_id)),
            // Other clients will interpolate this entity
            InterpolationTarget::to_all(NetworkTarget::AllExceptSingle(client_id)),
            // Track which client controls this entity (for input processing)
            ControlledBy {
                owner: trigger.entity,
                lifetime: Default::default(),
            },
            Name::new(player_name.clone()),
        ))
        .id();

    registry.client_to_player.insert(trigger.entity, (player_name.clone(), player_entity));
    registry.name_to_client.insert(player_name.clone(), trigger.entity);
    info!("   ✅ Spawned player '{}' entity {:?}", player_name, player_entity);
}

/// Handle client disconnections
fn handle_client_disconnected(
    trigger: On<Add, Disconnected>,
    mut commands: Commands,
    mut registry: ResMut<PlayerRegistry>,
) {
    let entity = trigger.entity;
    
    // Remove player entity if it exists
    if let Some((player_name, player_entity)) = registry.client_to_player.remove(&entity) {
        info!("👋 Client '{}' disconnected", player_name);
        registry.name_to_client.remove(&player_name);
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

    // 4. WebSocket - third transport, also feeds connections to the SAME server
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
            TransportOf::new(server),  // Same server!
            Name::new("WebSocketTransport"),
        ))
        .id();
    commands.trigger(Start { entity: ws_transport });
    info!("🔌 WebSocket transport starting on port {} -> Server {:?}", WEBSOCKET_PORT, server);

    // 5. Manually add Started to the Server entity
    // In single-transport setups, Start trigger adds Started automatically because Server has NetcodeServer.
    // In multi-transport, the Server entity doesn't have NetcodeServer (only transports do),
    // so we need to add Started manually to enable input processing.
    commands.entity(server).insert(Started);
    info!("🚀 Server entity {:?} marked as Started", server);

    info!("\n✅ Server initialized with 3 transports. All clients connect to Server {:?}\n", server);
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
            // Send to UDP client
            if let Some(&link_entity) = registry.name_to_client.get("UDP") {
                info!("📤 SERVER -> UDP: Sending targeted message");
                if let Ok((_, mut sender)) = client_sender_query.get_mut(link_entity) {
                    sender.send::<DefaultChannel>(ServerToClientMessage {
                        content: format!("Hello from server!"),
                    });
                }
            }
        }
        1 => {
            // Send to WebTransport client
            if let Some(&link_entity) = registry.name_to_client.get("WT") {
                info!("📤 SERVER -> WT: Sending targeted message");
                if let Ok((_, mut sender)) = client_sender_query.get_mut(link_entity) {
                    sender.send::<DefaultChannel>(ServerToClientMessage {
                        content: format!("Hello from server!"),
                    });
                }
            }
        }
        2 => {
            // Broadcast to ALL clients
            let client_count = registry.name_to_client.len();
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
    mut sender_query: Query<&mut MessageSender<ForwardedMessage>, With<ClientOf>>,
) {
    // Handle regular client messages
    for (entity, mut receiver) in client_msg_query.iter_mut() {
        for msg in receiver.receive() {
            let from_name = registry.client_to_player.get(&entity)
                .map(|(name, _)| name.as_str())
                .unwrap_or("Unknown");
            info!("📥 SERVER <- {}: {}", from_name, msg.content);
        }
    }

    // Handle forward requests (by target name)
    for (_entity, mut receiver) in forward_query.iter_mut() {
        for msg in receiver.receive() {
            let from_name = &msg.from_name;
            let target_name = &msg.target_name;
            let msg_type = if msg.is_reply { "reply" } else { "message" };
            
            info!("🔀 SERVER: {} -> {} ({}): {}", from_name, target_name, msg_type, msg.content);
            
            // Find target client by name
            if let Some(&target_link) = registry.name_to_client.get(target_name) {
                if let Ok(mut sender) = sender_query.get_mut(target_link) {
                    sender.send::<DefaultChannel>(ForwardedMessage {
                        from_name: from_name.clone(),
                        content: msg.content.clone(),
                        is_reply: msg.is_reply,
                    });
                    info!("   ✅ Delivered to {}", target_name);
                }
            } else {
                info!("   ⚠️ Target '{}' not found", target_name);
            }
        }
    }
}

/// Read client inputs and move players on server
/// This gives a basis for other clients to interpolate
/// NOTE: ActionState is added to the entity when the first input message is received
fn movement(
    timeline: Res<LocalTimeline>,
    mut position_query: Query<
        (Entity, &mut PlayerPosition, Option<&ActionState<Inputs>>, &Name),
        // In host-server mode, don't apply to local client's entities
        // because they are already moved by the client plugin
        Without<Predicted>,
    >,
    mut logged: Local<bool>,
) {
    let tick = timeline.tick();

    // Log once to show we're checking for entities
    if !*logged && position_query.iter().count() > 0 {
        info!("🔍 Server movement system found {} player entities", position_query.iter().count());
        *logged = true;
    }

    for (entity, position, inputs, name) in position_query.iter_mut() {
        // ActionState is only present after the first input message is received
        if let Some(inputs) = inputs {
            if let Inputs::Direction(ref dir) = &inputs.0 {
                if !dir.is_none() {
                    info!("🎮 Server processing input for {} (entity {:?}) at tick {:?}: {:?}",
                        name, entity, tick, dir);
                }
            }
            shared_movement_behaviour(position, inputs);
        }
    }
}

/// Debug system to check if ActionState is being updated by the InputPlugin
fn debug_action_states(
    query: Query<(Entity, Option<&ActionState<Inputs>>, &Name), With<PlayerId>>,
    mut timer: Local<Option<Timer>>,
    time: Res<Time>,
) {
    let timer = timer.get_or_insert_with(|| Timer::from_seconds(2.0, TimerMode::Repeating));
    timer.tick(time.delta());

    if timer.just_finished() {
        for (entity, action_state, name) in query.iter() {
            match action_state {
                Some(state) => info!("📊 Server ActionState for {} ({:?}): {:?}", name, entity, state.0),
                None => info!("📊 Server ActionState for {} ({:?}): <not yet received>", name, entity),
            }
        }
    }
}

/// Debug system to check for incoming input messages and input buffers
fn debug_input_messages(
    // Check for InputBuffer on player entities
    player_buffers: Query<(Entity, Option<&InputBuffer<ActionState<Inputs>, Inputs>>, Option<&ActionState<Inputs>>, &Name), With<PlayerId>>,
    // Check for client links
    client_links: Query<(Entity, &Name), With<ClientOf>>,
    // Check for servers with Started
    servers: Query<(Entity, Has<Started>, &Name), With<Server>>,
    timeline: Res<LocalTimeline>,
    mut timer: Local<Option<Timer>>,
    time: Res<Time>,
) {
    let timer = timer.get_or_insert_with(|| Timer::from_seconds(2.0, TimerMode::Repeating));
    timer.tick(time.delta());

    if timer.just_finished() {
        let tick = timeline.tick();
        
        // Check servers
        for (entity, has_started, name) in servers.iter() {
            info!("🖥️ Server {} ({:?}): Started={}, current_tick={:?}", name, entity, has_started, tick);
        }
        
        // Check for active client links
        for (entity, name) in client_links.iter() {
            info!("🔗 Server has client link: {} ({:?})", name, entity);
        }

        // Check input buffers on player entities
        for (entity, buffer, action_state, name) in player_buffers.iter() {
            match buffer {
                Some(b) => {
                    info!("📦 InputBuffer for {} ({:?}): start={:?}, len={}, tick={:?}",
                        name, entity, b.start_tick, b.len(), tick);
                },
                None => {}
            }
            match action_state {
                Some(s) => info!("✅ ActionState for {} ({:?}): {:?}", name, entity, s),
                None => {}
            }
        }
    }
}