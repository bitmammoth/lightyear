//! Renderer plugin for the FPS example

use crate::protocol::*;
use crate::shared::BOT_RADIUS;
use bevy::color::palettes::basic::GREEN;
use bevy::color::palettes::css::BLUE;
use bevy::prelude::*;
use lightyear::prelude::Predicted;
use lightyear_frame_interpolation::{FrameInterpolate, FrameInterpolationPlugin};

pub struct RendererPlugin;

impl Plugin for RendererPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, init);
        
        app.add_observer(add_predicted_player_visuals);
        app.add_plugins(FrameInterpolationPlugin::<Transform>::default());
        
        app.add_systems(Update, (
            draw_players,
            draw_bullets,
            draw_bots,
        ));
    }
}

fn init(mut commands: Commands) {
    commands.spawn(Camera2d);
}

/// When predicted player spawns, add frame interpolation
fn add_predicted_player_visuals(
    trigger: On<Add, (PlayerId, Predicted)>,
    mut commands: Commands,
) {
    commands.entity(trigger.entity)
        .insert(FrameInterpolate::<Transform>::default());
}

/// Draw all players as rectangles using gizmos
fn draw_players(
    mut gizmos: Gizmos,
    query: Query<(&Transform, &ColorComponent), With<PlayerMarker>>,
) {
    for (transform, color) in query.iter() {
        gizmos.rect_2d(
            Isometry2d::new(transform.translation.truncate(), Rot2::radians(transform.rotation.to_euler(EulerRot::XYZ).2)),
            Vec2::splat(PLAYER_SIZE),
            color.0,
        );
    }
}

/// Draw all bullets as circles using gizmos
fn draw_bullets(
    mut gizmos: Gizmos,
    query: Query<(&Transform, &ColorComponent), With<BulletMarker>>,
) {
    for (transform, color) in query.iter() {
        gizmos.circle_2d(
            Isometry2d::new(transform.translation.truncate(), Rot2::default()),
            BULLET_SIZE,
            color.0,
        );
    }
}

/// Draw bots as circles using gizmos
fn draw_bots(
    mut gizmos: Gizmos,
    interpolated_query: Query<&Transform, With<InterpolatedBot>>,
    predicted_query: Query<&Transform, With<PredictedBot>>,
) {
    for transform in interpolated_query.iter() {
        gizmos.circle_2d(
            Isometry2d::new(transform.translation.truncate(), Rot2::default()),
            BOT_RADIUS,
            Color::from(GREEN),
        );
    }
    for transform in predicted_query.iter() {
        gizmos.circle_2d(
            Isometry2d::new(transform.translation.truncate(), Rot2::default()),
            BOT_RADIUS,
            Color::from(BLUE),
        );
    }
}
