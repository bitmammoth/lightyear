//! Protocol definitions for client replication example.

use bevy::math::Curve;
use bevy::prelude::*;
use lightyear::prelude::*;
use serde::{Deserialize, Serialize};

// ============ Components ============

/// Unique identifier for each player - wraps PeerId
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlayerId(pub PeerId);

/// Cursor position - client authoritative, replicated to server and other clients
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Deref, DerefMut)]
pub struct CursorPosition(pub Vec2);

impl Ease for CursorPosition {
    fn interpolating_curve_unbounded(start: Self, end: Self) -> impl Curve<Self> {
        bevy::math::curve::FunctionCurve::new(bevy::math::curve::Interval::UNIT, move |t| {
            CursorPosition(Vec2::lerp(start.0, end.0, t))
        })
    }
}

/// Color for rendering
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlayerColor(pub Color);

// ============ Protocol Plugin ============

pub struct ProtocolPlugin;

impl Plugin for ProtocolPlugin {
    fn build(&self, app: &mut App) {
        // Register components
        app.register_component::<PlayerId>();
        app.register_component::<PlayerColor>();
        
        // CursorPosition with interpolation (no prediction - cursor is authoritative locally)
        app.register_component::<CursorPosition>()
            .add_linear_interpolation();
    }
}
