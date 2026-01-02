//! Shared code between client and server

use crate::protocol::*;
use avian2d::prelude::*;
use avian2d::PhysicsPlugins;
use bevy::prelude::*;
use core::time::Duration;
use leafwing_input_manager::prelude::ActionState;

use lightyear::prelude::*;
use lightyear_avian2d::plugin::AvianReplicationMode;

const EPS: f32 = 0.0001;
pub const BOT_RADIUS: f32 = 15.0;
pub const BOT_MOVE_SPEED: f32 = 1.0;
const BULLET_MOVE_SPEED: f32 = 300.0;

pub struct SharedPlugin;

impl Plugin for SharedPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ProtocolPlugin);

        // Avian2d physics integration
        app.add_plugins(lightyear::avian2d::plugin::LightyearAvianPlugin {
            replication_mode: AvianReplicationMode::PositionButInterpolateTransform,
            ..default()
        });

        app.add_systems(PreUpdate, despawn_after);

        // Movement and shooting systems
        app.add_systems(
            FixedUpdate,
            (predicted_bot_movement, player_movement, shoot_bullet).chain(),
        );
        
        // Physics plugins
        app.add_plugins(
            PhysicsPlugins::default()
                .build()
                .disable::<PhysicsTransformPlugin>(),
        )
        .insert_resource(Gravity(Vec2::ZERO));
    }
}

/// Generate pseudo-random color from id
pub fn color_from_id(client_id: PeerId) -> Color {
    let h = (((client_id.to_bits().wrapping_mul(90)) % 360) as f32) / 360.0;
    let s = 1.0;
    let l = 0.5;
    Color::hsl(h, s, l)
}

/// Shared player movement behavior
pub fn shared_player_movement(
    mut position: Mut<Position>,
    mut rotation: Mut<Rotation>,
    action: &ActionState<PlayerActions>,
) {
    const PLAYER_MOVE_SPEED: f32 = 10.0;
    let Some(cursor_data) = action.dual_axis_data(&PlayerActions::MoveCursor) else {
        return;
    };
    let angle = Vec2::new(0.0, 1.0).angle_to(cursor_data.pair - position.0);
    if (angle - rotation.as_radians()).abs() > EPS {
        *rotation = Rotation::from(angle);
    }
    if action.pressed(&PlayerActions::Up) {
        position.y += PLAYER_MOVE_SPEED;
    }
    if action.pressed(&PlayerActions::Down) {
        position.y -= PLAYER_MOVE_SPEED;
    }
    if action.pressed(&PlayerActions::Right) {
        position.x += PLAYER_MOVE_SPEED;
    }
    if action.pressed(&PlayerActions::Left) {
        position.x -= PLAYER_MOVE_SPEED;
    }
}

fn player_movement(
    mut player_query: Query<
        (
            &mut Position,
            &mut Rotation,
            &ActionState<PlayerActions>,
        ),
        (Or<(With<Predicted>, With<Replicate>)>, With<PlayerMarker>),
    >,
) {
    for (position, rotation, action_state) in player_query.iter_mut() {
        shared_player_movement(position, rotation, action_state);
    }
}

fn predicted_bot_movement(
    timeline: Res<LocalTimeline>,
    mut query: Query<&mut Position, With<PredictedBot>>,
) {
    let tick = timeline.tick();
    query.iter_mut().for_each(|mut position| {
        let direction = if (tick.0 / 200) % 2 == 0 { 1.0 } else { -1.0 };
        position.x += BOT_MOVE_SPEED * direction;
    });
}

/// Shoot a bullet from the player
pub fn shoot_bullet(
    mut commands: Commands,
    timeline: Res<LocalTimeline>,
    mut query: Query<
        (
            &PlayerId,
            &Transform,
            &ColorComponent,
            &mut ActionState<PlayerActions>,
            Option<&ControlledBy>,
        ),
        (Or<(With<Predicted>, With<Replicate>)>, With<PlayerMarker>),
    >,
) {
    let tick = timeline.tick();
    for (id, transform, color, action, controlled_by) in query.iter_mut() {
        let is_server = controlled_by.is_some();
        if action.just_pressed(&PlayerActions::Shoot) {
            info!(?tick, pos=?transform.translation.truncate(), "spawn bullet");
            for delta in [0.0] {
                let salt: u64 = if delta < 0.0 { 0 } else { 1 };
                let mut bullet_transform = transform.clone();
                bullet_transform.rotate_z(delta);
                let bullet_bundle = (
                    bullet_transform,
                    LinearVelocity(bullet_transform.up().as_vec3().truncate() * BULLET_MOVE_SPEED),
                    RigidBody::Kinematic,
                    *id,
                    *color,
                    BulletMarker,
                    Name::new("Bullet"),
                );

                if is_server {
                    commands.spawn((
                        bullet_bundle,
                        PreSpawned::default_with_salt(salt),
                        DespawnAfter(Timer::new(Duration::from_secs(2), TimerMode::Once)),
                        Replicate::to_clients(NetworkTarget::All),
                        PredictionTarget::to_clients(NetworkTarget::Single(id.0)),
                        InterpolationTarget::to_clients(NetworkTarget::AllExceptSingle(id.0)),
                        controlled_by.unwrap().clone(),
                    ));
                } else {
                    commands.spawn((bullet_bundle, PreSpawned::default_with_salt(salt)));
                }
            }
        }
    }
}

#[derive(Component)]
pub struct DespawnAfter(pub Timer);

fn despawn_after(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut DespawnAfter)>,
) {
    for (entity, mut despawn_after) in query.iter_mut() {
        despawn_after.0.tick(time.delta());
        if despawn_after.0.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}
