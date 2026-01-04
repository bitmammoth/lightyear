// src/config.rs
//! Configuration types for the multi-transport launcher.

use bevy::prelude::{Reflect, Resource};
use core::net::{IpAddr, Ipv4Addr, SocketAddr};
use core::time::Duration;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

// --- Constants ---
pub const FIXED_TIMESTEP_HZ: f64 = 64.0;
pub const SERVER_UDP_PORT: u16 = 5000;
pub const SERVER_WEBTRANSPORT_PORT: u16 = 5001;
pub const SERVER_WEBSOCKET_PORT: u16 = 5002;
pub const CLIENT_PORT: u16 = 0;
pub const SERVER_IP: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);

pub const PROTOCOL_ID: u64 = 0;
pub const PRIVATE_KEY: [u8; 32] = [0u8; 32];

// --- Configuration Enums ---

/// Available examples in the launcher
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, Display, Serialize, Deserialize, Reflect, Default,
)]
pub enum Example {
    #[default]
    SimpleBox,
    // TODO: Add more examples as they're created
    // Fps,
    // Lobby,
}

/// Networking mode for the instance
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    EnumIter,
    Display,
    EnumString,
    Serialize,
    Deserialize,
    Reflect,
    Default,
)]
pub enum NetworkingMode {
    ClientOnly,
    #[default]
    ServerOnly,
    HostServer,
}

/// Client transport selection
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, Display, Serialize, Deserialize, Reflect, Default,
)]
pub enum ClientTransport {
    #[default]
    Udp,
    WebTransport,
    WebSocket,
}

impl ClientTransport {
    pub fn default_port(&self) -> u16 {
        match self {
            ClientTransport::Udp => SERVER_UDP_PORT,
            ClientTransport::WebTransport => SERVER_WEBTRANSPORT_PORT,
            ClientTransport::WebSocket => SERVER_WEBSOCKET_PORT,
        }
    }
}

// --- Main Configuration Struct ---

#[derive(Resource, Debug, Clone, Serialize, Deserialize)]
pub struct LauncherConfig {
    pub example: Example,
    pub mode: NetworkingMode,
    pub client_transport: ClientTransport,
    pub client_id: u64,
    pub server_ip: String,
    #[serde(with = "duration_serde")]
    pub tick_duration: Duration,
    // Server settings - multi-transport supports all three
    pub enable_udp: bool,
    pub enable_webtransport: bool,
    pub enable_websocket: bool,
    pub udp_port: u16,
    pub webtransport_port: u16,
    pub websocket_port: u16,
}

impl Default for LauncherConfig {
    fn default() -> Self {
        Self {
            example: Example::default(),
            mode: NetworkingMode::default(),
            client_transport: ClientTransport::default(),
            client_id: rand::random::<u64>() % 1000,
            server_ip: "127.0.0.1".to_string(),
            tick_duration: Duration::from_secs_f64(1.0 / FIXED_TIMESTEP_HZ),
            // Server defaults - enable all transports
            enable_udp: true,
            enable_webtransport: true,
            enable_websocket: true,
            udp_port: SERVER_UDP_PORT,
            webtransport_port: SERVER_WEBTRANSPORT_PORT,
            websocket_port: SERVER_WEBSOCKET_PORT,
        }
    }
}

impl LauncherConfig {
    /// Get the server address for the selected client transport
    pub fn server_addr(&self) -> SocketAddr {
        let port = self.client_transport.default_port();
        let ip: IpAddr = self.server_ip.parse().unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
        SocketAddr::new(ip, port)
    }

    /// Check if configuration is valid for the selected mode
    pub fn is_valid(&self) -> bool {
        match self.mode {
            NetworkingMode::ClientOnly => {
                // Client needs a valid server IP
                self.server_ip.parse::<IpAddr>().is_ok()
            }
            NetworkingMode::ServerOnly => {
                // Server needs at least one transport enabled
                self.enable_udp || self.enable_webtransport || self.enable_websocket
            }
            NetworkingMode::HostServer => {
                // Both client and server requirements
                self.server_ip.parse::<IpAddr>().is_ok()
                    && (self.enable_udp || self.enable_webtransport || self.enable_websocket)
            }
        }
    }
}

// Helper module for Duration serialization/deserialization
mod duration_serde {
    use core::time::Duration;
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_f64(duration.as_secs_f64())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = f64::deserialize(deserializer)?;
        Ok(Duration::from_secs_f64(secs))
    }
}
