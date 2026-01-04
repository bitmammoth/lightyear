# Multi-Transport Spaceships Debug Status

## Current Status: Changed balls to InterpolationTarget - testing needed

### Major Change (2025-01-03):
**Problem:** With `PredictionTarget`, each client was running its own physics simulation on balls. This caused:
- Physics divergence between clients
- Balls appearing at different positions
- Inconsistent collision behavior

**Solution:** Changed balls from `PredictionTarget` to `InterpolationTarget`:
- Server is fully authoritative for ball physics
- Clients just display interpolated server state
- Clients add `Kinematic` rigid body (not Dynamic) so players can collide with balls
- Ball movement is smooth via interpolation

### Key Architecture Changes:
1. **Server `init()`**: Changed `PredictionTarget::to_all()` → `InterpolationTarget::to_all()` for balls
2. **Client `add_ball_physics()`**: 
   - Changed filter from `With<Predicted>` to `With<Interpolated>`
   - Changed physics from `Dynamic` rigid body to `Kinematic`
   - Kinematic bodies participate in collisions but don't simulate physics

### Entity Setup Summary:
| Entity | Server | Client |
|--------|--------|--------|
| **Own Player** | Dynamic + Replicate | Predicted + Dynamic (full physics) |
| **Other Players** | Dynamic + Replicate | Interpolated + Kinematic (collide but don't simulate) |
| **Balls** | Dynamic + Replicate | Interpolated + Kinematic (collide but don't simulate) |
| **Bullets** | Dynamic + Replicate | Predicted + Dynamic (full physics) |

### Files Modified:
- `server.rs` - Ball init: `InterpolationTarget::to_all()` + added `Rotation::default()`
- `client.rs` - `add_ball_physics`: filter `Interpolated`, use `RigidBody::Kinematic`
- `client.rs` - `debug_client_positions`: filter `Interpolated` for balls

### Expected Behavior:
- Balls should now be perfectly in sync across all clients
- Players can still collide with balls (kinematic colliders)
- Ball movement comes entirely from server
- Smooth interpolation between server updates
