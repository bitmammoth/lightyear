//! Protocol definitions for bevy_enhanced_input integration
//!
//! Defines components, inputs (via BEI), and registers them with lightyear.

use bevy::math::Curve;
use bevy::prelude::*;
use lightyear::input::prelude::InputConfig;
use lightyear::prelude::input::bei::*;
use lightyear::prelude::*;
use serde::{Deserialize, Serialize};

// Components

/// Identifies which peer owns this player
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlayerId(pub PeerId);

/// Player position - supports interpolation via the Ease trait
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Deref, DerefMut)]
pub struct PlayerPosition(pub Vec2);

impl Ease for PlayerPosition {
    fn interpolating_curve_unbounded(start: Self, end: Self) -> impl Curve<Self> {
        FunctionCurve::new(Interval::UNIT, move |t| {
            PlayerPosition(Vec2::lerp(start.0, end.0, t))
        })
    }
}

/// Player color for visual distinction
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlayerColor(pub Color);

// Inputs using bevy_enhanced_input

/// The context component for player inputs - will be replicated
#[derive(Component, Serialize, Deserialize, Reflect, Clone, Debug, PartialEq)]
pub struct Player;

/// Movement action - outputs a Vec2 direction
#[derive(Debug, InputAction)]
#[action_output(Vec2)]
pub struct Movement;

// Protocol Plugin

#[derive(Clone)]
pub struct ProtocolPlugin;

impl Plugin for ProtocolPlugin {
    fn build(&self, app: &mut App) {
        // Register BEI inputs
        app.add_plugins(InputPlugin::<Player> {
            config: InputConfig::<Player> {
                rebroadcast_inputs: true,
                ..default()
            },
        });
        app.register_input_action::<Movement>();

        // Register components
        app.register_component::<PlayerId>();
        
        app.register_component::<PlayerPosition>()
            .add_prediction()
            .add_linear_interpolation();

        app.register_component::<PlayerColor>();
    }
}
