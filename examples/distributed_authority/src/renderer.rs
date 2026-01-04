//! Renderer module - draws players and the ball using gizmos.
//!
//! Used on both server and client for visual feedback.

use bevy::prelude::*;
use crate::protocol::*;

pub struct ExampleRendererPlugin;

impl Plugin for ExampleRendererPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, init);
        app.add_systems(Update, (draw_players, draw_ball));
    }
}

fn init(mut commands: Commands) {
    commands.spawn(Camera2d);
}

/// Draw player boxes with their assigned colors
pub fn draw_players(
    mut gizmos: Gizmos,
    players: Query<(&Position, &PlayerColor), Without<BallMarker>>,
) {
    for (position, color) in &players {
        gizmos.rect_2d(
            Isometry2d::from_translation(position.0),
            Vec2::ONE * 50.0,
            color.0,
        );
    }
}

/// Draw the ball as a circle with the color of its current authority owner
pub fn draw_ball(
    mut gizmos: Gizmos,
    balls: Query<(&Position, &PlayerColor), With<BallMarker>>,
) {
    for (position, color) in balls.iter() {
        gizmos.circle_2d(position.0, 25.0, color.0);
    }
}
