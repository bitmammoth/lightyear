//! Client module - connects to server and receives replicated player entities.
//!
//! Keyboard Commands:
//! - S: Send message to server
//! - F: Send message to server to forward to OTHER client
//! - P: Show replicated players

use crate::shared::*;
use bevy::prelude::*;
use core::net::Ipv4Addr;
use std::net::SocketAddr;
use lightyear::connection::client::{Connected, Disconnected, Connecting};
use lightyear::netcode::Key;
use lightyear::prelude::client::*;
use lightyear::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Transport {
    #[default]
    Udp,
    WebTransport,
}

#[derive(Resource)]
pub struct ClientConfig {
    pub client_id: u64,
    pub transport: Transport,
    pub cert_digest: Option<String>,
}

pub struct ExampleClientPlugin;

impl Plugin for ExampleClientPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ClientMessageTimer>();
        app.add_systems(Startup, startup);
        app.add_observer(on_connecting);
        app.add_observer(on_connected);
        app.add_observer(on_disconnected);
        app.add_systems(Update, (
            display_new_players,
            count_replicated_entities,
            auto_send_messages,
            receive_server_messages,
        ));
    }
}

/// Timer for automatic message testing from client
#[derive(Resource)]
struct ClientMessageTimer {
    timer: Timer,
    phase: u32,
}

impl Default for ClientMessageTimer {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(5.0, TimerMode::Repeating),
            phase: 0,
        }
    }
}

fn on_connecting(trigger: On<Add, Connecting>, names: Query<&Name>) {
    if let Ok(name) = names.get(trigger.entity) {
        info!("🔄 {} is connecting...", name);
    }
}

fn on_connected(trigger: On<Add, Connected>, names: Query<&Name>) {
    if let Ok(name) = names.get(trigger.entity) {
        info!("✅ {} connected to server!", name);
    }
}

fn on_disconnected(trigger: On<Add, Disconnected>, names: Query<&Name>) {
    if let Ok(name) = names.get(trigger.entity) {
        info!("❌ {} disconnected from server", name);
    }
}

/// Connect to server via UDP or WebTransport
fn startup(mut commands: Commands, config: Res<ClientConfig>) -> Result {
    let client_port = 4000 + (config.client_id % 100) as u16;
    let client_addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), client_port);

    match config.transport {
        Transport::Udp => {
            info!("📡 Connecting via UDP to {}", UDP_SERVER_ADDR);
            let auth = Authentication::Manual {
                server_addr: UDP_SERVER_ADDR,
                client_id: config.client_id,
                private_key: Key::default(),
                protocol_id: 0,
            };
            let client = commands
                .spawn((
                    Client::default(),
                    LocalAddr(client_addr),
                    PeerAddr(UDP_SERVER_ADDR),
                    Link::new(None),
                    ReplicationReceiver::default(),
                    NetcodeClient::new(auth, NetcodeConfig::default())?,
                    UdpIo::default(),
                    Name::new("UdpClient"),
                ))
                .id();
            commands.trigger(Connect { entity: client });
        }
        Transport::WebTransport => {
            info!("🌐 Connecting via WebTransport to {}", WEBTRANSPORT_SERVER_ADDR);
            let auth = Authentication::Manual {
                server_addr: WEBTRANSPORT_SERVER_ADDR,
                client_id: config.client_id,
                private_key: Key::default(),
                protocol_id: 0,
            };
            let cert_digest = config.cert_digest.clone().expect("WebTransport requires cert_digest");
            let client = commands
                .spawn((
                    Client::default(),
                    LocalAddr(client_addr),
                    PeerAddr(WEBTRANSPORT_SERVER_ADDR),
                    Link::new(None),
                    ReplicationReceiver::default(),
                    NetcodeClient::new(auth, NetcodeConfig::default())?,
                    WebTransportClientIo {
                        certificate_digest: cert_digest,
                    },
                    Name::new("WebTransportClient"),
                ))
                .id();
            commands.trigger(Connect { entity: client });
        }
    }
    
    info!("🔗 Connection initiated...\n");
    Ok(())
}

/// Display information about newly replicated player entities
fn display_new_players(
    players: Query<(&Player, &PlayerPosition, &PlayerColor, &Name), Added<Player>>,
) {
    for (player, pos, color, name) in players.iter() {
        let color_rgb = match color.0 {
            Color::Srgba(c) => format!("({:.1}, {:.1}, {:.1})", c.red, c.green, c.blue),
            _ => "unknown".to_string(),
        };
        info!(
            "🎮 Replicated player spawned: {} (ID: {:?}) at ({:.1}, {:.1}) color: {}",
            name, player.id, pos.0.x, pos.0.y, color_rgb
        );
    }
}

/// Log the count of replicated entities when it changes
fn count_replicated_entities(
    players: Query<Entity, With<Player>>,
    mut last_count: Local<usize>,
) {
    let count = players.iter().count();
    if count != *last_count {
        info!("📊 Total replicated players: {}", count);
        *last_count = count;
    }
}

/// Automatically send test messages on a timer
fn auto_send_messages(
    time: Res<Time>,
    mut timer: ResMut<ClientMessageTimer>,
    config: Res<ClientConfig>,
    mut client_sender_query: Query<&mut MessageSender<ClientToServerMessage>, (With<Client>, With<Connected>)>,
    mut forward_query: Query<&mut MessageSender<ForwardMessage>, (With<Client>, With<Connected>)>,
    players: Query<&Player>,
) {
    timer.timer.tick(time.delta());
    
    if !timer.timer.just_finished() {
        return;
    }
    
    // Only test when we have 2 players replicated
    if players.iter().count() < 2 {
        return;
    }
    
    let transport_name = match config.transport {
        Transport::Udp => "UDP",
        Transport::WebTransport => "WebTransport",
    };

    timer.phase = (timer.phase + 1) % 2;
    
    match timer.phase {
        0 => {
            // Send message to server
            info!("📤 {} CLIENT -> SERVER: Sending message", transport_name);
            for mut sender in client_sender_query.iter_mut() {
                sender.send::<DefaultChannel>(ClientToServerMessage {
                    content: format!("Hello from {} client!", transport_name),
                });
            }
        }
        1 => {
            // Forward message to OTHER client via server
            let my_player_id = match config.transport {
                Transport::Udp => 0,
                Transport::WebTransport => 1,
            };
            let target_player_id = if my_player_id == 0 { 1 } else { 0 };
            
            info!("📤 {} CLIENT -> SERVER (forward to Player {}): Sending forward request", 
                  transport_name, target_player_id);
            for mut sender in forward_query.iter_mut() {
                sender.send::<DefaultChannel>(ForwardMessage {
                    target_player_id,
                    content: format!("Forwarded message from {} client!", transport_name),
                });
            }
        }
        _ => {}
    }
}

/// Receive messages from server
fn receive_server_messages(
    mut direct_receiver: Query<&mut MessageReceiver<ServerToClientMessage>, With<Client>>,
    mut broadcast_receiver: Query<&mut MessageReceiver<BroadcastMessage>, With<Client>>,
    config: Res<ClientConfig>,
) {
    let transport_name = match config.transport {
        Transport::Udp => "UDP",
        Transport::WebTransport => "WebTransport",
    };

    for mut receiver in direct_receiver.iter_mut() {
        for msg in receiver.receive() {
            info!("📥 {} CLIENT <- SERVER (direct): {}", transport_name, msg.content);
        }
    }

    for mut receiver in broadcast_receiver.iter_mut() {
        for msg in receiver.receive() {
            info!("📥 {} CLIENT <- SERVER (broadcast): {}", transport_name, msg.content);
        }
    }
}