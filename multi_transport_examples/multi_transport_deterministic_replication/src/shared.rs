//! Shared code between client and server for deterministic replication

use crate::protocol::*;
use avian2d::prelude::*;
use bevy::prelude::*;
use core::time::Duration;
use leafwing_input_manager::prelude::ActionState;
use lightyear::avian2d::plugin::AvianReplicationMode;
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

// Input delay for deterministic replication
pub const INPUT_DELAY_TICKS: u16 = 0;

pub const MAX_VELOCITY: f32 = 200.0;
const WALL_SIZE: f32 = 350.0;

pub struct SharedPlugin;

impl Plugin for SharedPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ProtocolPlugin);

        // Physics plugins with deterministic settings
        app.add_plugins(lightyear::avian2d::plugin::LightyearAvianPlugin {
            replication_mode: AvianReplicationMode::Position,
            rollback_resources: true,
            ..default()
        });
        app.add_plugins(
            PhysicsPlugins::default()
                .build()
                .disable::<PhysicsTransformPlugin>()
                .disable::<PhysicsInterpolationPlugin>()
                // disable island sleeping plugin as it's not compatible with rollbacks
                .disable::<IslandPlugin>()
                .disable::<IslandSleepingPlugin>(),
        )
        .insert_resource(Gravity(Vec2::ZERO));
    }
}

/// Wall bundle for arena boundaries
#[derive(Bundle)]
pub struct WallBundle {
    position: Position,
    collider: Collider,
    rigid_body: RigidBody,
    color: ColorComponent,
}

impl WallBundle {
    pub fn new(start: Vec2, end: Vec2, color: Color) -> Self {
        let center = (start + end) / 2.0;
        let length = (end - start).length();
        
        Self {
            position: Position(center),
            collider: Collider::rectangle(length, 10.0),
            rigid_body: RigidBody::Static,
            color: ColorComponent(color),
        }
    }
}

/// Spawn arena walls
pub fn spawn_walls(commands: &mut Commands) {
    commands.spawn(WallBundle::new(
        Vec2::new(-WALL_SIZE, -WALL_SIZE),
        Vec2::new(-WALL_SIZE, WALL_SIZE),
        Color::WHITE,
    ));
    commands.spawn(WallBundle::new(
        Vec2::new(-WALL_SIZE, WALL_SIZE),
        Vec2::new(WALL_SIZE, WALL_SIZE),
        Color::WHITE,
    ));
    commands.spawn(WallBundle::new(
        Vec2::new(WALL_SIZE, WALL_SIZE),
        Vec2::new(WALL_SIZE, -WALL_SIZE),
        Color::WHITE,
    ));
    commands.spawn(WallBundle::new(
        Vec2::new(WALL_SIZE, -WALL_SIZE),
        Vec2::new(-WALL_SIZE, -WALL_SIZE),
        Color::WHITE,
    ));
}

/// Generate color from client ID
pub fn color_from_id(client_id: PeerId) -> Color {
    let h = (((client_id.to_bits().wrapping_mul(30)) % 360) as f32) / 360.0;
    let s = 1.0;
    let l = 0.5;
    Color::hsl(h, s, l)
}

/// Player bundle with physics components
pub fn player_bundle(peer_id: PeerId) -> impl Bundle {
    let color = color_from_id(peer_id);
    let y = (peer_id.to_bits() as f32 * 50.0) % 500.0 - 250.0;
    (
        Position::from(Vec2::new(-50.0, y)),
        ColorComponent(color),
        PhysicsBundle::player(),
    )
}

/// Shared movement behavior
pub fn shared_movement_behaviour(
    mut velocity: Mut<LinearVelocity>,
    action: &ActionState<PlayerActions>,
) {
    const MOVE_SPEED: f32 = 10.0;
    if action.pressed(&PlayerActions::Up) {
        velocity.y += MOVE_SPEED;
    }
    if action.pressed(&PlayerActions::Down) {
        velocity.y -= MOVE_SPEED;
    }
    if action.pressed(&PlayerActions::Left) {
        velocity.x -= MOVE_SPEED;
    }
    if action.pressed(&PlayerActions::Right) {
        velocity.x += MOVE_SPEED;
    }
    *velocity = LinearVelocity(velocity.clamp_length_max(MAX_VELOCITY));
}
