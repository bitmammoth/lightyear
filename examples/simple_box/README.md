# Multi-Transport Example

This comprehensive example demonstrates lightyear's **multi-transport architecture**, enabling a single server to accept connections from clients using different network transports simultaneously.

## Features Demonstrated

### Core Multi-Transport
- **Single logical server** accepting connections via UDP, WebTransport, and WebSocket
- **Transport decoupling** using `TransportOf` relationship (transports are separate entities from the Server)
- **Unified game state** - all clients see the same world regardless of transport

### Replication & Prediction
- **Server-authoritative spawning** - players spawned when clients connect
- **Client prediction** - immediate local response to inputs
- **Server reconciliation** - corrections from authoritative server
- **Entity interpolation** - smooth rendering of other players

### Input System
- **Native inputs** - keyboard input capture with Direction enum
- **Input buffering** - reliable input transmission to server
- **Shared movement** - same movement logic on client and server

### Messaging
- **Bidirectional messages** - client-to-client messaging via server relay
- **Named entities** - players identified by transport name (UDP, WS, WT)

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Server Entity (logical)                   │
│  - Server component                                          │
│  - LocalTimeline                                             │
│  - Started                                                   │
└─────────────────────────────────────────────────────────────┘
         ▲                    ▲                    ▲
         │ TransportOf        │ TransportOf        │ TransportOf
         │                    │                    │
┌────────┴────────┐  ┌────────┴────────┐  ┌────────┴────────┐
│  UDP Transport  │  │  WT Transport   │  │  WS Transport   │
│  :5000          │  │  :5001          │  │  :5002          │
│  NetcodeServer  │  │  NetcodeServer  │  │  NetcodeServer  │
│  Started        │  │  Started        │  │  Started        │
└─────────────────┘  └─────────────────┘  └─────────────────┘
```

## Running the Example

### Start Server
```bash
cargo run -p simple_box -- server
```

The server starts all three transports:
- UDP on port 5000
- WebTransport on port 5001 (prints certificate digest)
- WebSocket on port 5002

### Connect Clients

**UDP Client:**
```bash
cargo run -p simple_box -- client --transport udp
# or short form
cargo run -p simple_box -- client -t udp
```

**WebSocket Client:**
```bash
cargo run -p simple_box -- client --transport websocket
# or short form
cargo run -p simple_box -- client -t ws
```

**WebTransport Client:**
```bash
# Copy the certificate digest from server output, then:
cargo run -p simple_box -- client --transport webtransport --cert <DIGEST>
# or short form
cargo run -p simple_box -- client -t wt -c <DIGEST>
```

## Controls

- **Arrow Keys / WASD**: Move player
- Players are rendered as colored boxes using gizmos

## Code Structure

```
src/
├── main.rs      # CLI and app setup
├── shared.rs    # Protocol, components, input types
├── server.rs    # Multi-transport server setup, player spawning, movement
├── client.rs    # Client connection, prediction, input handling
└── renderer.rs  # Camera and gizmo rendering
```

## Key Implementation Details

### Transport Setup (server.rs)
```rust
// 1. Spawn logical Server entity
let server = commands.spawn((Server::default(), Name::new("GameServer"))).id();

// 2. Spawn transport entities with TransportOf relationship
commands.spawn((
    NetcodeServer::new(NetcodeConfig::default()),
    LocalAddr(udp_addr),
    ServerUdpIo::default(),
    TransportOf::new(server),  // Links to logical server
    Name::new("UdpTransport"),
));
```

### Player Spawning (server.rs)
```rust
fn handle_connected(trigger: On<Add, Connected>, mut commands: Commands) {
    let client_entity = trigger.entity;
    
    commands.spawn((
        PlayerId(client_id),
        PlayerPosition::default(),
        ActionState::<Inputs>::default(),
        // Replication targets
        PredictionTarget::to_clients(NetworkTarget::Single(client_id)),
        InterpolationTarget::default(),
        Replicating::to_clients(NetworkTarget::All),
        Name::new(client_name),
    ));
}
```

### Input Handling (client.rs)
```rust
fn buffer_input(
    tick: Res<LocalTimeline>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut query: Query<&mut InputBuffer<ActionState<Inputs>, Inputs>, With<InputMarker>>,
) {
    let direction = Direction {
        up: keyboard.pressed(KeyCode::KeyW) || keyboard.pressed(KeyCode::ArrowUp),
        down: keyboard.pressed(KeyCode::KeyS) || keyboard.pressed(KeyCode::ArrowDown),
        left: keyboard.pressed(KeyCode::KeyA) || keyboard.pressed(KeyCode::ArrowLeft),
        right: keyboard.pressed(KeyCode::KeyD) || keyboard.pressed(KeyCode::ArrowRight),
    };
    
    for mut buffer in query.iter_mut() {
        buffer.set(tick.tick(), &ActionState(Inputs::Direction(direction)));
    }
}
```

## Testing Scenarios

1. **Single UDP client**: Basic connectivity test
2. **Single WebSocket client**: Web-compatible transport test
3. **Single WebTransport client**: Modern web transport test
4. **Multiple clients same transport**: Test client isolation
5. **Multiple clients different transports**: Test unified game state
6. **Client disconnect/reconnect**: Test cleanup and rejoin

## Related Changes to Lightyear Core

This example requires the following changes to lightyear (included in this fork):

1. **`lightyear_inputs/src/server.rs`**: Changed `With<Started>` to `With<Server>` in `update_action_state` query to support multi-transport
2. **`lightyear_link/src/server.rs`**: Added `TransportOf` relationship for decoupled transports
3. **`lightyear_connection/src/server_role.rs`**: Added `ServerRole` resource for transport state tracking
