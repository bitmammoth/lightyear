//! Protocol definitions for delta compression example.
//!
//! The key feature is the `Diffable` trait implementation on `PlayerPosition`,
//! which enables delta compression - sending only the change rather than the full value.

use bevy::ecs::entity::MapEntities;
use bevy::math::Curve;
use bevy::prelude::*;
use lightyear::prelude::*;
use serde::{Deserialize, Serialize};
use tracing::trace;

// ============ Components ============

/// Player identifier
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Reflect)]
pub struct PlayerId(pub u64);

/// Player position - with delta compression support
/// Instead of sending full Vec2 (8 bytes), sends delta as (i8, i8) (2 bytes)
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Deref, DerefMut, Reflect)]
pub struct PlayerPosition(pub Vec2);

impl Ease for PlayerPosition {
    fn interpolating_curve_unbounded(start: Self, end: Self) -> impl Curve<Self> {
        bevy::math::curve::FunctionCurve::new(bevy::math::curve::Interval::UNIT, move |t| {
            PlayerPosition(Vec2::lerp(start.0, end.0, t))
        })
    }
}

/// Maximum position change per tick that can be encoded
const MAX_POSITION_DELTA: f32 = 200.0;

/// Delta compression implementation for PlayerPosition
/// Compresses Vec2 changes to (i8, i8) - 75% bandwidth reduction
impl Diffable<(i8, i8)> for PlayerPosition {
    fn base_value() -> Self {
        Self(Vec2::new(0.0, 0.0))
    }

    fn diff(&self, new: &Self) -> (i8, i8) {
        let mut diff = new.0 - self.0;

        // Clamp the diff to a discrete set of values
        // i.e i8::MIN = -MAX_POSITION_DELTA, i8::MAX = MAX_POSITION_DELTA
        diff.x = diff.x.clamp(-MAX_POSITION_DELTA, MAX_POSITION_DELTA);
        diff.y = diff.y.clamp(-MAX_POSITION_DELTA, MAX_POSITION_DELTA);
        diff.x = diff.x / MAX_POSITION_DELTA * (i8::MAX as f32);
        diff.y = diff.y / MAX_POSITION_DELTA * (i8::MAX as f32);
        
        trace!(
            "Computing diff between {:?} and {:?}: {:?}",
            self,
            new,
            diff
        );

        // Convert to i8
        (diff.x as i8, diff.y as i8)
    }

    fn apply_diff(&mut self, delta: &(i8, i8)) {
        trace!("Applying diff {:?} to {:?}", delta, self);
        let mut diff = Vec2::new(delta.0 as f32, delta.1 as f32);
        diff.x = diff.x / (i8::MAX as f32) * MAX_POSITION_DELTA;
        diff.y = diff.y / (i8::MAX as f32) * MAX_POSITION_DELTA;
        self.0 += diff;
    }
}

/// Player color
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Reflect)]
pub struct PlayerColor(pub Color);

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
        
        // PlayerPosition with prediction, interpolation, AND delta compression
        // NOTE: delta compression must be added AFTER prediction/interpolation
        app.register_component::<PlayerPosition>()
            .add_prediction()
            .add_linear_interpolation()
            .add_delta_compression::<(i8, i8)>();
    }
}
