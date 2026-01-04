//! Shared code between client and server.

use bevy::prelude::*;
use core::net::{IpAddr, Ipv4Addr, SocketAddr};
use core::time::Duration;
use lightyear::prelude::*;

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

/// Grid spacing for spawning circles
pub const GRID_SIZE: f32 = 200.0;
/// Number of circles in each direction from origin
pub const NUM_CIRCLES: i32 = 2;
/// Radius within which circles become visible to a player
pub const INTEREST_RADIUS: f32 = 150.0;

// ============ Plugin ============

pub struct SharedPlugin;

impl Plugin for SharedPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ProtocolPlugin);
    }
}

// ============ Shared Movement ============

/// Generate pseudo-random color from PeerId
pub fn color_from_id(client_id: PeerId) -> Color {
    let h = (((client_id.to_bits().wrapping_mul(30)) % 360) as f32) / 360.0;
    let s = 1.0;
    let l = 0.5;
    Color::hsl(h, s, l)
}

/// Shared player movement behavior - used by both client (prediction) and server (authority)
pub fn shared_movement_behaviour(mut position: Mut<Position>, input: &Inputs) {
    if input.up {
        position.y += MOVE_SPEED;
    }
    if input.down {
        position.y -= MOVE_SPEED;
    }
    if input.left {
        position.x -= MOVE_SPEED;
    }
    if input.right {
        position.x += MOVE_SPEED;
    }
}
