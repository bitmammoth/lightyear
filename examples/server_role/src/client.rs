//! Client module - connects via UDP or WebTransport.

use crate::shared::*;
use bevy::prelude::*;
use core::net::Ipv4Addr;
use std::net::SocketAddr;
use lightyear::netcode::Key;
use lightyear::prelude::client::*;
use lightyear::prelude::*;

/// Transport type for the client
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transport {
    Udp,
    WebTransport,
}

#[derive(Resource)]
pub struct ClientConfig {
    pub transport: Transport,
    pub client_id: u64,
    pub cert_digest: Option<String>,
}

pub struct ExampleClientPlugin;

impl Plugin for ExampleClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, startup);
        app.add_observer(on_connecting);
        app.add_observer(on_connected);
        app.add_observer(on_disconnected);
        app.add_systems(Update, (send_pings, handle_pongs));
    }
}

fn on_connecting(trigger: On<Add, Connecting>, names: Query<&Name>) {
    if let Ok(name) = names.get(trigger.entity) {
        info!("🔄 {} is connecting...", name);
    }
}

fn on_connected(trigger: On<Add, Connected>, names: Query<&Name>) {
    if let Ok(name) = names.get(trigger.entity) {
        info!("✅ {} is now connected!", name);
    }
}

fn on_disconnected(trigger: On<Add, Disconnected>, names: Query<&Name>) {
    if let Ok(name) = names.get(trigger.entity) {
        info!("❌ {} disconnected", name);
    }
}

#[derive(Resource)]
struct PingState {
    timer: Timer,
    sequence: u32,
}

/// Connect to server based on configured transport
fn startup(mut commands: Commands, config: Res<ClientConfig>) -> Result {
    commands.insert_resource(PingState {
        timer: Timer::from_seconds(2.0, TimerMode::Repeating),
        sequence: 0,
    });

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
            let digest = config.cert_digest.clone()
                .expect("WebTransport requires certificate digest");
            info!("🌐 Connecting via WebTransport to {}", WEBTRANSPORT_SERVER_ADDR);
            info!("🔐 Using cert digest: {}", digest);
            
            let auth = Authentication::Manual {
                server_addr: WEBTRANSPORT_SERVER_ADDR,
                client_id: config.client_id,
                private_key: Key::default(),
                protocol_id: 0,
            };
            let client = commands
                .spawn((
                    Client::default(),
                    LocalAddr(client_addr),
                    PeerAddr(WEBTRANSPORT_SERVER_ADDR),
                    Link::new(None),
                    ReplicationReceiver::default(),
                    NetcodeClient::new(auth, NetcodeConfig::default())?,
                    WebTransportClientIo {
                        certificate_digest: digest,
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

/// Send periodic ping messages
fn send_pings(
    time: Res<Time>,
    mut ping_state: ResMut<PingState>,
    config: Res<ClientConfig>,
    mut query: Query<&mut MessageSender<PingMessage>, (With<Client>, With<Connected>)>,
) {
    ping_state.timer.tick(time.delta());
    
    if ping_state.timer.just_finished() {
        for mut sender in query.iter_mut() {
            ping_state.sequence += 1;
            let ping = PingMessage {
                client_id: config.client_id,
                sequence: ping_state.sequence,
            };
            
            sender.send::<ReliableChannel>(ping);
            info!("📤 Sent ping (seq: {})", ping_state.sequence);
        }
    }
}

/// Handle pong responses
fn handle_pongs(
    mut query: Query<&mut MessageReceiver<PongMessage>, With<Client>>,
) {
    for mut receiver in query.iter_mut() {
        for pong in receiver.receive() {
            info!("📨 Received pong (seq: {})", pong.sequence);
        }
    }
}
