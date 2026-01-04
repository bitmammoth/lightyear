# Multi-Transport Simple Setup Example

This minimal example showcases how to setup lightyear with multiple transports (UDP, WebTransport, WebSocket).

## Running

### Start Server
```bash
cargo run -p simple_setup -- server
```

The server will start all three transports:
- UDP on port 5000
- WebTransport on port 5001 (prints certificate digest)  
- WebSocket on port 5002

### Connect Clients

**UDP:**
```bash
cargo run -p simple_setup -- client -t udp
```

**WebSocket:**
```bash
cargo run -p simple_setup -- client -t websocket
```

**WebTransport:**
```bash
# Copy certificate digest from server output
cargo run -p simple_setup -- client -t webtransport -c <DIGEST>
```

## Key Multi-Transport Concepts

1. **Single logical Server entity** - All transports point to one Server via `TransportOf`
2. **Multiple transport entities** - Each transport (UDP, WT, WS) is a separate entity
3. **Unified client handling** - Clients appear the same regardless of transport
