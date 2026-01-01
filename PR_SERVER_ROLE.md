# Multi-Transport Architecture: FishNet-Style Single Server

## Summary

This PR implements a **FishNet/Mirror-style multi-transport architecture** where there is ONE logical `Server` entity and multiple transport entities that feed connections to it via `TransportOf`.

## FishNet → Lightyear Architecture Mapping

| FishNet (Unity OOP) | Lightyear (Bevy ECS) | Notes |
|---------------------|---------------------|-------|
| `NetworkManager` (GameObject) | **Server entity** with `Server` component | Singleton-ish, relationship target for all `LinkOf` |
| `NetworkManager.TransportManager` | **Queries over transports** with `TransportOf` | ECS iterates all transports naturally |
| `NetworkManager.ServerManager` | Systems in `lightyear_link`/`lightyear_replication` | Logic in systems, not objects |
| `NetworkManager.ClientManager` | Systems + `LinkOf` relationships | Each client = entity with `LinkOf` |
| `Transport` (abstract class) | **Transport marker components** (`ServerUdpIo`, etc.) | Each transport = entity |
| `Multipass` (Transport wrapper) | **Multiple transport entities + `TransportOf`** | ECS handles multi-transport naturally! |
| `Multipass._transports[]` | Query for all `(TransportOf, &mut TransportIo)` | |
| `Multipass.multipassId` → transportId mapping | `LinkOf` entity → transport marker component | Each link knows its transport via marker |
| `NetworkConnection` | **Entity with `LinkOf`** + transport-specific marker | `UdpLinkOfIO`, `WebTransportLinkOfIO`, etc. |

### Key Insight: Bevy ECS is Naturally "Multipass"

In FishNet, `Multipass` exists because they need to wrap multiple transports behind ONE `Transport` interface. They track:
- `_transports[]` - array of transports  
- `_multpassIdLookup` - unified multipassId → (transportIndex, transportId)
- `_transportIdLookup[]` - per-transport transportId → multipassId

**In Bevy ECS, we don't need explicit ID remapping!** The `Server` component automatically collects all `LinkOf` relationships via its `links: Vec<Entity>`. Each client connection is an entity that:
1. Has `LinkOf { server }` pointing to the ONE Server
2. Has a transport-specific marker (`UdpLinkOfIO`, `WebTransportLinkOfIO`, etc.)

### Queries for Different Transport Scenarios

\`\`\`rust
// ALL clients regardless of transport (equivalent to FishNet's ServerManager.Clients)
Query<(Entity, &LinkOf)>

// UDP clients only  
Query<(Entity, &LinkOf), With<UdpLinkOfIO>>

// WebTransport clients only
Query<(Entity, &LinkOf), With<WebTransportLinkOfIO>>

// Get the Server entity and all its clients
Query<&Server>  // Server.links contains all LinkOf entities
\`\`\`

## Architecture Diagrams

### Before (Multiple Server Entities)
\`\`\`
Server (SteamServerIo) ──── LinkOf (client)
Server (UdpServerIo) ────── LinkOf (client)  
Server (WtServerIo) ─────── LinkOf (client)
                            LinkOf (client)
\`\`\`

### After (One Server, Multiple Transports)
\`\`\`
        ┌─────────────────────────────────────────────────────┐
        │               Server Entity                          │
        │  ┌─────────────────────────────────────────────────┐│
        │  │ Server { links: [client1, client2, client3, ...] }│
        │  └─────────────────────────────────────────────────┘│
        └─────────────────────────────────────────────────────┘
                              ▲
          ┌───────────────────┼───────────────────┐
          │                   │                   │
   ┌──────┴──────┐     ┌──────┴──────┐     ┌──────┴──────┐
   │ UDP Transport│     │ WT Transport │     │ WS Transport│
   │TransportOf{srv}│   │TransportOf{srv}│   │TransportOf{srv}│
   └──────────────┘     └──────────────┘     └──────────────┘
          │                   │                   │
          ▼                   ▼                   ▼
   ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
   │ LinkOf{srv} │     │ LinkOf{srv} │     │ LinkOf{srv} │
   │ UdpLinkOfIO │     │WTLinkOfIO   │     │ WSLinkOfIO  │
   └─────────────┘     └─────────────┘     └─────────────┘
\`\`\`

## Implementation Details

### 1. `TransportOf` Component (lightyear_link/src/server.rs)

Links a transport entity to its Server.

\`\`\`rust
#[derive(Component, Clone, Copy, Debug, Reflect)]
pub struct TransportOf {
    pub server: Entity,
}

impl TransportOf {
    pub fn new(server: Entity) -> Self {
        Self { server }
    }
}
\`\`\`

### 2. `ViaTransport` Component (lightyear_link/src/server.rs)

Tracks which transport entity spawned each client `LinkOf`. Essential for multi-transport setups where each transport's protocol layer (e.g., NetcodeServer) needs to know which clients belong to it.

\`\`\`rust
#[derive(Component, Clone, Copy, Debug, Reflect)]
pub struct ViaTransport {
    pub transport: Entity,
}

impl ViaTransport {
    pub fn new(transport: Entity) -> Self {
        Self { transport }
    }
}
\`\`\`

### 3. Transport Entities (example: UDP)

\`\`\`rust
// ServerUdpIo NO LONGER has #[require(Server)]
#[derive(Component)]
#[require(IoServer::udp())]  // Just marks it as a server transport
pub struct ServerUdpIo { ... }

// When spawning new client connections:
fn receive(transport_entity: Entity, transport_of: &TransportOf, ...) {
    commands.spawn((
        LinkOf { server: transport_of.server },  // Points to THE Server
        ViaTransport { transport: transport_entity },  // Tracks which transport spawned this
        link,
        Linked,
        PeerAddr(address),
        UdpLinkOfIO,  // Marker for transport type
    ));
}
\`\`\`

### 4. Protocol Layer Filtering (lightyear_netcode/src/server_plugin.rs)

The netcode server plugin filters clients by `ViaTransport` to only process clients that came through the same transport:

\`\`\`rust
fn send(
    transport_query: Query<(Entity, &mut NetcodeServer, &TransportOf), Without<Stopped>>,
    server_query: Query<&Server>,
    client_query: Query<(Entity, &mut Link, &ViaTransport, ...), With<LinkOf>>,
) {
    for (transport_entity, mut netcode_server, transport_of) in transport_query.iter_mut() {
        let server = server_query.get(transport_of.server)?;
        
        // Only process clients that came through THIS transport
        for (entity, link, via_transport, ...) in client_query.iter_many(server.collection()) {
            if via_transport.transport == transport_entity {
                // Process this client - it belongs to this transport's NetcodeServer
                netcode_server.send(...);
            }
        }
    }
}
\`\`\`

### 3. Spawning Multi-Transport Server

\`\`\`rust
// Spawn ONE Server entity
let server = commands.spawn(Server::default()).id();

// Spawn multiple transports pointing to it
commands.spawn((
    ServerUdpIo::default(),
    TransportOf::new(server),
    LocalAddr(udp_addr),
));

commands.spawn((
    WebTransportServerIo { ... },
    TransportOf::new(server),
    LocalAddr(wt_addr),
));

commands.spawn((
    WebSocketServerIo::default(),
    TransportOf::new(server),
    LocalAddr(ws_addr),
));
\`\`\`

## Files Changed

### Core
- `lightyear_link/src/server.rs` - Added `TransportOf` and `ViaTransport` components
- `lightyear_netcode/src/server_plugin.rs` - Updated queries to use `TransportOf` lookup and `ViaTransport` filtering

### Transports (removed `#[require(Server)]`, use `TransportOf` and add `ViaTransport` to new clients)
- `lightyear_udp/src/server.rs` - Added `UdpLinkOfIO` marker, spawns with `ViaTransport`
- `lightyear_webtransport/src/server.rs` - Added `WebTransportLinkOfIO` marker, spawns with `ViaTransport`
- `lightyear_websocket/src/server.rs` - Added `WebSocketLinkOfIO` marker
- `lightyear_steam/src/server.rs` - Uses existing `SteamClientOf` marker

### Examples
- `examples/multi_transport/src/server.rs` - Updated to new architecture

## Compatibility

This is a **breaking change** for anyone spawning server transports:

**Before:**
\`\`\`rust
commands.spawn((ServerUdpIo::default(), LocalAddr(addr)));
// Server component was auto-added via #[require(Server)]
\`\`\`

**After:**
\`\`\`rust
let server = commands.spawn(Server::default()).id();
commands.spawn((ServerUdpIo::default(), TransportOf::new(server), LocalAddr(addr)));
\`\`\`

## Benefits

1. **Entities match across transports** - A player connecting via UDP or WebTransport gets a `LinkOf` pointing to the same `Server` entity
2. **Clean queries** - Query all clients: `Query<&LinkOf>`, query by transport: `Query<&LinkOf, With<UdpLinkOfIO>>`
3. **Natural ECS pattern** - No need for FishNet's `Multipass` ID remapping; Bevy relationships handle it
4. **Single source of truth** - One `Server` entity holds all game state, timelines, etc.

## Next Steps / TODO

- [x] Full integration test with multi_transport example - **WORKING!**
- [ ] Update book documentation
- [ ] Consider adding `ServerRole` resource for server state (Running/Stopped/etc.)
- [ ] Consider transport priority/preferences for replication
- [ ] Add `ViaTransport` to WebSocket transport
