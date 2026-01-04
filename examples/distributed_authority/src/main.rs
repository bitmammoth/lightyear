//! Multi-Transport Distributed Authority Example
//!
//! This example demonstrates:
//! - Server running UDP, WebTransport, and WebSocket transports
//! - Dynamic authority transfer between server and clients
//! - A ball entity whose authority transfers to the nearest player
//! - Bidirectional entity replication (client-to-server when having authority)
//!
//! The ball bounces vertically, and when a player is close, authority transfers
//! to that player, who then controls the ball. Other players see the updates
//! interpolated.
//!
//! Usage:
//!   # Start the server (runs UDP:5000, WebTransport:5001, WebSocket:5002)
//!   cargo run -p distributed_authority -- server
//!
//!   # Connect via UDP (in another terminal)
//!   cargo run -p distributed_authority -- client --transport udp
//!
//!   # Connect via WebTransport (copy cert digest from server output)
//!   cargo run -p distributed_authority -- client --transport webtransport --cert <DIGEST>
//!
//!   # Connect via WebSocket
//!   cargo run -p distributed_authority -- client --transport websocket

use bevy::prelude::*;
use clap::{Parser, Subcommand, ValueEnum};
use core::time::Duration;
use lightyear::prelude::DebugUIPlugin;

mod client;
mod protocol;
mod renderer;
mod server;
mod shared;

use shared::*;

#[derive(Parser)]
#[command(name = "distributed_authority")]
#[command(about = "Multi-transport distributed authority example")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run as server (UDP + WebTransport + WebSocket)
    Server,
    /// Run as client
    Client {
        /// Transport to use for connection
        #[arg(short, long, default_value = "udp")]
        transport: TransportArg,
        /// Certificate digest (required for WebTransport)
        #[arg(short, long)]
        cert: Option<String>,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum)]
enum TransportArg {
    Udp,
    Webtransport,
    Websocket,
}

fn main() {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Server => run_server(),
        Commands::Client { transport, cert } => run_client(transport, cert),
    }
}

fn run_server() {
    App::new()
        .add_plugins(DefaultPlugins.set(bevy::log::LogPlugin {
            level: bevy::log::Level::INFO,
            filter: "wgpu=error,naga=error,bevy_render=error,bevy_ecs=warn,bevy_app=warn,bevy_winit=warn,bevy_asset=warn".to_string(),
            ..default()
        }))
        .add_plugins(lightyear::prelude::server::ServerPlugins {
            tick_duration: Duration::from_secs_f64(1.0 / FIXED_TIMESTEP_HZ),
        })
        .add_plugins(SharedPlugin)
        .add_plugins(server::ExampleServerPlugin)
        .add_plugins(renderer::ExampleRendererPlugin)
        .add_plugins(DebugUIPlugin)
        .run();
}

fn run_client(transport: TransportArg, cert: Option<String>) {
    // Generate a random client ID
    let client_id: u64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64 % 100000;
    
    let transport_enum = match transport {
        TransportArg::Udp => client::Transport::Udp,
        TransportArg::Webtransport => client::Transport::WebTransport,
        TransportArg::Websocket => client::Transport::WebSocket,
    };
    
    App::new()
        .add_plugins(DefaultPlugins.set(bevy::log::LogPlugin {
            level: bevy::log::Level::INFO,
            filter: "wgpu=error,naga=error,bevy_render=error,bevy_ecs=warn,bevy_app=warn,bevy_winit=warn,bevy_asset=warn".to_string(),
            ..default()
        }))
        .insert_resource(client::ClientConfig {
            client_id,
            transport: transport_enum,
            cert_digest: cert,
        })
        .add_plugins(lightyear::prelude::client::ClientPlugins {
            tick_duration: Duration::from_secs_f64(1.0 / FIXED_TIMESTEP_HZ),
        })
        .add_plugins(SharedPlugin)
        .add_plugins(client::ExampleClientPlugin)
        .add_plugins(renderer::ExampleRendererPlugin)
        .add_plugins(DebugUIPlugin)
        .run();
}
