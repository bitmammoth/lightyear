//! Multi-Transport Entity Replication Example
//!
//! Demonstrates:
//! - Server running both UDP and WebTransport transports
//! - Server spawning player entities when clients connect
//! - Entity replication to all connected clients
//! - Clients can connect via either transport
//!
//! Usage:
//!   # Start the server (runs both UDP:5000 and WebTransport:5001)
//!   cargo run -p multi_transport_example -- server
//!
//!   # Connect via UDP (in another terminal)
//!   cargo run -p multi_transport_example -- client --transport udp
//!
//!   # Connect via WebTransport (copy cert digest from server output)
//!   cargo run -p multi_transport_example -- client --transport webtransport --cert <DIGEST>

use bevy::prelude::*;
use clap::{Parser, Subcommand, ValueEnum};
use core::time::Duration;

mod client;
mod server;
mod shared;

use shared::*;

#[derive(Parser)]
#[command(name = "multi_transport_example")]
#[command(about = "Multi-transport entity replication example")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run as server (UDP + WebTransport)
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
        .add_plugins(client::ExampleClientPlugin)
        .run();
}
