//! Multi-Transport Simple Setup Example
//!
//! This minimal example showcases how to setup the lightyear plugins with multiple transports.
//!
//! Run with:
//! - `cargo run -p simple_setup -- server`
//! - `cargo run -p simple_setup -- client -t udp`
//! - `cargo run -p simple_setup -- client -t websocket`
//! - `cargo run -p simple_setup -- client -t webtransport -c <CERT_DIGEST>`
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]

mod client;
mod server;
mod shared;

use crate::shared::{SharedPlugin, FIXED_TIMESTEP_HZ};
use bevy::prelude::*;
use clap::{Parser, Subcommand, ValueEnum};
use core::time::Duration;
use lightyear::prelude::client::ClientPlugins;
use lightyear::prelude::server::ServerPlugins;

#[derive(Parser, Debug)]
#[command(version, about = "Multi-transport simple setup example")]
pub struct Cli {
    #[command(subcommand)]
    pub mode: Mode,
}

#[derive(Subcommand, Debug)]
pub enum Mode {
    /// Run as server (UDP + WebTransport + WebSocket)
    Server,
    /// Run as client
    Client {
        /// Transport to use
        #[arg(short, long, default_value = "udp")]
        transport: TransportArg,
        /// Certificate digest (required for WebTransport)
        #[arg(short, long)]
        cert: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum TransportArg {
    Udp,
    Webtransport,
    Websocket,
}

fn main() {
    let cli = Cli::parse();
    let mut app = App::new();

    match cli.mode {
        Mode::Client { transport, cert } => {
            app.add_plugins(DefaultPlugins);
            app.add_plugins(ClientPlugins {
                tick_duration: Duration::from_secs_f64(1.0 / FIXED_TIMESTEP_HZ),
            });
            app.add_plugins(SharedPlugin);
            
            // Configure which transport to use
            let transport_config = match transport {
                TransportArg::Udp => client::Transport::Udp,
                TransportArg::Webtransport => client::Transport::WebTransport,
                TransportArg::Websocket => client::Transport::WebSocket,
            };
            app.insert_resource(client::ClientConfig {
                client_id: rand::random::<u64>(),
                transport: transport_config,
                cert_digest: cert,
            });
            
            app.add_plugins(client::ExampleClientPlugin);
        }
        Mode::Server => {
            app.add_plugins(DefaultPlugins);
            app.add_plugins(ServerPlugins {
                tick_duration: Duration::from_secs_f64(1.0 / FIXED_TIMESTEP_HZ),
            });
            app.add_plugins(SharedPlugin);
            app.add_plugins(server::ExampleServerPlugin);
        }
    }
    app.run();
}
