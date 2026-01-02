//! Protocol definitions for projectiles example

use crate::shared::color_from_id;
use avian2d::prelude::*;
use bevy::ecs::entity::MapEntities;
use bevy::prelude::*;
use lightyear::input::bei::prelude::*;
use lightyear::input::prelude::InputConfig;
use lightyear::prelude::*;
use serde::{Deserialize, Serialize};

pub const BULLET_SIZE: f32 = 3.0;
pub const PLAYER_SIZE: f32 = 40.0;

// Components
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Reflect)]
pub struct PlayerId(pub PeerId);

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Reflect)]
pub struct PlayerMarker;

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Reflect)]
pub struct Score(pub usize);

#[derive(Component, Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Reflect)]
pub struct ColorComponent(pub Color);

#[derive(Component, MapEntities, Serialize, Deserialize, Clone, Debug, PartialEq, Reflect)]
pub struct BulletMarker {
    #[entities]
    pub shooter: Entity,
}

// Input context
#[derive(Component, Serialize, Deserialize, Reflect, Clone, Debug, PartialEq)]
pub struct PlayerContext;

// Input actions
#[derive(Debug, InputAction)]
#[action_output(Vec2)]
pub struct MovePlayer;

#[derive(Debug, InputAction)]
#[action_output(Vec2)]
pub struct MoveCursor;

#[derive(Debug, InputAction)]
#[action_output(bool)]
pub struct Shoot;

// Global context for weapon/mode switching
#[derive(Component, Serialize, Deserialize, Reflect, Clone, Debug, PartialEq)]
pub struct ClientContext;

#[derive(Debug, InputAction)]
#[action_output(bool)]
pub struct CycleWeapon;

// Weapon types
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Reflect, Default)]
pub enum WeaponType {
    #[default]
    Hitscan,
    Bullet,
}

impl WeaponType {
    pub fn next(&self) -> Self {
        match self {
            WeaponType::Hitscan => WeaponType::Bullet,
            WeaponType::Bullet => WeaponType::Hitscan,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            WeaponType::Hitscan => "Hitscan",
            WeaponType::Bullet => "Linear Projectile",
        }
    }

    pub fn fire_rate(&self) -> f32 {
        match self {
            WeaponType::Hitscan => 5.0,
            WeaponType::Bullet => 2.0,
        }
    }
}

// Weapon component
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Reflect)]
pub struct Weapon {
    pub last_fire_tick: Option<Tick>,
}

impl Default for Weapon {
    fn default() -> Self {
        Self { last_fire_tick: None }
    }
}

// Hitscan visual component
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Reflect)]
pub struct HitscanVisual {
    pub start: Vec2,
    pub end: Vec2,
    pub lifetime: f32,
    pub max_lifetime: f32,
}

// Hit detection event
#[derive(MapEntities, Event, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct HitDetected {
    #[entities]
    pub shooter: Entity,
    #[entities]
    pub target: Entity,
}

// Channel for hit events
pub struct HitChannel;

// Physics bundle
#[derive(Bundle)]
pub struct PhysicsBundle {
    pub collider: Collider,
    pub collider_density: ColliderDensity,
    pub rigid_body: RigidBody,
}

impl PhysicsBundle {
    pub fn player() -> Self {
        Self {
            collider: Collider::rectangle(PLAYER_SIZE, PLAYER_SIZE),
            collider_density: ColliderDensity(0.2),
            rigid_body: RigidBody::Kinematic,
        }
    }

    pub fn bullet() -> Self {
        Self {
            collider: Collider::circle(BULLET_SIZE),
            collider_density: ColliderDensity(0.05),
            rigid_body: RigidBody::Dynamic,
        }
    }
}

// Protocol plugin
pub struct ProtocolPlugin;

impl Plugin for ProtocolPlugin {
    fn build(&self, app: &mut App) {
        // inputs
        app.add_plugins(InputPlugin::new(InputConfig::<PlayerContext> {
            lag_compensation: true,
            rebroadcast_inputs: true,
            ..default()
        }));
        app.register_input_action::<MovePlayer>();
        app.register_input_action::<MoveCursor>();
        app.register_input_action::<Shoot>();

        app.add_plugins(InputPlugin::new(InputConfig::<ClientContext> {
            ignore_rollbacks: true,
            ..default()
        }));
        app.register_input_action::<CycleWeapon>();

        // channel
        app.add_channel::<HitChannel>(ChannelSettings {
            mode: ChannelMode::OrderedReliable(ReliableSettings::default()),
            ..default()
        })
        .add_direction(NetworkDirection::Bidirectional);

        // messages
        app.register_event::<HitDetected>()
            .add_map_entities()
            .add_direction(NetworkDirection::ClientToServer);

        // components
        app.register_component::<Name>();
        app.register_component::<PlayerId>();
        app.register_component::<PlayerMarker>();

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

        app.register_component::<LinearVelocity>()
            .add_prediction()
            .add_should_rollback(linear_velocity_should_rollback);

        app.register_component::<ColorComponent>();
        app.register_component::<Score>();
        app.register_component::<HitscanVisual>();
        app.register_component::<RigidBody>();
        app.register_component::<BulletMarker>().add_map_entities();
        app.register_component::<WeaponType>();
        app.register_component::<Weapon>().add_prediction();
    }
}

fn position_should_rollback(this: &Position, that: &Position) -> bool {
    (this.0 - that.0).length() >= 0.01
}

fn rotation_should_rollback(this: &Rotation, that: &Rotation) -> bool {
    this.angle_between(*that) >= 0.01
}

fn linear_velocity_should_rollback(this: &LinearVelocity, that: &LinearVelocity) -> bool {
    (this.0 - that.0).length() >= 0.01
}
