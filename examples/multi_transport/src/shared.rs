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

// ============ Channels ============

/// Default channel for replication
pub struct DefaultChannel;

// ============ Messages ============

/// Message sent from server to specific client
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ServerToClientMessage {
    pub content: String,
}

/// Message sent from server to ALL clients
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BroadcastMessage {
    pub content: String,
}

/// Message sent from client to server
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ClientToServerMessage {
    pub content: String,
}

/// Message sent from client to server, to be forwarded to another client
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ForwardMessage {
    pub target_player_id: u64,
    pub content: String,
}

// ============ Plugin ============

/// Plugin to register the shared protocol.
/// MUST be added AFTER ClientPlugins/ServerPlugins.
pub struct SharedPlugin;

impl Plugin for SharedPlugin {
    fn build(&self, app: &mut App) {
        // Register channel
        app.add_channel::<DefaultChannel>(ChannelSettings {
            mode: ChannelMode::OrderedReliable(ReliableSettings::default()),
            ..default()
        })
        .add_direction(NetworkDirection::Bidirectional);

        // Register replicated components
        app.register_component::<Player>();
        app.register_component::<PlayerId>();
        app.register_component::<PlayerPosition>();
        app.register_component::<PlayerColor>();

        // Register messages
        app.register_message::<ServerToClientMessage>()
            .add_direction(NetworkDirection::ServerToClient);
        app.register_message::<BroadcastMessage>()
            .add_direction(NetworkDirection::ServerToClient);
        app.register_message::<ClientToServerMessage>()
            .add_direction(NetworkDirection::ClientToServer);
        app.register_message::<ForwardMessage>()
            .add_direction(NetworkDirection::ClientToServer);
    }
}
