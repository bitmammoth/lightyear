//! Shared protocol between server and clients.
//!
//! This defines the messages, channels, and constants used for communication.

use bevy::prelude::*;
use core::net::{IpAddr, Ipv4Addr, SocketAddr};
use core::time::Duration;
use lightyear::prelude::*;
use serde::{Deserialize, Serialize};

/// Fixed timestep for networking (64 Hz)
pub const FIXED_TIMESTEP_HZ: f64 = 64.0;

/// Server replication interval
pub const SERVER_REPLICATION_INTERVAL: Duration = Duration::from_millis(100);

/// UDP server port
pub const UDP_PORT: u16 = 5000;

/// WebTransport server port  
pub const WEBTRANSPORT_PORT: u16 = 5001;

/// UDP server address
pub const UDP_SERVER_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), UDP_PORT);

/// WebTransport server address
pub const WEBTRANSPORT_SERVER_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), WEBTRANSPORT_PORT);

/// Channel for reliable ordered messages
pub struct ReliableChannel;

/// A ping message from client to server
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PingMessage {
    pub client_id: u64,
    pub sequence: u32,
}

/// A pong response from server to client
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PongMessage {
    pub sequence: u32,
}

/// Plugin to register the shared protocol.
/// MUST be added AFTER ClientPlugins/ServerPlugins.
pub struct SharedPlugin;

impl Plugin for SharedPlugin {
    fn build(&self, app: &mut App) {
        // Register messages
        app.register_message::<PingMessage>()
            .add_direction(NetworkDirection::ClientToServer);
        
        app.register_message::<PongMessage>()
            .add_direction(NetworkDirection::ServerToClient);

        // Register channel
        app.add_channel::<ReliableChannel>(ChannelSettings {
            mode: ChannelMode::OrderedReliable(ReliableSettings::default()),
            ..default()
        })
        .add_direction(NetworkDirection::Bidirectional);
    }
}
