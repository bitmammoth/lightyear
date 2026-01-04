//! Shared code between client and server

use crate::protocol::*;
use avian3d::prelude::*;
use bevy::prelude::*;
use core::time::Duration;
use leafwing_input_manager::prelude::ActionState;
use lightyear::avian3d::plugin::AvianReplicationMode;
use lightyear::prelude::*;

// Network settings
pub const PROTOCOL_ID: u64 = 0;
pub const PRIVATE_KEY: [u8; 32] = [0u8; 32];

pub const FIXED_TIMESTEP_HZ: f64 = 64.0;
pub const SERVER_REPLICATION_INTERVAL: Duration = Duration::from_millis(100);

// Server ports
pub const SERVER_UDP_PORT: u16 = 5000;
pub const SERVER_WEBTRANSPORT_PORT: u16 = 5001;
pub const SERVER_WEBSOCKET_PORT: u16 = 5002;

// Character dimensions
pub const CHARACTER_CAPSULE_RADIUS: f32 = 0.5;
pub const CHARACTER_CAPSULE_HEIGHT: f32 = 0.5;

// Floor dimensions
pub const FLOOR_WIDTH: f32 = 100.0;
pub const FLOOR_HEIGHT: f32 = 1.0;

pub struct SharedPlugin;

impl Plugin for SharedPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ProtocolPlugin);

        // Physics plugins
        app.add_plugins(lightyear::avian3d::plugin::LightyearAvianPlugin {
            replication_mode: AvianReplicationMode::Position,
            ..default()
        });
        app.add_plugins(
            PhysicsPlugins::default()
                .build()
                .disable::<PhysicsTransformPlugin>()
                .disable::<PhysicsInterpolationPlugin>(),
        );
    }
}

/// Character physics bundle
#[derive(Bundle)]
pub struct CharacterPhysicsBundle {
    pub collider: Collider,
    pub rigid_body: RigidBody,
    pub lock_axes: LockedAxes,
    pub friction: Friction,
}

impl Default for CharacterPhysicsBundle {
    fn default() -> Self {
        Self {
            collider: Collider::capsule(CHARACTER_CAPSULE_RADIUS, CHARACTER_CAPSULE_HEIGHT),
            rigid_body: RigidBody::Dynamic,
            lock_axes: LockedAxes::default()
                .lock_rotation_x()
                .lock_rotation_y()
                .lock_rotation_z(),
            friction: Friction::new(0.0).with_combine_rule(CoefficientCombine::Min),
        }
    }
}

/// Floor physics bundle
#[derive(Bundle)]
pub struct FloorPhysicsBundle {
    pub collider: Collider,
    pub rigid_body: RigidBody,
}

impl Default for FloorPhysicsBundle {
    fn default() -> Self {
        Self {
            collider: Collider::cuboid(FLOOR_WIDTH, FLOOR_HEIGHT, FLOOR_WIDTH),
            rigid_body: RigidBody::Static,
        }
    }
}

/// Generate color from client ID
pub fn color_from_id(client_id: PeerId) -> Color {
    let h = (((client_id.to_bits().wrapping_mul(30)) % 360) as f32) / 360.0;
    let s = 1.0;
    let l = 0.5;
    Color::hsl(h, s, l)
}

/// Apply character movement
pub fn apply_character_action(
    entity: Entity,
    velocity: &mut LinearVelocity,
    spatial_query: &SpatialQuery,
    action_state: &ActionState<CharacterAction>,
    position: &Position,
) {
    const MAX_SPEED: f32 = 5.0;
    const MOVE_FORCE: f32 = 20.0;
    const JUMP_IMPULSE: f32 = 10.0;

    // Movement
    let axis_pair = action_state.clamped_axis_pair(&CharacterAction::Move);
    let move_dir = Vec3::new(axis_pair.x, 0.0, -axis_pair.y);
    if move_dir.length_squared() > 0.01 {
        velocity.x = (velocity.x + move_dir.x * MOVE_FORCE * 0.016).clamp(-MAX_SPEED, MAX_SPEED);
        velocity.z = (velocity.z + move_dir.z * MOVE_FORCE * 0.016).clamp(-MAX_SPEED, MAX_SPEED);
    }

    // Jumping
    if action_state.just_pressed(&CharacterAction::Jump) {
        let ray_origin = position.0 + Vec3::new(0.0, -CHARACTER_CAPSULE_HEIGHT / 2.0 - CHARACTER_CAPSULE_RADIUS, 0.0);
        
        // Check if grounded
        if spatial_query.cast_ray(
            ray_origin,
            Dir3::NEG_Y,
            0.1,
            true,
            &SpatialQueryFilter::from_excluded_entities([entity]),
        ).is_some() {
            velocity.y = JUMP_IMPULSE;
        }
    }
}
