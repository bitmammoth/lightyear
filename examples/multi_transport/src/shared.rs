//! Shared protocol between server and clients.
//!
//! Defines components for replication demonstration.

use bevy::ecs::entity::MapEntities;
use bevy::math::Curve;
use bevy::prelude::*;
use core::net::{IpAddr, Ipv4Addr, SocketAddr};
use core::time::Duration;
use lightyear::prelude::*;
use serde::{Deserialize, Serialize};

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

// ============ Inputs ============

#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Eq, Clone, Reflect)]
pub struct Direction {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
}

impl Direction {
    pub fn is_none(&self) -> bool {
        !self.up && !self.down && !self.left && !self.right
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Reflect, Default)]
pub enum Inputs {
    #[default]
    None,
    Direction(Direction),
}

impl MapEntities for Inputs {
    fn map_entities<M: bevy::ecs::entity::EntityMapper>(&mut self, _entity_mapper: &mut M) {}
}

// ============ Components ============

/// Unique ID for each player (server-assigned)
/// This is a flat component matching simple_box pattern - no nesting!
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect)]
pub struct PlayerId(pub u64);

/// The player's position in the world
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Deref, DerefMut, Default, Reflect)]
pub struct PlayerPosition(pub Vec2);

impl Ease for PlayerPosition {
    fn interpolating_curve_unbounded(start: Self, end: Self) -> impl Curve<Self> {
        bevy::math::curve::FunctionCurve::new(bevy::math::curve::Interval::UNIT, move |t| {
            PlayerPosition(Vec2::lerp(start.0, end.0, t))
        })
    }
}

/// The player's color (generated from their ID)
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlayerColor(pub Color);

// ============ Shared Movement ============

/// Shared movement behavior - same on client (prediction) and server (authority)
pub fn shared_movement_behaviour(mut position: Mut<PlayerPosition>, input: &Inputs) {
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
    pub from_name: String,
    pub target_name: String,
    pub content: String,
    pub is_reply: bool,  // True if this is a reply to a forwarded message
}

/// Message forwarded from another client (via server)
/// Contains sender info so recipient can reply
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ForwardedMessage {
    pub from_name: String,
    pub content: String,
    pub is_reply: bool,  // True if this is a reply (don't reply to replies)
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

        // Register inputs
        app.add_plugins(input::native::InputPlugin::<Inputs>::default());

        // Register replicated components
        app.register_component::<PlayerId>();
        
        // PlayerPosition with prediction and interpolation
        app.register_component::<PlayerPosition>()
            .add_prediction()
            .add_linear_interpolation();
            
        app.register_component::<PlayerColor>();

        // Register messages
        app.register_message::<ServerToClientMessage>()
            .add_direction(NetworkDirection::ServerToClient);
        app.register_message::<BroadcastMessage>()
            .add_direction(NetworkDirection::ServerToClient);
        app.register_message::<ForwardedMessage>()
            .add_direction(NetworkDirection::ServerToClient);
        app.register_message::<ClientToServerMessage>()
            .add_direction(NetworkDirection::ClientToServer);
        app.register_message::<ForwardMessage>()
            .add_direction(NetworkDirection::ClientToServer);
    }
}
