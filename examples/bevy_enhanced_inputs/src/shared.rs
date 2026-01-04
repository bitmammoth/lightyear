//! Shared code between client and server
//!
//! Movement behavior must be identical on client and server for prediction to work.

use crate::protocol::*;
use bevy::prelude::*;
use core::time::Duration;

// Network settings
pub const PROTOCOL_ID: u64 = 0;
pub const PRIVATE_KEY: [u8; 32] = [0u8; 32];

pub const FIXED_TIMESTEP_HZ: f64 = 64.0;
pub const SERVER_REPLICATION_INTERVAL: Duration = Duration::from_millis(100);

// Server ports
pub const SERVER_UDP_PORT: u16 = 5000;
pub const SERVER_WEBTRANSPORT_PORT: u16 = 5001;
pub const SERVER_WEBSOCKET_PORT: u16 = 5002;

pub struct SharedPlugin;

impl Plugin for SharedPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ProtocolPlugin);
    }
}

/// Movement behavior - must be identical on client and server
pub fn shared_movement_behaviour(mut position: Mut<PlayerPosition>, input: Vec2) {
    const MOVE_SPEED: f32 = 10.0;
    position.0.y += input.y * MOVE_SPEED;
    position.0.x += input.x * MOVE_SPEED;
}
