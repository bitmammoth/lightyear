//! Client module - connects to server via selected transport.

use crate::shared::*;
use bevy::prelude::*;
use core::net::Ipv4Addr;
use std::net::SocketAddr;
use lightyear::connection::client::{Connected, Disconnected, Connecting};
use lightyear::netcode::Key;
use lightyear::prelude::client::*;
use lightyear::prelude::*;
use lightyear::prediction::manager::PredictionManager;
use lightyear::websocket::prelude::client::ClientConfig as WebSocketClientConfig;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Transport {
    #[default]
    Udp,
    WebTransport,
    WebSocket,
}

#[derive(Resource)]
pub struct ClientConfig {
    pub client_id: u64,
    pub transport: Transport,
    pub cert_digest: Option<String>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            client_id: 0,
            transport: Transport::Udp,
            cert_digest: None,
        }
    }
}

pub struct ExampleClientPlugin;

impl Plugin for ExampleClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, startup);
        app.add_observer(on_connecting);
        app.add_observer(on_connected);
        app.add_observer(on_disconnected);
    }
}

fn on_connecting(trigger: On<Add, Connecting>, names: Query<&Name>) {
    if let Ok(name) = names.get(trigger.entity) {
        info!("🔄 {} is connecting...", name);
    }
}

fn on_connected(trigger: On<Add, Connected>, names: Query<&Name>) {
    if let Ok(name) = names.get(trigger.entity) {
        info!("✅ {} connected!", name);
    }
}

fn on_disconnected(trigger: On<Add, Disconnected>, names: Query<&Name>) {
    if let Ok(name) = names.get(trigger.entity) {
        warn!("❌ {} disconnected", name);
    }
}

/// Connect to server using the configured transport
fn startup(mut commands: Commands, config: Res<ClientConfig>) -> Result {
    commands.spawn(Camera2d);

    let (server_addr, transport_name) = match config.transport {
        Transport::Udp => (UDP_SERVER_ADDR, "UDP"),
        Transport::WebTransport => (WT_SERVER_ADDR, "WebTransport"),
        Transport::WebSocket => (WS_SERVER_ADDR, "WebSocket"),
    };

    info!("Connecting via {} to {}...", transport_name, server_addr);

    let auth = Authentication::Manual {
        server_addr,
        client_id: config.client_id,
        private_key: Key::default(),
        protocol_id: 0,
    };

    // Choose a random local port for the client
    let client_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0);

    let client = match config.transport {
        Transport::Udp => {
            commands.spawn((
                Client::default(),
                LocalAddr(client_addr),
                PeerAddr(server_addr),
                Link::new(None),
                ReplicationReceiver::default(),
                PredictionManager::default(),
                NetcodeClient::new(auth, NetcodeConfig::default())?,
                UdpIo::default(),
                Name::new(format!("UdpClient-{}", config.client_id)),
            )).id()
        }
        Transport::WebTransport => {
            let cert_digest = config.cert_digest.clone()
                .expect("WebTransport requires --cert <DIGEST> argument");
            commands.spawn((
                Client::default(),
                LocalAddr(client_addr),
                PeerAddr(server_addr),
                Link::new(None),
                ReplicationReceiver::default(),
                PredictionManager::default(),
                NetcodeClient::new(auth, NetcodeConfig::default())?,
                WebTransportClientIo {
                    certificate_digest: cert_digest,
                },
                Name::new(format!("WebTransportClient-{}", config.client_id)),
            )).id()
        }
        Transport::WebSocket => {
            let ws_config = WebSocketClientConfig::builder()
                .with_no_cert_validation();
            commands.spawn((
                Client::default(),
                LocalAddr(client_addr),
                PeerAddr(server_addr),
                Link::new(None),
                ReplicationReceiver::default(),
                PredictionManager::default(),
                NetcodeClient::new(auth, NetcodeConfig::default())?,
                WebSocketClientIo {
                    config: ws_config,
                    scheme: WebSocketScheme::Secure,
                },
                Name::new(format!("WebSocketClient-{}", config.client_id)),
            )).id()
        }
    };

    commands.trigger(Connect { entity: client });
    info!("🚀 {} client connecting to {}", transport_name, server_addr);

    Ok(())
}
