//! Multi-Transport Simple Box Example Library
//!
//! This module exports the client and server functionality so they can be
//! called from the launcher or other applications.

use bevy::prelude::*;
use core::time::Duration;
use std::net::SocketAddr;

pub mod client;
pub mod renderer;
pub mod server;
pub mod shared;

pub use client::Transport as TransportType;
use shared::*;

/// Run the multi-transport server
/// 
/// Starts a server listening on:
/// - UDP: port 5000
/// - WebTransport: port 5001
/// - WebSocket: port 5002
pub fn run_server(enable_udp: bool, enable_webtransport: bool, enable_websocket: bool) {
    App::new()
        .add_plugins(DefaultPlugins.set(bevy::log::LogPlugin {
            level: bevy::log::Level::INFO,
            filter: "wgpu=error,naga=error,bevy_render=error,bevy_ecs=warn,bevy_app=warn,bevy_winit=warn,bevy_asset=warn".to_string(),
            ..default()
        }))
        .add_plugins(lightyear::prelude::server::ServerPlugins {
            tick_duration: Duration::from_secs_f64(1.0 / FIXED_TIMESTEP_HZ),
        })
        .insert_resource(server::ServerTransportConfig {
            enable_udp,
            enable_webtransport,
            enable_websocket,
        })
        .add_plugins(SharedPlugin)
        .add_plugins(server::ExampleServerPlugin)
        .add_plugins(renderer::ExampleRendererPlugin)
        .add_plugins(lightyear::prelude::DebugUIPlugin)
        .run();
}

/// Run a client connecting to the specified server
pub fn run_client(client_id: u64, server_addr: SocketAddr, transport: TransportType) {
    App::new()
        .add_plugins(DefaultPlugins.set(bevy::log::LogPlugin {
            level: bevy::log::Level::INFO,
            filter: "wgpu=error,naga=error,bevy_render=error,bevy_ecs=warn,bevy_app=warn,bevy_winit=warn,bevy_asset=warn".to_string(),
            ..default()
        }))
        .insert_resource(client::ClientConfig {
            client_id,
            transport,
            cert_digest: None,
            server_addr: Some(server_addr),
        })
        .add_plugins(lightyear::prelude::client::ClientPlugins {
            tick_duration: Duration::from_secs_f64(1.0 / FIXED_TIMESTEP_HZ),
        })
        .add_plugins(SharedPlugin)
        .add_plugins(client::ExampleClientPlugin)
        .add_plugins(renderer::ExampleRendererPlugin)
        .add_plugins(lightyear::prelude::DebugUIPlugin)
        .run();
}
