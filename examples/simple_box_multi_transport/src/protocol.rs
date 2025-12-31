//! Protocol - shared component and message definitions.

use bevy::ecs::entity::MapEntities;
use bevy::math::Curve;
use bevy::prelude::*;
use lightyear::prelude::*;
use serde::{Deserialize, Serialize};

// ============ Components ============

/// Marker component for player entities
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Player;

/// Player ID component
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlayerId(pub PeerId);

/// The player's position in the world
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Reflect, Deref, DerefMut)]
pub struct PlayerPosition(pub Vec2);

impl Ease for PlayerPosition {
    fn interpolating_curve_unbounded(start: Self, end: Self) -> impl Curve<Self> {
        FunctionCurve::new(Interval::UNIT, move |t| {
            PlayerPosition(Vec2::lerp(start.0, end.0, t))
        })
    }
}

/// The player's color
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlayerColor(pub Color);

// ============ Inputs ============

/// Player movement direction
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Reflect)]
pub struct Direction {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
}

/// Input actions
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Reflect)]
pub enum Inputs {
    Direction(Direction),
}

impl Default for Inputs {
    fn default() -> Self {
        Self::Direction(Direction::default())
    }
}

impl MapEntities for Inputs {
    fn map_entities<M: EntityMapper>(&mut self, _entity_mapper: &mut M) {}
}

// ============ Bundles ============

/// Bundle for spawning a player
#[derive(Bundle)]
pub struct PlayerBundle {
    pub player: Player,
    pub id: PlayerId,
    pub position: PlayerPosition,
    pub color: PlayerColor,
}

impl PlayerBundle {
    pub fn new(id: PeerId, position: Vec2) -> Self {
        // Generate pseudo random color from client id
        let h = (((id.to_bits().wrapping_mul(30)) % 360) as f32) / 360.0;
        let s = 0.8;
        let l = 0.5;
        let color = Color::hsl(h, s, l);
        Self {
            player: Player,
            id: PlayerId(id),
            position: PlayerPosition(position),
            color: PlayerColor(color),
        }
    }
}

// ============ Protocol Plugin ============

/// Registers the protocol components, inputs, and channels.
#[derive(Clone)]
pub struct ProtocolPlugin;

impl Plugin for ProtocolPlugin {
    fn build(&self, app: &mut App) {
        // Register inputs
        app.add_plugins(input::native::InputPlugin::<Inputs>::default());
        
        // Register components
        app.register_component::<Player>();
        app.register_component::<PlayerId>();
        
        app.register_component::<PlayerPosition>()
            .add_prediction()
            .add_linear_interpolation();
        
        app.register_component::<PlayerColor>();
    }
}
