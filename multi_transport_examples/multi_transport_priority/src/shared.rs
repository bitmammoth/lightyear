//! Shared code between client and server.

use bevy::prelude::*;
use core::net::{IpAddr, Ipv4Addr, SocketAddr};
use core::time::Duration;

use crate::protocol::*;

// ============ Constants ============

pub const FIXED_TIMESTEP_HZ: f64 = 64.0;
pub const SERVER_REPLICATION_INTERVAL: Duration = Duration::from_millis(100);
pub const MOVE_SPEED: f32 = 10.0;

pub const UDP_PORT: u16 = 5000;
pub const WEBTRANSPORT_PORT: u16 = 5001;
pub const WEBSOCKET_PORT: u16 = 5002;

pub const UDP_SERVER_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), UDP_PORT);
pub const WEBTRANSPORT_SERVER_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), WEBTRANSPORT_PORT);
pub const WEBSOCKET_SERVER_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), WEBSOCKET_PORT);

// Grid settings
pub const GRID_SIZE: f32 = 20.0;
pub const NUM_SHAPES: i32 = 6;
// Bandwidth limit per client (bytes/second)
pub const BANDWIDTH_LIMIT: u32 = 3000;

// ============ Plugin ============

pub struct SharedPlugin;

impl Plugin for SharedPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ProtocolPlugin);
    }
}

// ============ Helpers ============

/// Shared player movement behavior
pub fn shared_movement_behaviour(mut position: Mut<PlayerPosition>, input: &Inputs) {
    if let Inputs::Direction(direction) = input {
        if direction.up {
            position.0.y += MOVE_SPEED;
        }
        if direction.down {
            position.0.y -= MOVE_SPEED;
        }
        if direction.left {
            position.0.x -= MOVE_SPEED;
        }
        if direction.right {
            position.0.x += MOVE_SPEED;
        }
    }
}
