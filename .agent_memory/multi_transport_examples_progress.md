# Multi-Transport Examples Progress

## Status: 14 of 16 COMPLETE (examples) + 1 DEMO COMPLETE

## Completed Examples (All Compile)
1. ✅ multi_transport_simple_box
2. ✅ multi_transport_distributed_authority
3. ✅ multi_transport_network_visibility
4. ✅ multi_transport_client_replication
5. ✅ multi_transport_replication_groups
6. ✅ multi_transport_priority
7. ✅ multi_transport_delta_compression
8. ✅ multi_transport_auth
9. ✅ multi_transport_bevy_enhanced_inputs
10. ✅ multi_transport_avian_physics
11. ✅ multi_transport_avian_3d_character
12. ✅ multi_transport_deterministic_replication
13. ✅ multi_transport_projectiles
14. ✅ multi_transport_lobby

## Completed Demos
1. ✅ multi_transport_spaceships (in multi_transport_demos/)
   - Server mode: All 3 transports (UDP:5000, WT:5001, WS:5002)
   - Client mode: UDP/WebTransport/WebSocket options
   - Host-client mode: Local channels, tested working

## Remaining Examples
15. ❌ multi_transport_launcher (combines multiple examples)
16. ❌ multi_transport_fps (most complex)

## Key Patterns
- ONE Server entity + multiple transport entities with TransportOf/ViaTransport
- Standard Ports: UDP:5000, WebTransport:5001, WebSocket:5002
- InputPlugin needs MapEntities impl for Inputs type
- Use `input::native::InputPlugin::<Inputs>::default()`
- WebSocketServerIo needs config (not Default): `WebSocketServerIo { config: ws_config }`
- WebSocketClientIo needs config and scheme: `WebSocketClientIo { config: ..., scheme: WebSocketScheme::Secure }`

## Host-Client Mode Key Startup Order
```rust
// CRITICAL: server::startup BEFORE client::startup
app.add_systems(Startup, (
    server::startup,
    client::startup,
).chain());
```

## Common Fixes Applied
- `rotation.rotate()` deprecated → use `rotation * Vec2::Y`
- MapEntities impl required for Inputs type
- Bevy features needed: bevy_gizmos, bevy_sprite, bevy_render, bevy_text, bevy_core_pipeline, tonemapping_luts
