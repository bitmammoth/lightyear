# Multi-Transport Examples Progress

## Status: 12/16 Examples Complete

### Completed Examples ✅
1. `simple_box` - Basic replication
2. `distributed_authority` - Authority transfer
3. `network_visibility` - Interest management
4. `client_replication` - Client->server replication
5. `replication_groups` - Linked entities
6. `priority` - Priority-based replication
7. `delta_compression` - Delta compression
8. `auth` - ConnectToken authentication
9. `bevy_enhanced_inputs` - BEI input integration
10. `avian_physics` - 2D physics with avian2d
11. `avian_3d_character` - 3D character controller
12. `deterministic_replication` - Deterministic with checksums

### Remaining Examples ❌
- `projectiles` - Complex: 6 game modes, lag compensation, client-side hit detection
- `lobby` - Complex: Dynamic host-server, lobby management
- `launcher` - Uses Docker, RON configs, separate binaries
- `fps` - 3D FPS, lag compensation

## Architecture Reference

### Multi-Transport Server Pattern
```rust
// 1. Spawn logical server
let server = commands.spawn((Server::default(), Name::new("Server"))).id();

// 2. UDP transport
commands.spawn((
    NetcodeServer::new(NetcodeConfig::default()),
    LocalAddr(SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 5000)),
    ServerUdpIo::default(),
    TransportOf::new(server),
    Name::new("UdpTransport"),
));
commands.trigger(Start { entity: udp_transport });

// 3. WebTransport
let identity = Identity::self_signed(vec!["localhost".to_string(), "127.0.0.1".to_string()]).unwrap();
commands.spawn((
    NetcodeServer::new(NetcodeConfig::default()),
    LocalAddr(wt_addr),
    WebTransportServerIo { certificate: identity },
    TransportOf::new(server),
));

// 4. WebSocket
let ws_config = lightyear::websocket::server::ServerConfig::builder()
    .with_bind_address(ws_addr)
    .with_identity(lightyear::websocket::server::Identity::self_signed(ws_sans).unwrap());
commands.spawn((
    NetcodeServer::new(NetcodeConfig::default()),
    LocalAddr(ws_addr),
    WebSocketServerIo { config: ws_config },
    TransportOf::new(server),
));
```

### Multi-Transport Client Pattern
```rust
let auth = Authentication::Manual {
    server_addr,
    client_id: config.client_id,
    private_key: Key::default(),
    protocol_id: PROTOCOL_ID,
};

// UDP
commands.spawn((
    Client::default(),
    LocalAddr(client_addr),
    PeerAddr(server_addr),
    Link::new(None),
    ReplicationReceiver::default(),
    NetcodeClient::new(auth, NetcodeConfig::default()).unwrap(),
    UdpIo::default(),
));

// WebTransport
commands.spawn((
    ...,
    WebTransportClientIo { certificate_digest: cert_digest },
));

// WebSocket
commands.spawn((
    ...,
    WebSocketClientIo {
        config: WebSocketClientConfig::builder().with_no_cert_validation(),
        scheme: WebSocketScheme::Secure,
    },
));
```

### Key Ports
- UDP: 5000
- WebTransport: 5001
- WebSocket: 5002

### Key Imports
- `lightyear::prelude::server::*` - Server types
- `lightyear::prelude::client::*` - Client types
- `lightyear::prelude::*` - Common types
- `lightyear::avian2d::plugin::LightyearAvianPlugin` - 2D physics
- `lightyear::avian3d::plugin::LightyearAvian3dPlugin` - 3D physics

## Last Session
- Completed deterministic_replication example
- All 12 examples compile together
- Next: Consider projectiles or lobby (both complex)
