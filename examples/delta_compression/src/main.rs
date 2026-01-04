//! Multi-Transport Delta Compression Example
//!
//! Demonstrates:
//! - Delta compression for bandwidth-efficient replication
//! - Instead of sending full Vec2 (8 bytes), sends delta as (i8, i8) (2 bytes)
//! - Multiple transports (UDP, WebTransport, WebSocket)
//!
//! Usage:
//!   cargo run -p delta_compression -- server
//!   cargo run -p delta_compression -- client --transport udp
//!   cargo run -p delta_compression -- client --transport websocket
//!   cargo run -p delta_compression -- client --transport webtransport --cert <DIGEST>

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
#[command(name = "delta_compression")]
#[command(about = "Multi-transport delta compression example")]
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
