//! Client module - connects via auth backend to get ConnectToken.
//!
//! Flow:
//! 1. Connect to TCP auth backend
//! 2. Receive ConnectToken
//! 3. Use token to connect to game server

use async_compat::Compat;
use bevy::prelude::*;
use bevy::tasks::futures_lite::future;
use bevy::tasks::{block_on, IoTaskPool, Task};
use core::net::Ipv4Addr;
use std::net::SocketAddr;
use lightyear::netcode::{ConnectToken, CONNECT_TOKEN_BYTES};
use lightyear::prelude::client::*;
use lightyear::prelude::*;
use lightyear::websocket::prelude::client::ClientConfig as WebSocketClientConfig;

use crate::shared::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Transport {
    #[default]
    Udp,
    WebTransport,
    WebSocket,
}

#[derive(Resource)]
pub struct ClientConfig {
    pub transport: Transport,
    pub cert_digest: Option<String>,
}

pub struct ExampleClientPlugin;

impl Plugin for ExampleClientPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ConnectTokenTask { task: None });
        app.add_systems(Startup, (setup_camera, start_token_request));
        app.add_systems(Update, poll_token_task);
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

/// Resource to hold the async task fetching the ConnectToken
#[derive(Resource)]
struct ConnectTokenTask {
    task: Option<Task<ConnectToken>>,
}

/// Start the async task to get a ConnectToken from the auth backend
fn start_token_request(mut token_task: ResMut<ConnectTokenTask>) {
    info!("🔐 Requesting ConnectToken from auth backend at {}", AUTH_BACKEND_ADDR);
    
    let task = IoTaskPool::get().spawn(Compat::new(async move {
        get_connect_token_from_backend().await
    }));
    token_task.task = Some(task);
}

/// Poll the token task and connect when token is received
fn poll_token_task(
    mut token_task: ResMut<ConnectTokenTask>,
    config: Res<ClientConfig>,
    mut commands: Commands,
    existing_clients: Query<Entity, With<Client>>,
) -> Result {
    // Don't create multiple clients
    if !existing_clients.is_empty() {
        return Ok(());
    }
    
    let Some(task) = &mut token_task.task else {
        return Ok(());
    };
    
    let Some(connect_token) = block_on(future::poll_once(task)) else {
        return Ok(());
    };
    
    info!("✅ Received ConnectToken, connecting to server...");
    token_task.task = None;
    
    // Now create the client with the token
    let client_port = 6000 + (rand::random::<u16>() % 100);
    let client_addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), client_port);
    
    match config.transport {
        Transport::Udp => {
            info!("📡 Connecting via UDP to {}", UDP_SERVER_ADDR);
            let client = commands
                .spawn((
                    Client::default(),
                    LocalAddr(client_addr),
                    PeerAddr(UDP_SERVER_ADDR),
                    Link::new(None),
                    ReplicationReceiver::default(),
                    NetcodeClient::new(
                        Authentication::Token(connect_token),
                        NetcodeConfig::default(),
                    )?,
                    UdpIo::default(),
                    Name::new("UdpClient"),
                ))
                .id();
            commands.trigger(Connect { entity: client });
        }
        Transport::WebTransport => {
            info!("🌐 Connecting via WebTransport to {}", WEBTRANSPORT_SERVER_ADDR);
            let cert_digest = config.cert_digest.clone()
                .expect("WebTransport requires cert_digest")
                .replace(":", "");
            let client = commands
                .spawn((
                    Client::default(),
                    LocalAddr(client_addr),
                    PeerAddr(WEBTRANSPORT_SERVER_ADDR),
                    Link::new(None),
                    ReplicationReceiver::default(),
                    NetcodeClient::new(
                        Authentication::Token(connect_token),
                        NetcodeConfig::default(),
                    )?,
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
            let client = commands
                .spawn((
                    Client::default(),
                    LocalAddr(client_addr),
                    PeerAddr(WEBSOCKET_SERVER_ADDR),
                    Link::new(None),
                    ReplicationReceiver::default(),
                    NetcodeClient::new(
                        Authentication::Token(connect_token),
                        NetcodeConfig::default(),
                    )?,
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
    
    Ok(())
}

/// Async function to get ConnectToken from TCP auth backend
async fn get_connect_token_from_backend() -> ConnectToken {
    let stream = tokio::net::TcpStream::connect(AUTH_BACKEND_ADDR)
        .await
        .expect(&format!("Failed to connect to auth backend at {}", AUTH_BACKEND_ADDR));
    
    stream.readable().await.unwrap();
    
    let mut buffer = [0u8; CONNECT_TOKEN_BYTES];
    match stream.try_read(&mut buffer) {
        Ok(n) if n == CONNECT_TOKEN_BYTES => {
            info!("📥 Received {} bytes from auth backend", n);
            ConnectToken::try_from_bytes(&buffer)
                .expect("Failed to parse ConnectToken")
        }
        Ok(n) => {
            panic!("Unexpected token size: {} bytes (expected {})", n, CONNECT_TOKEN_BYTES);
        }
        Err(e) => {
            panic!("Failed to read token: {:?}", e);
        }
    }
}
