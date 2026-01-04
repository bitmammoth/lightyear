# Multi-Transport Examples Status

## Current Status: ✅ WORKING!

### What Works
- 14+ multi-transport examples compile and run
- Multi-transport architecture: ONE Server entity + multiple transport entities
- UDP, WebTransport, and WebSocket all working
- Entity replication across all transports
- Both connection orders work (UDP first or WS first)

### Bug Analysis Complete
The order-dependent replication issue was investigated. Key findings:
1. `handle_connection` observer properly adds new clients to existing entities' ReplicationState
2. `on_insert` hook for `Replicate::to_all()` properly queries existing clients
3. Both code paths work correctly with `ReplicationMode::Target`

The issue in earlier testing may have been due to stale build artifacts or race conditions during rapid testing. After clean rebuild, both directions work:

## Latest: multi_transport_spaceships demo - BALL SYNC ISSUE

### Location
`multi_transport_demos/multi_transport_spaceships/`

### Current Issue: Ball Desync
When UDP client and WebSocket client both connect, balls start in sync but diverge after collisions.

**Observed behavior:**
- Server balls: 181v0-186v0 at initial positions (125.0, 0.0), (62.5, 108.3), etc.
- UDP client balls: 177v0-183v0 - positions diverge after player collision
- WebSocket client balls: 178v0-183v0 - different diverged positions

**Root Cause Analysis:**
1. Each client runs its own Avian2D physics simulation
2. Balls have `PredictionTarget` so they're `Predicted` on clients
3. `Predicted` entities run local physics AND receive server corrections
4. Collisions cause local physics to diverge, server corrections aren't fast/strong enough

**Key Discovery:**
- Entity IDs are DIFFERENT between server/clients (expected - local entity allocation)
- Ball POSITIONS diverge after collisions (the actual problem)
- Server shows balls mostly stationary, but clients show them moving from local physics

**Code Review:**
- Ball init matches original spaceships (PredictionTarget::to_all instead of to_clients)
- Client `add_ball_physics` adds `ball.physics_bundle()` (dynamic physics)
- Original spaceships may have same issue - need to test

**Potential Solutions:**
1. Don't run physics on balls client-side (use Kinematic not Dynamic)
2. Use InterpolationTarget instead of PredictionTarget for balls
3. Increase server update rate / correction strength

### Key Files
- `main.rs` - CLI with server/client/host-client commands
- `server.rs` - Multi-transport startup (UDP:5000, WT:5001, WS:5002)
- `client.rs` - Transport selection (UDP/WebTransport/WebSocket/Local)
- `shared.rs` - FIXED_TIMESTEP_HZ constant and shared systems
- `protocol.rs` - Same as original spaceships

### Architecture
```
Server entity (187v0)
├── UdpTransport (188v0) - TransportOf -> Server
├── WebTransportTransport (189v0) - TransportOf -> Server
└── WebSocketTransport (190v0) - TransportOf -> Server

Clients connect via any transport -> LinkOf points to Server entity
```

### Key Implementation Details
1. **Server startup()** spawns:
   - ONE Server entity (no NetcodeServer, just Server::default())
   - Three transport entities (each with NetcodeServer + TransportOf)
   - Manually add `Started` to Server entity

2. **Client startup()** spawns client with appropriate transport IO:
   - UDP: `UdpIo::default()`
   - WebTransport: `WebTransportClientIo { certificate_digest }`
   - WebSocket: `WebSocketClientIo { config, scheme: WebSocketScheme::Secure }`

3. **Input delay**: Observer on `On<Add, Client>` since Client entity created at runtime

### Commands
```bash
# Server (runs UDP:5000, WebTransport:5001, WebSocket:5002)
cargo run -p multi_transport_spaceships -- server

# UDP Client
cargo run -p multi_transport_spaceships -- client --transport udp

# WebSocket Client  
cargo run -p multi_transport_spaceships -- client --transport websocket

# WebTransport Client (copy cert from server)
cargo run -p multi_transport_spaceships -- client --transport webtransport --cert <DIGEST>
```

### Previous: multi_transport_lobby ✅
Also working with same multi-transport pattern.

## Critical Bug Fixes Applied
1. **ServerUdpIo vs UdpIo**: Server MUST use `ServerUdpIo::default()`, client uses `UdpIo::default()`
2. **Started component**: Multi-transport servers need `Started` manually added to Server entity
3. **Input delay**: Use observer pattern, not direct query before app.run()
