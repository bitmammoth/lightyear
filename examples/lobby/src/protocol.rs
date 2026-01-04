//! Protocol definitions for the lobby example

use bevy::app::{App, Plugin};
use bevy::ecs::entity::MapEntities;
use bevy::math::Curve;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use lightyear::prelude::*;

// Constants
pub const MOVE_SPEED: f32 = 10.0;
pub const SEND_INTERVAL: Duration = Duration::from_millis(40);

use std::time::Duration;

// Player bundle
#[derive(Bundle)]
pub struct PlayerBundle {
    pub id: PlayerId,
    pub position: PlayerPosition,
    pub color: PlayerColor,
}

impl PlayerBundle {
    pub fn new(id: PeerId, position: Vec2) -> Self {
        let h = (((id.to_bits().wrapping_mul(30)) % 360) as f32) / 360.0;
        let s = 0.8;
        let l = 0.5;
        let color = Color::hsl(h, s, l);
        Self {
            id: PlayerId(id),
            position: PlayerPosition(position),
            color: PlayerColor(color),
        }
    }
}

// Lobbies component - tracks all lobbies
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Default, Reflect)]
pub struct Lobbies {
    pub lobbies: Vec<Lobby>,
}

impl Lobbies {
    pub fn has_empty_lobby(&self) -> bool {
        if self.lobbies.is_empty() {
            return false;
        }
        self.lobbies.iter().any(|lobby| lobby.players.is_empty())
    }

    pub fn remove_client(&mut self, client_id: PeerId, commands: &mut Commands) {
        let mut removed_lobby = None;
        for (lobby_id, lobby) in self.lobbies.iter_mut().enumerate() {
            if let Some(index) = lobby.players.iter().position(|id| *id == client_id) {
                lobby.players.remove(index);
                if lobby.players.is_empty() {
                    removed_lobby = Some(lobby_id);
                    commands.entity(lobby.room).despawn();
                }
            }
        }
        if let Some(lobby_id) = removed_lobby {
            self.lobbies.remove(lobby_id);
            if !self.has_empty_lobby() {
                let room = commands.spawn(Room::default()).id();
                self.lobbies.push(Lobby::new(room));
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Reflect)]
pub struct Lobby {
    pub players: Vec<PeerId>,
    pub host: Option<PeerId>,
    pub room: Entity,
    pub in_game: bool,
}

impl Lobby {
    pub fn new(room: Entity) -> Self {
        Self {
            players: vec![],
            host: None,
            room,
            in_game: false,
        }
    }
}

// Components
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Reflect)]
pub struct PlayerId(pub PeerId);

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Deref, DerefMut, Reflect)]
pub struct PlayerPosition(pub Vec2);

impl Ease for PlayerPosition {
    fn interpolating_curve_unbounded(start: Self, end: Self) -> impl Curve<Self> {
        FunctionCurve::new(Interval::UNIT, move |t| {
            PlayerPosition(Vec2::lerp(start.0, end.0, t))
        })
    }
}

#[derive(Component, Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct PlayerColor(pub Color);

// Messages
pub struct Channel1;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct StartGame {
    pub lobby_id: usize,
    pub host: Option<PeerId>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ExitLobby {
    pub lobby_id: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct JoinLobby {
    pub lobby_id: usize,
}

// Inputs
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Reflect)]
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Reflect)]
pub enum Inputs {
    Direction(Direction),
}

impl Default for Inputs {
    fn default() -> Self {
        Inputs::Direction(Direction {
            up: false,
            down: false,
            left: false,
            right: false,
        })
    }
}

impl MapEntities for Inputs {
    fn map_entities<M: EntityMapper>(&mut self, _entity_mapper: &mut M) {}
}

// Protocol plugin
pub struct ProtocolPlugin;

impl Plugin for ProtocolPlugin {
    fn build(&self, app: &mut App) {
        // Messages
        app.register_message::<StartGame>()
            .add_direction(NetworkDirection::Bidirectional);
        app.register_message::<JoinLobby>()
            .add_direction(NetworkDirection::ClientToServer);
        app.register_message::<ExitLobby>()
            .add_direction(NetworkDirection::ClientToServer);
        
        // Inputs
        app.add_plugins(lightyear::prelude::input::native::InputPlugin::<Inputs>::default());
        
        // Components
        app.register_component::<Name>();
        app.register_component::<PlayerId>();
        app.register_component::<PlayerPosition>()
            .add_prediction()
            .add_linear_interpolation();
        app.register_component::<PlayerColor>();
        app.register_component::<Lobbies>();
        
        // Channels
        app.add_channel::<Channel1>(ChannelSettings {
            mode: ChannelMode::OrderedReliable(ReliableSettings::default()),
            ..default()
        })
        .add_direction(NetworkDirection::Bidirectional);
    }
}
