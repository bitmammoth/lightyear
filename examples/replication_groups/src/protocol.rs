//! Protocol definitions for replication groups example.

use bevy::ecs::entity::MapEntities;
use bevy::math::Curve;
use bevy::prelude::*;
use lightyear::prelude::*;
use serde::{Deserialize, Serialize};

// ============ Components ============

/// Unique identifier for each player
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Reflect)]
pub struct PlayerId(pub u64);

/// Player head position
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Deref, DerefMut, Reflect)]
pub struct PlayerPosition(pub Vec2);

impl Ease for PlayerPosition {
    fn interpolating_curve_unbounded(start: Self, end: Self) -> impl Curve<Self> {
        bevy::math::curve::FunctionCurve::new(bevy::math::curve::Interval::UNIT, move |t| {
            PlayerPosition(Vec2::lerp(start.0, end.0, t))
        })
    }
}

/// Player color
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Reflect)]
pub struct PlayerColor(pub Color);

/// Trail color - usually same as player but with different alpha
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Reflect)]
pub struct TrailColor(pub Color);

/// Reference to the parent player entity - demonstrates entity references in replication
/// The #[entities] attribute marks this for entity mapping during replication
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Reflect)]
pub struct PlayerParent(#[entities] pub Entity);

impl MapEntities for PlayerParent {
    fn map_entities<M: bevy::ecs::entity::EntityMapper>(&mut self, entity_mapper: &mut M) {
        self.0 = entity_mapper.get_mapped(self.0);
    }
}

/// Trail position - linked to a player head
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Deref, DerefMut, Reflect)]
pub struct TrailPosition(pub Vec2);

impl Ease for TrailPosition {
    fn interpolating_curve_unbounded(start: Self, end: Self) -> impl Curve<Self> {
        bevy::math::curve::FunctionCurve::new(bevy::math::curve::Interval::UNIT, move |t| {
            TrailPosition(Vec2::lerp(start.0, end.0, t))
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
        app.register_component::<TrailColor>();
        
        // PlayerPosition with prediction and interpolation
        app.register_component::<PlayerPosition>()
            .add_prediction()
            .add_linear_interpolation();
        
        // TrailPosition with prediction and interpolation
        app.register_component::<TrailPosition>()
            .add_prediction()
            .add_linear_interpolation();
        
        // PlayerParent with entity mapping (critical for replication groups!)
        app.register_component::<PlayerParent>()
            .add_map_entities();
    }
}
