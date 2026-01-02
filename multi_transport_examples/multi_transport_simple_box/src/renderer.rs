//! Renderer module - draws player boxes using gizmos
//!
//! This module provides visual rendering of player entities using Bevy's gizmo system.
//! Each player is drawn as a colored rectangle at their position.

use crate::shared::*;
use bevy::prelude::*;

#[derive(Clone)]
pub struct ExampleRendererPlugin;

impl Plugin for ExampleRendererPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, init);
        app.add_systems(Update, draw_boxes);
    }
}

/// Initialize the 2D camera for rendering
fn init(mut commands: Commands) {
    commands.spawn(Camera2d);
}

/// System that draws the boxes of the player positions.
/// The components should be replicated from the server to the client
pub(crate) fn draw_boxes(
    mut gizmos: Gizmos, 
    players: Query<(&PlayerPosition, &PlayerColor)>,
) {
    for (position, color) in &players {
        // Draw the player box
        gizmos.rect_2d(
            Isometry2d::from_translation(position.0),
            Vec2::ONE * 50.0,
            color.0,
        );
    }
}
