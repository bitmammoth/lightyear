//! Shared constants for auth example.

use core::net::{IpAddr, Ipv4Addr, SocketAddr};
use core::time::Duration;
use lightyear::netcode::Key;

pub const FIXED_TIMESTEP_HZ: f64 = 64.0;
pub const SERVER_REPLICATION_INTERVAL: Duration = Duration::from_millis(100);

// Auth backend port (TCP)
pub const AUTH_BACKEND_PORT: u16 = 4000;
pub const AUTH_BACKEND_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), AUTH_BACKEND_PORT);

// Game server ports
pub const UDP_PORT: u16 = 5000;
pub const WEBTRANSPORT_PORT: u16 = 5001;
pub const WEBSOCKET_PORT: u16 = 5002;

pub const UDP_SERVER_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), UDP_PORT);
pub const WEBTRANSPORT_SERVER_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), WEBTRANSPORT_PORT);
pub const WEBSOCKET_SERVER_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), WEBSOCKET_PORT);

// Shared secret for token generation (in production, this would be securely shared)
pub const PROTOCOL_ID: u64 = 0;
pub const PRIVATE_KEY: Key = [0u8; 32];
