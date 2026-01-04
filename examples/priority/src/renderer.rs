//! Rendering code for visualizing shapes and players.

use bevy::prelude::*;

use crate::protocol::*;

// ============ Plugin ============

pub struct ExampleRendererPlugin;

impl Plugin for ExampleRendererPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_camera);
        app.add_systems(Update, (draw_shapes, draw_players));
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

/// Draw shapes in the grid
fn draw_shapes(
    mut gizmos: Gizmos,
    shapes: Query<(&Position, &Shape)>,
) {
    for (pos, shape) in shapes.iter() {
        let color = Color::srgba(0.5, 0.5, 0.5, 0.8);
        let size = 8.0;
        
        match shape {
            Shape::Circle => {
                gizmos.circle_2d(
                    Isometry2d::from_translation(pos.0),
                    size,
                    color,
                );
            }
            Shape::Triangle => {
                // Draw triangle using lines
                let offset = size;
                let p1 = pos.0 + Vec2::new(0.0, offset);
                let p2 = pos.0 + Vec2::new(-offset * 0.866, -offset * 0.5);
                let p3 = pos.0 + Vec2::new(offset * 0.866, -offset * 0.5);
                gizmos.line_2d(p1, p2, color);
                gizmos.line_2d(p2, p3, color);
                gizmos.line_2d(p3, p1, color);
            }
            Shape::Square => {
                gizmos.rect_2d(
                    Isometry2d::from_translation(pos.0),
                    Vec2::splat(size * 2.0),
                    color,
                );
            }
        }
    }
}

/// Draw player boxes
fn draw_players(
    mut gizmos: Gizmos,
    players: Query<(&PlayerPosition, &PlayerColor)>,
) {
    for (pos, color) in players.iter() {
        gizmos.rect_2d(
            Isometry2d::from_translation(pos.0),
            Vec2::splat(30.0),
            color.0,
        );
    }
}
