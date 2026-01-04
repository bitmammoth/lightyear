//! Client module - multi-transport client replication example.
//!
//! Spawns a cursor entity and replicates it to the server.
//! The cursor follows the mouse position.

use crate::protocol::*;
use crate::shared::*;
use bevy::prelude::*;
use core::net::Ipv4Addr;
use std::net::SocketAddr;
use lightyear::connection::client::{Connected, Disconnected, Connecting};
use lightyear::netcode::Key;
use lightyear::prelude::client::*;
use lightyear::prelude::*;
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

pub struct ExampleClientPlugin;

impl Plugin for ExampleClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, startup);
        app.add_observer(on_connecting);
        app.add_observer(on_connected);
        app.add_observer(on_disconnected);
        app.add_systems(Update, (cursor_movement, handle_interpolated_spawn));
    }
}

fn on_connecting(trigger: On<Add, Connecting>, names: Query<&Name>) {
    if let Ok(name) = names.get(trigger.entity) {
        info!("🔄 {} is connecting...", name);
    }
}

/// When connected, spawn cursor entity with Replicate::to_server()
fn on_connected(
    trigger: On<Add, Connected>,
    names: Query<&Name>,
    client_query: Query<&LocalId, With<Client>>,
    mut commands: Commands,
) {
    if let Ok(name) = names.get(trigger.entity) {
        info!("✅ {} connected to server!", name);
    }
    
    // Get our local ID to color the cursor
    if let Ok(local_id) = client_query.get(trigger.entity) {
        let client_id = local_id.0;
        let color = color_from_id(client_id);
        
        // Spawn cursor entity - CLIENT-AUTHORITATIVE
        // Replicate::to_server() means WE send updates TO the server
        let cursor = commands
            .spawn((
                PlayerId(client_id),
                CursorPosition(Vec2::ZERO),
                PlayerColor(color),
                Replicate::to_server(),  // Client -> Server replication
                Name::new("MyCursor"),
            ))
            .id();
        
        info!("🖱️ Spawned cursor entity {:?} for client {:?}", cursor, client_id);
    }
}

fn on_disconnected(trigger: On<Add, Disconnected>, names: Query<&Name>) {
    if let Ok(name) = names.get(trigger.entity) {
        info!("❌ {} disconnected from server", name);
    }
}

/// Connect to server via selected transport
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
                    // Receive replication FROM server (other clients' cursors)
                    ReplicationReceiver::default(),
                    // Send replication TO server (our cursor)
                    ReplicationSender::new(
                        SERVER_REPLICATION_INTERVAL,
                        SendUpdatesMode::SinceLastAck,
                        false,
                    ),
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
            let cert_digest_raw = config.cert_digest.clone().expect("WebTransport requires cert_digest");
            let cert_digest = cert_digest_raw.replace(":", "");
            let client = commands
                .spawn((
                    Client::default(),
                    LocalAddr(client_addr),
                    PeerAddr(WEBTRANSPORT_SERVER_ADDR),
                    Link::new(None),
                    ReplicationReceiver::default(),
                    ReplicationSender::new(
                        SERVER_REPLICATION_INTERVAL,
                        SendUpdatesMode::SinceLastAck,
                        false,
                    ),
                    NetcodeClient::new(auth, NetcodeConfig::default())?,
                    WebTransportClientIo {
                        certificate_digest: cert_digest,
                    },
                    Name::new("WebTransportClient"),
                ))
                .id();
            commands.trigger(Connect { entity: client });
        }
        Transport::WebSocket => {
            info!("🔌 Connecting via WebSocket to {}", WEBSOCKET_SERVER_ADDR);
            let auth = Authentication::Manual {
                server_addr: WEBSOCKET_SERVER_ADDR,
                client_id: config.client_id,
                private_key: Key::default(),
                protocol_id: 0,
            };
            let client = commands
                .spawn((
                    Client::default(),
                    LocalAddr(client_addr),
                    PeerAddr(WEBSOCKET_SERVER_ADDR),
                    Link::new(None),
                    ReplicationReceiver::default(),
                    ReplicationSender::new(
                        SERVER_REPLICATION_INTERVAL,
                        SendUpdatesMode::SinceLastAck,
                        false,
                    ),
                    NetcodeClient::new(auth, NetcodeConfig::default())?,
                    WebSocketClientIo {
                        config: WebSocketClientConfig::builder()
                            .with_no_cert_validation(),
                        scheme: WebSocketScheme::Secure,
                    },
                    Name::new("WebSocketClient"),
                ))
                .id();
            commands.trigger(Connect { entity: client });
        }
    }
    
    info!("🔗 Connection initiated...\n");
    Ok(())
}

/// Update cursor position based on mouse
fn cursor_movement(
    window_query: Query<&Window>,
    // Only update OUR cursor (the one with Replicate, not Interpolated)
    mut cursor_query: Query<&mut CursorPosition, (With<Replicate>, Without<Interpolated>)>,
) {
    let Ok(window) = window_query.single() else {
        return;
    };
    
    let Some(mouse_pos) = window.cursor_position() else {
        return;
    };
    
    // Convert to centered coordinates
    let centered = Vec2::new(
        mouse_pos.x - (window.width() / 2.0),
        -(mouse_pos.y - (window.height() / 2.0)),
    );
    
    for mut cursor in cursor_query.iter_mut() {
        cursor.set_if_neq(CursorPosition(centered));
    }
}

/// Handle when other clients' cursors spawn (interpolated)
fn handle_interpolated_spawn(
    mut interpolated: Query<(&PlayerId, &mut PlayerColor), Added<Interpolated>>,
) {
    for (player_id, mut color) in interpolated.iter_mut() {
        // Lower saturation for other clients' cursors
        let hsva = Hsva {
            saturation: 0.3,
            ..Hsva::from(color.0)
        };
        color.0 = Color::from(hsva);
        info!("🔮 Received interpolated cursor from client {:?}", player_id.0);
    }
}
