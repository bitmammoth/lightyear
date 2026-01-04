//! Shared code between client and server.
//!
//! Contains movement behaviors and helper functions that need to be identical
//! on both sides for prediction to work correctly.

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

// Distance threshold for authority transfer - when player is closer than this to ball
pub const AUTHORITY_TRANSFER_DISTANCE: f32 = 100.0;

// ============ Plugin ============

pub struct SharedPlugin;

impl Plugin for SharedPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ProtocolPlugin);
        // Ball movement runs in FixedUpdate with authority check
        app.add_systems(FixedUpdate, ball_movement);
    }
}

// ============ Shared Movement ============

/// Generate pseudo-random color from PeerId
pub fn color_from_id(client_id: PeerId) -> Color {
    let h = (((client_id.to_bits().wrapping_mul(80)) % 360) as f32) / 360.0;
    let s = 1.0;
    let l = 0.5;
    Color::hsl(h, s, l)
}

/// Shared player movement behavior - used by both client (prediction) and server (authority)
pub fn shared_movement_behaviour(mut position: Mut<Position>, input: &Inputs) {
    if let Inputs::Direction(direction) = input {
        if direction.up {
            position.y += MOVE_SPEED;
        }
        if direction.down {
            position.y -= MOVE_SPEED;
        }
        if direction.left {
            position.x -= MOVE_SPEED;
        }
        if direction.right {
            position.x += MOVE_SPEED;
        }
    }
}

/// Ball movement - only runs when we have authority over the ball.
///
/// The peer with authority (Server or Client) runs this system.
/// Other peers see interpolated updates.
pub fn ball_movement(
    mut balls: Query<(&mut Position, &mut Speed), (With<BallMarker>, With<HasAuthority>)>,
) {
    for (mut position, mut speed) in balls.iter_mut() {
        // Bounce at vertical bounds
        if position.y > 300.0 {
            speed.y = -1.0;
        }
        if position.y < -300.0 {
            speed.y = 1.0;
        }
        // Apply velocity
        position.0 += speed.0;
    }
}
