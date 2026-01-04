//! Rendering code for visualizing player heads + trails.

use bevy::prelude::*;

use crate::protocol::*;

// ============ Plugin ============

pub struct ExampleRendererPlugin;

impl Plugin for ExampleRendererPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_camera);
        app.add_systems(Update, draw_entities);
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

/// Draw player heads and their connected trails
fn draw_entities(
    mut gizmos: Gizmos,
    players: Query<(Entity, &PlayerPosition, &PlayerColor), With<PlayerId>>,
    trails: Query<(&TrailPosition, &TrailColor, &PlayerParent)>,
) {
    for (player_entity, player_pos, player_color) in players.iter() {
        // Draw player head as a larger box
        gizmos.rect_2d(
            Isometry2d::from_translation(player_pos.0),
            Vec2::splat(50.0),
            player_color.0,
        );
        
        // Find and draw trail for this player
        for (trail_pos, trail_color, parent) in trails.iter() {
            if parent.0 == player_entity {
                // Draw trail as a smaller box
                gizmos.rect_2d(
                    Isometry2d::from_translation(trail_pos.0),
                    Vec2::splat(25.0),
                    trail_color.0,
                );
                // Draw connecting line
                gizmos.line_2d(
                    player_pos.0,
                    trail_pos.0,
                    player_color.0.with_alpha(0.5),
                );
            }
        }
    }
}
