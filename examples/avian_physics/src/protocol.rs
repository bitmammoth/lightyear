//! Protocol definitions for avian2d physics integration

use avian2d::prelude::*;
use bevy::prelude::*;
use leafwing_input_manager::prelude::*;
use lightyear::input::config::InputConfig;
use lightyear::prelude::input::leafwing;
use lightyear::prelude::*;
use serde::{Deserialize, Serialize};

pub const BALL_SIZE: f32 = 15.0;
pub const PLAYER_SIZE: f32 = 40.0;

#[derive(Bundle)]
pub struct PhysicsBundle {
    pub collider: Collider,
    pub collider_density: ColliderDensity,
    pub rigid_body: RigidBody,
    pub restitution: Restitution,
}

impl PhysicsBundle {
    pub fn ball() -> Self {
        Self {
            collider: Collider::circle(BALL_SIZE),
            collider_density: ColliderDensity(0.05),
            rigid_body: RigidBody::Dynamic,
            restitution: Restitution::new(0.5),
        }
    }

    pub fn player() -> Self {
        Self {
            collider: Collider::rectangle(PLAYER_SIZE, PLAYER_SIZE),
            collider_density: ColliderDensity(0.2),
            rigid_body: RigidBody::Dynamic,
            restitution: Restitution::new(0.3),
        }
    }
}

// Components

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Reflect)]
pub struct PlayerId(pub PeerId);

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ColorComponent(pub Color);

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BallMarker;

// Inputs using leafwing

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Copy, Hash, Reflect, Actionlike)]
pub enum PlayerActions {
    Up,
    Down,
    Left,
    Right,
}

// Protocol Plugin

#[derive(Clone)]
pub struct ProtocolPlugin;

impl Plugin for ProtocolPlugin {
    fn build(&self, app: &mut App) {
        // Leafwing inputs
        app.add_plugins(leafwing::InputPlugin::<PlayerActions> {
            config: InputConfig {
                rebroadcast_inputs: true,
                ..default()
            },
        });

        // Components
        app.register_component::<PlayerId>();
        app.register_component::<ColorComponent>();
        app.register_component::<BallMarker>();

        app.register_component::<Position>()
            .add_prediction()
            .add_should_rollback(position_should_rollback)
            .add_linear_interpolation()
            .add_linear_correction_fn();

        app.register_component::<Rotation>()
            .add_prediction()
            .add_should_rollback(rotation_should_rollback)
            .add_linear_interpolation()
            .add_linear_correction_fn();

        // Velocities need prediction but not interpolation
        app.register_component::<LinearVelocity>().add_prediction();
        app.register_component::<AngularVelocity>().add_prediction();
    }
}

fn position_should_rollback(this: &Position, that: &Position) -> bool {
    (this.0 - that.0).length() >= 0.01
}

fn rotation_should_rollback(this: &Rotation, that: &Rotation) -> bool {
    this.angle_between(*that) >= 0.01
}
