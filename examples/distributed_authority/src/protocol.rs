//! Protocol definitions for the distributed authority example.
//!
//! Components, inputs, and messages shared between client and server.

use bevy::ecs::entity::MapEntities;
use bevy::math::Curve;
use bevy::prelude::*;
use lightyear::prelude::*;
use serde::{Deserialize, Serialize};

// ============ Components ============

/// Unique identifier for each player - wraps PeerId to track owner
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Deref, DerefMut, Reflect)]
pub struct PlayerId(pub PeerId);

/// Position in 2D space - used for players and the ball
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Default, Deref, DerefMut, Reflect)]
pub struct Position(pub Vec2);

impl Ease for Position {
    fn interpolating_curve_unbounded(start: Self, end: Self) -> impl Curve<Self> {
        bevy::math::curve::FunctionCurve::new(bevy::math::curve::Interval::UNIT, move |t| {
            Position(Vec2::lerp(start.0, end.0, t))
        })
    }
}

/// Velocity for the ball
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Default, Deref, DerefMut, Reflect)]
pub struct Speed(pub Vec2);

/// Color for rendering
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlayerColor(pub Color);

/// Marker component for the ball entity
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct BallMarker;

// ============ Inputs ============

/// Direction input from players
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

/// Input actions that can be sent to the server
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Reflect, Default)]
pub enum Inputs {
    #[default]
    None,
    Direction(Direction),
}

impl MapEntities for Inputs {
    fn map_entities<M: bevy::ecs::entity::EntityMapper>(&mut self, _entity_mapper: &mut M) {}
}

// ============ Protocol Plugin ============

/// Registers all protocol components, inputs, and channels
pub struct ProtocolPlugin;

impl Plugin for ProtocolPlugin {
    fn build(&self, app: &mut App) {
        // Components - basic replication
        app.register_component::<PlayerId>();
        app.register_component::<PlayerColor>();
        app.register_component::<BallMarker>();
        
        // Position with prediction and linear interpolation
        app.register_component::<Position>()
            .add_prediction()
            .add_linear_interpolation();
        
        // Speed - replicated but not interpolated
        app.register_component::<Speed>();
        
        // Register inputs
        app.add_plugins(lightyear::prelude::input::native::InputPlugin::<Inputs>::default());
    }
}
