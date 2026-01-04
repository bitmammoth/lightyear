//! Protocol definitions for the FPS example

use avian2d::prelude::*;
use bevy::prelude::*;
use leafwing_input_manager::prelude::*;
use lightyear::prelude::input::leafwing;
use lightyear::prelude::*;
use serde::{Deserialize, Serialize};

pub const BULLET_SIZE: f32 = 3.0;
pub const PLAYER_SIZE: f32 = 40.0;

// ============ Components ============

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Reflect)]
pub struct PredictedBot;

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Reflect)]
pub struct InterpolatedBot;

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Reflect)]
pub struct PlayerId(pub PeerId);

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Reflect)]
pub struct PlayerMarker;

/// Number of bullet hits
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Reflect)]
pub struct Score(pub usize);

#[derive(Component, Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Reflect)]
pub struct ColorComponent(pub Color);

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BulletMarker;

// ============ Inputs ============

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Copy, Hash, Reflect)]
pub enum PlayerActions {
    Up,
    Down,
    Left,
    Right,
    Shoot,
    MoveCursor,
}

impl Actionlike for PlayerActions {
    fn input_control_kind(&self) -> InputControlKind {
        match self {
            Self::MoveCursor => InputControlKind::DualAxis,
            _ => InputControlKind::Button,
        }
    }
}

// ============ Protocol Plugin ============

pub struct ProtocolPlugin;

impl Plugin for ProtocolPlugin {
    fn build(&self, app: &mut App) {
        // Leafwing input plugin with lag compensation
        app.add_plugins(leafwing::InputPlugin::<PlayerActions> {
            config: lightyear::input::prelude::InputConfig::<PlayerActions> {
                lag_compensation: true,
                ..default()
            },
        });
        
        // Components
        app.register_component::<Name>();
        app.register_component::<PlayerId>();
        app.register_component::<PlayerMarker>();

        app.register_component::<Position>()
            .add_prediction()
            .add_linear_interpolation()
            .enable_correction();

        app.register_component::<Rotation>()
            .add_prediction()
            .add_linear_interpolation()
            .enable_correction();

        app.register_component::<ColorComponent>();
        app.register_component::<Score>();
        app.register_component::<RigidBody>();
        app.register_component::<BulletMarker>();
        app.register_component::<PredictedBot>();
        app.register_component::<InterpolatedBot>();
    }
}
