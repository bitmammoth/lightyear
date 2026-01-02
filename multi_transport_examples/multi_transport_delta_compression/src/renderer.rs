//! Rendering code for visualizing players.

use bevy::prelude::*;

use crate::protocol::*;

// ============ Plugin ============

pub struct ExampleRendererPlugin;

impl Plugin for ExampleRendererPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_camera);
        app.add_systems(Update, draw_players);
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

/// Draw player boxes
fn draw_players(
    mut gizmos: Gizmos,
    players: Query<(&PlayerPosition, &PlayerColor)>,
) {
    for (pos, color) in players.iter() {
        gizmos.rect_2d(
            Isometry2d::from_translation(pos.0),
            Vec2::splat(50.0),
            color.0,
        );
    }
}
