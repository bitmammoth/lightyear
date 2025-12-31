# RFC: ServerRole Resource for Multi-Transport Support

## Summary

This PR introduces a `ServerRole` resource that represents the "logical server" independent of IO transport types, enabling cleaner multi-transport architectures (e.g., simultaneous Steam + UDP + WebTransport connections).

## Motivation

Currently, each IO transport type (Steam, UDP, WebTransport) creates its own `Server` entity. This causes issues when an application wants to run multiple transports simultaneously:

1. **Multiple Server entities** - Which one is "the" server for game logic?
2. **Duplicate state** - Each Server could theoretically have its own timeline/state
3. **Confusing queries** - Systems need to query all Server entities or pick one arbitrarily

This is similar to how Unity's FishNet/Mirror handle this - they have a single `NetworkManager` that manages multiple transports.

### Current Architecture
```
Server (SteamServerIo) ──── LinkOf (client)
Server (UdpServerIo) ────── LinkOf (client)
Server (WtServerIo) ─────── LinkOf (client)
                            LinkOf (client)
```

### Proposed Architecture  
```
ServerRole (Resource) ─── Single logical server identity
       │
       ├── Server (SteamServerIo) ──── LinkOf
       ├── Server (UdpServerIo) ────── LinkOf  
       └── Server (WtServerIo) ─────── LinkOf, LinkOf
```

## Design

### New Types

```rust
/// The logical server role - a Resource
#[derive(Resource)]
pub struct ServerRole {
    pub state: ServerRoleState,
}

pub enum ServerRoleState {
    Stopped,   // No transports running
    Starting,  // At least one transport starting
    Running,   // At least one transport running
    Stopping,  // All transports stopping
}

/// Marker for IO transport entities
#[derive(Component)]
pub struct IoServer {
    pub transport_name: &'static str,
}
```

### Usage

```rust
// Check if we're a server (game logic)
fn my_server_system(
    server_role: Res<ServerRole>,
) {
    if server_role.is_running() {
        // Do server things
    }
}

// Iterate all connected clients regardless of transport
fn broadcast_to_all(
    links: Query<&Link, With<Connected>>,
) {
    for link in links.iter() {
        // Send to all clients on any transport
    }
}

// Transport-specific logic
fn steam_specific(
    steam_links: Query<&Link, (With<Connected>, With<SteamClientOf>)>,
) {
    // Steam-specific handling
}
```

### Run Conditions

```rust
// New run conditions
pub fn is_server_running(server_role: Option<Res<ServerRole>>) -> bool;
pub fn has_server_role(server_role: Option<Res<ServerRole>>) -> bool;
```

## Implementation Plan

1. ✅ Add `ServerRole` resource and `ServerRolePlugin`
2. ⬜ Add `IoServer` component to transport crates (lightyear_udp, lightyear_steam, etc.)
3. ⬜ Update `is_server` / `is_headless_server` run conditions to use `ServerRole`
4. ⬜ Update documentation
5. ⬜ Add integration test with multiple transports

## Compatibility

This is **additive** and **non-breaking**:
- Existing code continues to work
- `Server` entities still function the same way
- New code can optionally use `ServerRole` for cleaner architecture

## Related Discussion

From NOTES.md:
> "maybe we just consider all Link entities in the World, because we will basically never have multiple 'logical' Servers"

From PROMPT.md:
> "multiple 'servers' that are still part of a same global server - i.e. global server timeline, multiple Server entities (Websocket, WebTransport, etc.) that each have their own ClientOfs. but otherwise the big 'SERVER' is the same. (it is a resource)"

This PR implements exactly that vision.

## Questions for Maintainer

1. Should `ServerRole` eventually contain the server's `LocalTimeline` reference?
2. Should `IoServer` be a required component when `Server` + IO component is added?
3. How should this interact with the P2P topology plans?
