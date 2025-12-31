//! Client module - connects to server and receives replicated player entities.

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
        app.add_systems(Startup, startup);
        app.add_observer(on_connecting);
        app.add_observer(on_connected);
        app.add_observer(on_disconnected);
        app.add_systems(Update, (display_new_players, count_replicated_entities));
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
