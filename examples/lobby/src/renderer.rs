//! Renderer for the lobby example

use bevy::prelude::*;
use bevy_egui::EguiPlugin;

use crate::protocol::*;

pub struct RendererPlugin;

impl Plugin for RendererPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(EguiPlugin::default());
        }
        app.add_systems(Startup, init);
        app.add_systems(Update, draw_boxes);
    }
}

fn init(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn draw_boxes(
    mut gizmos: Gizmos,
    players: Query<(&PlayerPosition, &PlayerColor)>,
) {
    for (position, color) in &players {
        gizmos.rect_2d(
            Isometry2d::from_translation(position.0),
            Vec2::ONE * 50.0,
            color.0,
        );
    }
}
