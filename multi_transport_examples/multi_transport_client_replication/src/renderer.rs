//! Renderer module - draws cursors.

use bevy::prelude::*;
use crate::protocol::*;

pub struct ExampleRendererPlugin;

impl Plugin for ExampleRendererPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, init);
        app.add_systems(Update, draw_cursors);
    }
}

fn init(mut commands: Commands) {
    commands.spawn(Camera2d);
}

/// Draw cursor circles with their assigned colors
pub fn draw_cursors(
    mut gizmos: Gizmos,
    cursors: Query<(&CursorPosition, &PlayerColor)>,
) {
    for (position, color) in &cursors {
        gizmos.circle_2d(position.0, 10.0, color.0);
    }
}
