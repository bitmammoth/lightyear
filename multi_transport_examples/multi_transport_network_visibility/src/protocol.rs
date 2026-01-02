//! Protocol definitions for the network visibility example.

use bevy::ecs::entity::MapEntities;
use bevy::math::Curve;
use bevy::prelude::*;
use lightyear::prelude::*;
use serde::{Deserialize, Serialize};

// ============ Components ============

/// Unique identifier for each player - wraps PeerId
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlayerId(pub PeerId);

/// Position in 2D space
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Deref, DerefMut)]
pub struct Position(pub Vec2);

impl Ease for Position {
    fn interpolating_curve_unbounded(start: Self, end: Self) -> impl Curve<Self> {
        bevy::math::curve::FunctionCurve::new(bevy::math::curve::Interval::UNIT, move |t| {
            Position(Vec2::lerp(start.0, end.0, t))
        })
    }
}

/// Color for rendering
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlayerColor(pub Color);

/// Marker component for circles (used for visibility demo)
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CircleMarker;

// ============ Inputs ============

/// Input directions
#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Eq, Clone, Reflect)]
pub struct Inputs {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
}

impl Inputs {
    pub fn is_none(&self) -> bool {
        !self.up && !self.down && !self.left && !self.right
    }
}

impl MapEntities for Inputs {
    fn map_entities<M: bevy::ecs::entity::EntityMapper>(&mut self, _entity_mapper: &mut M) {}
}

// ============ Protocol Plugin ============

pub struct ProtocolPlugin;

impl Plugin for ProtocolPlugin {
    fn build(&self, app: &mut App) {
        // Register inputs
        app.add_plugins(lightyear::prelude::input::native::InputPlugin::<Inputs>::default());
        
        // Register components
        app.register_component::<PlayerId>();
        app.register_component::<PlayerColor>();
        app.register_component::<CircleMarker>();
        
        // Position with prediction and interpolation
        app.register_component::<Position>()
            .add_prediction()
            .add_linear_interpolation();
    }
}
