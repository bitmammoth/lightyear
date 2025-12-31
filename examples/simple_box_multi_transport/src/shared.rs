//! Shared systems and registration.

use crate::protocol::*;
use bevy::prelude::*;
use lightyear::prelude::input::native::ActionState;

/// Plugin to register the shared protocol components.
pub struct SharedPlugin;

impl Plugin for SharedPlugin {
    fn build(&self, app: &mut App) {
        // Register protocol (includes input plugin registration)
        app.add_plugins(ProtocolPlugin);
    }
}

// This system defines how we update the player's positions when we receive an input
pub(crate) fn shared_movement_behaviour(mut position: Mut<PlayerPosition>, input: &ActionState<Inputs>) {
    const MOVE_SPEED: f32 = 10.0;
    let Inputs::Direction(direction) = &input.0;
    if direction.up {
        position.0.y += MOVE_SPEED;
    }
    if direction.down {
        position.0.y -= MOVE_SPEED;
    }
    if direction.left {
        position.0.x -= MOVE_SPEED;
    }
    if direction.right {
        position.0.x += MOVE_SPEED;
    }
}
