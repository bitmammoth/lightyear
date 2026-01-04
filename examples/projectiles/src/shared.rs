//! Shared code between client and server for projectiles

use crate::protocol::*;
use avian2d::prelude::*;
use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;
use core::time::Duration;
use lightyear::avian2d::plugin::AvianReplicationMode;
use lightyear::prelude::*;
use lightyear_avian2d::prelude::LagCompensationPlugin;

// Network settings
pub const PROTOCOL_ID: u64 = 0;
pub const PRIVATE_KEY: [u8; 32] = [0u8; 32];

pub const FIXED_TIMESTEP_HZ: f64 = 64.0;
pub const SERVER_REPLICATION_INTERVAL: Duration = Duration::from_millis(100);

// Server ports
pub const SERVER_UDP_PORT: u16 = 5000;
pub const SERVER_WEBTRANSPORT_PORT: u16 = 5001;
pub const SERVER_WEBSOCKET_PORT: u16 = 5002;

// Game constants
pub const PLAYER_MOVE_SPEED: f32 = 1.5;
pub const BULLET_MOVE_SPEED: f32 = 300.0;
pub const HITSCAN_LIFETIME: f32 = 0.15;
pub const HITSCAN_RANGE: f32 = 2000.0;
const MAP_LIMIT: f32 = 500.0;

pub struct SharedPlugin;

impl Plugin for SharedPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ProtocolPlugin);

        // Movement observers
        app.add_observer(rotate_player);
        app.add_observer(move_player);
        
        // Shooting
        app.add_observer(shoot_weapon);
        
        // Physics
        app.add_plugins(lightyear::avian2d::plugin::LightyearAvianPlugin {
            replication_mode: AvianReplicationMode::Position,
            ..default()
        });
        app.add_plugins(
            PhysicsPlugins::default()
                .build()
                .disable::<PhysicsTransformPlugin>()
                .disable::<PhysicsInterpolationPlugin>()
                .disable::<IslandPlugin>()
                .disable::<IslandSleepingPlugin>(),
        )
        .insert_resource(Gravity(Vec2::ZERO));

        // Update systems
        app.add_systems(FixedUpdate, (update_bullets, update_hitscan_visuals));
        app.add_systems(PreUpdate, despawn_after);
    }
}

/// Generate color from client ID
pub fn color_from_id(client_id: PeerId) -> Color {
    let h = (((client_id.to_bits().wrapping_mul(90)) % 360) as f32) / 360.0;
    let s = 1.0;
    let l = 0.5;
    Color::hsl(h, s, l)
}

/// Create player bundle
pub fn player_bundle(peer_id: PeerId) -> impl Bundle {
    let color = color_from_id(peer_id);
    let y = (peer_id.to_bits() as f32 * 50.0) % 400.0 - 200.0;
    (
        Position::from(Vec2::new(0.0, y)),
        Rotation::default(),
        ColorComponent(color),
        PhysicsBundle::player(),
        PlayerMarker,
        PlayerId(peer_id),
        Score(0),
        Weapon::default(),
        Name::from("Player"),
    )
}

/// Rotate player to face cursor
pub fn rotate_player(
    trigger: On<Fire<MoveCursor>>,
    mut player: Query<(&mut Rotation, &Position), Without<Interpolated>>,
) {
    if let Ok((mut rotation, position)) = player.get_mut(trigger.context) {
        let angle = Vec2::new(0.0, 1.0).angle_to(trigger.value - position.0);
        if (angle - rotation.as_radians()).abs() > 0.0001 {
            *rotation = Rotation::from(angle);
        }
    }
}

/// Move player based on input
pub fn move_player(
    trigger: On<Fire<MovePlayer>>,
    mut player: Query<&mut Position, Without<Interpolated>>,
) {
    if let Ok(mut position) = player.get_mut(trigger.context) {
        let value = trigger.value;
        if value.x > 0.0 { position.x += PLAYER_MOVE_SPEED; }
        if value.x < 0.0 { position.x -= PLAYER_MOVE_SPEED; }
        if value.y > 0.0 { position.y += PLAYER_MOVE_SPEED; }
        if value.y < 0.0 { position.y -= PLAYER_MOVE_SPEED; }
        
        // Clamp to map bounds
        position.x = position.x.clamp(-MAP_LIMIT, MAP_LIMIT);
        position.y = position.y.clamp(-MAP_LIMIT, MAP_LIMIT);
    }
}

/// Shoot weapon based on current weapon type
pub fn shoot_weapon(
    trigger: On<Complete<Shoot>>,
    mut commands: Commands,
    timeline: Res<LocalTimeline>,
    query: SpatialQuery,
    mut player_query: Query<(
        Entity,
        &PlayerId,
        &Position,
        &Rotation,
        &ColorComponent,
        &mut Weapon,
        Option<&ControlledBy>,
    ), With<PlayerMarker>>,
    global: Query<&WeaponType, With<ClientContext>>,
) {
    let tick = timeline.tick();
    let shooter = trigger.context;
    
    let weapon_type = global.iter().next().copied().unwrap_or_default();

    if let Ok((entity, id, position, rotation, color, mut weapon, controlled_by)) =
        player_query.get_mut(shooter)
    {
        // Check fire rate
        if let Some(last_fire) = weapon.last_fire_tick {
            let ticks_since = tick - last_fire;
            let time_since = Duration::from_secs_f64(ticks_since as f64 / 64.0);
            let min_interval = Duration::from_secs_f32(1.0 / weapon_type.fire_rate());
            if time_since < min_interval {
                return;
            }
        }

        weapon.last_fire_tick = Some(tick);
        let is_server = controlled_by.is_some();

        info!(?tick, ?weapon_type, "Player {:?} shooting", shooter);

        match weapon_type {
            WeaponType::Hitscan => {
                shoot_hitscan(&mut commands, &query, entity, position, rotation, color, is_server);
            }
            WeaponType::Bullet => {
                shoot_bullet(&mut commands, entity, id, position, rotation, color, is_server);
            }
        }
    }
}

fn shoot_hitscan(
    commands: &mut Commands,
    query: &SpatialQuery,
    shooter: Entity,
    position: &Position,
    rotation: &Rotation,
    color: &ColorComponent,
    is_server: bool,
) {
    let direction = *rotation * Vec2::Y;
    let start = position.0;
    
    // Ray cast to find hit
    let end = if let Some(hit) = query.cast_ray(
        start,
        Dir2::new(direction).unwrap_or(Dir2::Y),
        HITSCAN_RANGE,
        true,
        &SpatialQueryFilter::from_excluded_entities([shooter]),
    ) {
        start + direction * hit.distance
    } else {
        start + direction * HITSCAN_RANGE
    };

    // Spawn hitscan visual (only on server for replication, or client for prediction)
    if is_server {
        commands.spawn((
            HitscanVisual {
                start,
                end,
                lifetime: HITSCAN_LIFETIME,
                max_lifetime: HITSCAN_LIFETIME,
            },
            Position(start),
            ColorComponent(color.0),
            Replicate::to_clients(NetworkTarget::All),
            Name::new("Hitscan"),
        ));
    }
}

fn shoot_bullet(
    commands: &mut Commands,
    shooter: Entity,
    player_id: &PlayerId,
    position: &Position,
    rotation: &Rotation,
    color: &ColorComponent,
    is_server: bool,
) {
    let direction = *rotation * Vec2::Y;
    let velocity = direction * BULLET_MOVE_SPEED;
    
    if is_server {
        commands.spawn((
            Position(position.0 + direction * (PLAYER_SIZE / 2.0 + BULLET_SIZE)),
            *rotation,
            LinearVelocity(velocity),
            ColorComponent(color.0),
            BulletMarker { shooter },
            PhysicsBundle::bullet(),
            DespawnAfter(Timer::from_seconds(5.0, TimerMode::Once)),
            Replicate::to_clients(NetworkTarget::All),
            PredictionTarget::to_clients(NetworkTarget::All),
            Name::new("Bullet"),
        ));
    }
}

/// Update bullet positions
fn update_bullets(
    mut bullets: Query<(&mut Position, &LinearVelocity), With<BulletMarker>>,
    time: Res<Time<Fixed>>,
) {
    for (mut pos, vel) in bullets.iter_mut() {
        pos.0 += vel.0 * time.delta_secs();
    }
}

/// Update hitscan visual lifetime
fn update_hitscan_visuals(
    mut commands: Commands,
    mut hitscans: Query<(Entity, &mut HitscanVisual)>,
    time: Res<Time<Fixed>>,
) {
    for (entity, mut visual) in hitscans.iter_mut() {
        visual.lifetime -= time.delta_secs();
        if visual.lifetime <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

/// Component to despawn entity after timer
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Reflect)]
pub struct DespawnAfter(pub Timer);

fn despawn_after(
    mut commands: Commands,
    mut query: Query<(Entity, &mut DespawnAfter)>,
    time: Res<Time>,
) {
    for (entity, mut despawn) in query.iter_mut() {
        despawn.0.tick(time.delta());
        if despawn.0.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

use serde::{Deserialize, Serialize};
