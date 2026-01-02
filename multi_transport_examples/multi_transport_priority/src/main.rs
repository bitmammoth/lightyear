//! Multi-Transport Priority Example
//!
//! Demonstrates:
//! - Priority-based replication with limited bandwidth
//! - Higher-priority entities get updated more frequently
//! - Multiple transports (UDP, WebTransport, WebSocket)
//!
//! A grid of shapes is spawned. Rows closer to center have lower priority (1.0),
//! rows further away have higher priority. When bandwidth is limited, higher-priority
//! rows get replicated more often.
//!
//! Usage:
//!   cargo run -p multi_transport_priority -- server
//!   cargo run -p multi_transport_priority -- client --transport udp
//!   cargo run -p multi_transport_priority -- client --transport websocket
//!   cargo run -p multi_transport_priority -- client --transport webtransport --cert <DIGEST>

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
#[command(name = "multi_transport_priority")]
#[command(about = "Multi-transport priority replication example")]
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
