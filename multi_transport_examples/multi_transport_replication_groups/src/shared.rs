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
pub const TRAIL_OFFSET: f32 = 50.0;  // How far behind the trail follows

pub const UDP_PORT: u16 = 5000;
pub const WEBTRANSPORT_PORT: u16 = 5001;
pub const WEBSOCKET_PORT: u16 = 5002;

pub const UDP_SERVER_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), UDP_PORT);
pub const WEBTRANSPORT_SERVER_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), WEBTRANSPORT_PORT);
pub const WEBSOCKET_SERVER_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), WEBSOCKET_PORT);

// ============ Plugin ============

pub struct SharedPlugin;

impl Plugin for SharedPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ProtocolPlugin);
        // Trail follows head in FixedUpdate
        app.add_systems(FixedUpdate, update_trail_position);
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

/// Update trail positions to follow their parent player
/// This runs on both client (for predicted entities) and server
pub fn update_trail_position(
    player_query: Query<Ref<PlayerPosition>, Or<(With<Predicted>, With<Replicate>)>>,
    mut trail_query: Query<(&mut TrailPosition, &PlayerParent), Or<(With<Predicted>, With<ReplicateLike>)>>,
) {
    for (mut trail_pos, parent) in trail_query.iter_mut() {
        let Ok(player_pos) = player_query.get(parent.0) else {
            continue;
        };
        
        // Only update if player position changed (avoids spurious change detection)
        if !player_pos.is_changed() {
            continue;
        }
        
        // Trail follows behind the player
        trail_pos.0 = Vec2::new(player_pos.0.x - TRAIL_OFFSET, player_pos.0.y - TRAIL_OFFSET);
    }
}
