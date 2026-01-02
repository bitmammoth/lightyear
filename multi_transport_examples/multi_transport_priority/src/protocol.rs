//! Protocol definitions for priority example.

use bevy::math::Curve;
use bevy::prelude::*;
use lightyear::prelude::*;
use serde::{Deserialize, Serialize};

// ============ Components ============

/// Position of a shape in the grid
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Deref, DerefMut, Reflect)]
pub struct Position(pub Vec2);

impl Ease for Position {
    fn interpolating_curve_unbounded(start: Self, end: Self) -> impl Curve<Self> {
        bevy::math::curve::FunctionCurve::new(bevy::math::curve::Interval::UNIT, move |t| {
            Position(Vec2::lerp(start.0, end.0, t))
        })
    }
}

/// Shape type - cycles through Circle -> Triangle -> Square
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Reflect)]
pub enum Shape {
    Circle,
    Triangle,
    Square,
}

/// Timer to cycle shape type
#[derive(Component, Deref, DerefMut)]
pub struct ShapeChangeTimer(pub Timer);

/// Player identifier
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Reflect)]
pub struct PlayerId(pub u64);

/// Player color
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Reflect)]
pub struct PlayerColor(pub Color);

/// Player position (separate from grid Position)
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Deref, DerefMut, Reflect)]
pub struct PlayerPosition(pub Vec2);

impl Ease for PlayerPosition {
    fn interpolating_curve_unbounded(start: Self, end: Self) -> impl Curve<Self> {
        bevy::math::curve::FunctionCurve::new(bevy::math::curve::Interval::UNIT, move |t| {
            PlayerPosition(Vec2::lerp(start.0, end.0, t))
        })
    }
}

// ============ Inputs ============

/// Direction input
#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Eq, Clone, Reflect)]
pub struct Direction {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
}

impl Direction {
    #[allow(dead_code)]
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

impl bevy::ecs::entity::MapEntities for Inputs {
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
        app.register_component::<Shape>();
        
        // Position with interpolation (for grid shapes - no prediction needed)
        app.register_component::<Position>()
            .add_linear_interpolation();
        
        // PlayerPosition with prediction and interpolation
        app.register_component::<PlayerPosition>()
            .add_prediction()
            .add_linear_interpolation();
    }
}
