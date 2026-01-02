//! Renderer module - draws players and circles.

use bevy::color::palettes::basic::GREEN;
use bevy::prelude::*;
use crate::protocol::*;

pub struct ExampleRendererPlugin;

impl Plugin for ExampleRendererPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, init);
        app.add_systems(Update, (draw_players, draw_circles));
    }
}

fn init(mut commands: Commands) {
    commands.spawn(Camera2d);
}

/// Draw player boxes with their assigned colors
pub fn draw_players(
    mut gizmos: Gizmos,
    players: Query<(&Position, &PlayerColor), Without<CircleMarker>>,
) {
    for (position, color) in &players {
        gizmos.rect(
            Isometry3d::from_translation(Vec3::new(position.x, position.y, 0.0)),
            Vec2::ONE * 50.0,
            color.0,
        );
    }
}

/// Draw circles (small green dots) - only visible ones will be present
pub fn draw_circles(
    mut gizmos: Gizmos,
    circles: Query<&Position, With<CircleMarker>>,
) {
    for position in &circles {
        gizmos.circle_2d(Isometry2d::from_translation(position.0), 5.0, GREEN);
    }
}
