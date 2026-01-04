//! Shared code between client and server

use bevy::prelude::*;

use crate::protocol::*;

pub struct SharedPlugin;

impl Plugin for SharedPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ProtocolPlugin);
    }
}

/// Shared movement behavior
pub fn shared_movement_behaviour(mut position: Mut<PlayerPosition>, input: &Inputs) {
    match input {
        Inputs::Direction(direction) => {
            if direction.up {
                position.y += MOVE_SPEED;
            }
            if direction.down {
                position.y -= MOVE_SPEED;
            }
            if direction.left {
                position.x -= MOVE_SPEED;
            }
            if direction.right {
                position.x += MOVE_SPEED;
            }
        }
    }
}
