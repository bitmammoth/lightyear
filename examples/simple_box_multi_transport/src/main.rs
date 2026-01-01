//! Simple Box Multi-Transport Example
//!
//! Demonstrates:
//! - Server running both UDP and WebTransport transports
//! - Player spawning with movement via prediction/interpolation
//! - Entity replication to all connected clients regardless of transport
//! - Input handling with ActionState
//!
//! Usage:
//!   # Start the server (runs both UDP:5000 and WebTransport:5001)
//!   cargo run -p simple_box_multi_transport -- server
//!
//!   # Connect via UDP (in another terminal)
//!   cargo run -p simple_box_multi_transport -- client --transport udp
//!
//!   # Connect via WebTransport (copy cert digest from server output)
//!   cargo run -p simple_box_multi_transport -- client --transport webtransport --cert <DIGEST>
//!
//!   Move with WASD or arrow keys!

use bevy::prelude::*;
use clap::{Parser, Subcommand, ValueEnum};
use core::time::Duration;

mod client;
mod protocol;
mod renderer;
mod server;
mod shared;

use shared::SharedPlugin;

pub const FIXED_TIMESTEP_HZ: f64 = 64.0;

#[derive(Parser)]
#[command(name = "simple_box_multi")]
#[command(about = "Simple box multi-transport example with movement")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run as server (starts both UDP:5000 and WebTransport:5001)
    Server,
    /// Run as client
    Client {
        /// Transport to use for connection
        #[arg(short, long, default_value = "udp")]
        transport: TransportArg,
        /// Certificate digest (required for WebTransport with self-signed certs)
        #[arg(short, long)]
        cert: Option<String>,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum)]
enum TransportArg {
    Udp,
    Webtransport,
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
        .add_plugins(server::ServerPlugin)
        .add_plugins(renderer::RendererPlugin { is_server: true })
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
        .add_plugins(client::ClientPlugin)
        .add_plugins(renderer::RendererPlugin { is_server: false })
        .run();
}
