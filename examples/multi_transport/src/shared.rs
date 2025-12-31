//! Shared protocol between server and clients.
//!
//! Defines components for replication demonstration.

use bevy::prelude::*;
use core::net::{IpAddr, Ipv4Addr, SocketAddr};
use core::time::Duration;
use lightyear::prelude::*;
use serde::{Deserialize, Serialize};

// ============ Constants ============

pub const FIXED_TIMESTEP_HZ: f64 = 64.0;
pub const SERVER_REPLICATION_INTERVAL: Duration = Duration::from_millis(100);

pub const UDP_PORT: u16 = 5000;
pub const WEBTRANSPORT_PORT: u16 = 5001;

pub const UDP_SERVER_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), UDP_PORT);
pub const WEBTRANSPORT_SERVER_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), WEBTRANSPORT_PORT);

// ============ Components ============

/// Unique ID for each player (server-assigned)
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PlayerId(pub u64);

/// Marker component for player entities
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Player {
    pub id: PlayerId,
}

/// The player's position in the world
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Deref, DerefMut, Default)]
pub struct PlayerPosition(pub Vec2);

/// The player's color (generated from their ID)
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlayerColor(pub Color);

// ============ Plugin ============

/// Plugin to register the shared protocol.
/// MUST be added AFTER ClientPlugins/ServerPlugins.
pub struct SharedPlugin;

impl Plugin for SharedPlugin {
    fn build(&self, app: &mut App) {
        // Register replicated components
        app.register_component::<Player>();
        app.register_component::<PlayerId>();
        app.register_component::<PlayerPosition>();
        app.register_component::<PlayerColor>();
    }
}
