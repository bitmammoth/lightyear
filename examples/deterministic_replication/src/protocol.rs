//! Protocol definitions for deterministic replication

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

#[derive(Component, Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct ColorComponent(pub Color);

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BallMarker;

// Inputs
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Copy, Hash, Reflect, Actionlike)]
pub enum PlayerActions {
    Up,
    Down,
    Left,
    Right,
}

// Protocol
#[derive(Clone)]
pub struct ProtocolPlugin;

impl Plugin for ProtocolPlugin {
    fn build(&self, app: &mut App) {
        // inputs
        app.add_plugins(leafwing::InputPlugin::<PlayerActions> {
            config: InputConfig {
                rebroadcast_inputs: true,
                ..default()
            },
        });

        // components
        app.register_component::<PlayerId>();

        // add prediction for non-networked physics components with checksums
        app.add_rollback::<Position>()
            // add a hash function to perform a checksum in order to catch desyncs
            .add_custom_hash(lightyear::avian2d::types::position::hash)
            // register a linear interpolation function without actually running Interpolation systems
            // it will be used for FrameInterpolation
            .register_linear_interpolation()
            .add_linear_correction_fn();

        app.add_rollback::<Rotation>()
            .add_custom_hash(lightyear::avian2d::types::rotation::hash)
            .register_linear_interpolation()
            .add_linear_correction_fn();

        // NOTE: interpolation/correction is only needed for components that are visually displayed!
        // we still need prediction to be able to correctly predict the physics on the client
        app.add_rollback::<LinearVelocity>();
        app.add_rollback::<AngularVelocity>();
    }
}
