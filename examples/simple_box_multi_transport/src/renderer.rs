//! Renderer - draws player boxes using sprites + inspector GUI
use crate::protocol::*;
use bevy::prelude::*;
use bevy_inspector_egui::bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use lightyear::prelude::*;

/// Marker for the visual sprite of a player
#[derive(Component)]
pub struct PlayerSprite;

#[derive(Clone)]
pub struct RendererPlugin {
    pub is_server: bool,
}

impl Plugin for RendererPlugin {
    fn build(&self, app: &mut App) {
        // Add egui first, then inspector GUI
        app.add_plugins(EguiPlugin::default());
        app.add_plugins(WorldInspectorPlugin::new());
        
        app.add_systems(Startup, init);
        app.add_systems(Update, spawn_player_sprites);
        app.add_systems(Update, update_sprite_positions);
        
        // Register types for inspector
        app.register_type::<PlayerPosition>();
    }
}

fn init(mut commands: Commands) {
    // Spawn camera at origin
    commands.spawn((
        Camera2d,
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));
    info!("📷 Camera2d spawned at origin");
    
    // Spawn a TEST sprite at origin to verify rendering works
    commands.spawn((
        Sprite {
            color: Color::srgb(1.0, 0.0, 0.0),  // Bright red
            custom_size: Some(Vec2::new(100.0, 100.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0),
        Visibility::Visible,
        Name::new("TEST_SPRITE_AT_ORIGIN"),
    ));
    info!("🔴 TEST: Red sprite spawned at origin");
}

/// Spawn sprites for new players
fn spawn_player_sprites(
    mut commands: Commands,
    players: Query<(Entity, &PlayerPosition, &PlayerColor, Option<&Predicted>, Option<&Interpolated>), (Added<Player>, Without<PlayerSprite>)>,
) {
    for (entity, pos, color, predicted, interpolated) in &players {
        let mode = if predicted.is_some() { "Predicted" } else if interpolated.is_some() { "Interpolated" } else { "None" };
        info!("🎨 SPAWNING SPRITE for Player {:?} at {:?} color {:?} mode {}", entity, pos.0, color.0, mode);
        
        // Add sprite components with explicit visibility
        commands.entity(entity).insert((
            Sprite {
                color: color.0,
                custom_size: Some(Vec2::new(50.0, 50.0)),
                ..default()
            },
            Transform::from_xyz(pos.0.x, pos.0.y, 0.0),
            Visibility::Visible,
            PlayerSprite,
        ));
    }
}

/// Update sprite positions to match PlayerPosition
fn update_sprite_positions(
    mut players: Query<(&PlayerPosition, &mut Transform), With<PlayerSprite>>,
) {
    for (pos, mut transform) in &mut players {
        transform.translation.x = pos.0.x;
        transform.translation.y = pos.0.y;
    }
}
