//! Multi-transport FPS example demonstrating:
//! - Lag compensation for hit detection
//! - Leafwing input manager for player actions
//! - Avian2d physics for bullet movement
//! - Multi-transport support (UDP, WebTransport, WebSocket)
//!
//! Run with:
//! - `cargo run -p multi_transport_fps -- server`
//! - `cargo run -p multi_transport_fps -- client -c 1 --transport udp`
//! - `cargo run -p multi_transport_fps -- client -c 2 --transport web-transport`
//! - `cargo run -p multi_transport_fps -- client -c 3 --transport web-socket`

mod client;
mod protocol;
mod server;
mod shared;

#[cfg(feature = "gui")]
mod renderer;

use clap::{Parser, Subcommand, ValueEnum};
use std::net::Ipv4Addr;
use std::time::Duration;

use bevy::prelude::*;

pub const SERVER_ADDR: Ipv4Addr = Ipv4Addr::LOCALHOST;
pub const UDP_PORT: u16 = 5000;
pub const WEBTRANSPORT_PORT: u16 = 5001;
pub const WEBSOCKET_PORT: u16 = 5002;
pub const FIXED_TIMESTEP_HZ: f64 = 64.0;

#[derive(Parser)]
#[command(name = "multi_transport_fps")]
struct Cli {
    #[command(subcommand)]
    mode: Mode,
}

#[derive(Subcommand)]
enum Mode {
    /// Run as dedicated server (all transports)
    Server,
    /// Run as client
    Client {
        /// Client ID
        #[arg(short, long)]
        client_id: u64,
        /// Transport to use
        #[arg(short, long, default_value = "udp")]
        transport: TransportArg,
    },
}

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum TransportArg {
    Udp,
    WebTransport,
    WebSocket,
}

fn main() {
    let cli = Cli::parse();
    
    let mut app = App::new();
    
    app.add_plugins(DefaultPlugins);
    app.add_plugins(shared::SharedPlugin);
    
    match cli.mode {
        Mode::Server => {
            app.add_plugins(lightyear::prelude::server::ServerPlugins {
                tick_duration: Duration::from_secs_f64(1.0 / FIXED_TIMESTEP_HZ),
            });
            app.add_plugins(server::ServerPlugin);
        }
        Mode::Client { client_id, transport } => {
            app.add_plugins(lightyear::prelude::client::ClientPlugins {
                tick_duration: Duration::from_secs_f64(1.0 / FIXED_TIMESTEP_HZ),
            });
            app.add_plugins(client::ClientPlugin);
            
            // Connect client to dedicated server
            client::connect_client(&mut app, client_id, transport);
        }
    }
    
    #[cfg(feature = "gui")]
    app.add_plugins(renderer::RendererPlugin);
    
    app.run();
}
