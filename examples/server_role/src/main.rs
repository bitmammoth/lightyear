//! ServerRole Multi-Transport Example
//!
//! This example demonstrates running a server with multiple transports (UDP + WebTransport)
//! and having clients connect via either transport.
//!
//! The ServerRole resource tracks the overall server state independent of how many
//! transport types are running.
//!
//! Run with:
//!   Server: `cargo run -p server_role_example -- server`
//!   UDP Client: `cargo run -p server_role_example -- client --transport udp`
//!   WebTransport Client: `cargo run -p server_role_example -- client --transport webtransport --cert-digest <DIGEST>`
//!
//! The server will print the certificate digest on startup - use that for WebTransport clients.

mod client;
mod server;
mod shared;

use crate::client::{ClientConfig, Transport};
use crate::shared::{SharedPlugin, FIXED_TIMESTEP_HZ};
use bevy::prelude::*;
use clap::{Parser, Subcommand};
use core::time::Duration;
use lightyear::prelude::client::ClientPlugins;
use lightyear::prelude::server::ServerPlugins;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Parser, Debug)]
#[command(version, about = "ServerRole Multi-Transport Example")]
pub struct Cli {
    #[command(subcommand)]
    pub mode: Mode,
}

#[derive(Subcommand, Debug)]
pub enum Mode {
    /// Run as server with UDP + WebTransport
    Server,
    /// Run as client
    Client {
        /// Transport type: udp or webtransport
        #[arg(short, long, default_value = "udp")]
        transport: String,
        /// Certificate digest for WebTransport (required for webtransport)
        #[arg(short, long)]
        cert_digest: Option<String>,
        /// Client ID (auto-generated if not specified)
        #[arg(long)]
        client_id: Option<u64>,
    },
}

fn main() {
    let cli = Cli::parse();
    let mut app = App::new();

    match cli.mode {
        Mode::Server => {
            app.add_plugins(MinimalPlugins);
            app.add_plugins(bevy::log::LogPlugin::default());
            app.add_plugins(ServerPlugins {
                tick_duration: Duration::from_secs_f64(1.0 / FIXED_TIMESTEP_HZ),
            });
            app.add_plugins(SharedPlugin);
            app.add_plugins(server::ExampleServerPlugin);
        }
        Mode::Client { transport, cert_digest, client_id } => {
            let transport = match transport.to_lowercase().as_str() {
                "webtransport" | "wt" => Transport::WebTransport,
                _ => Transport::Udp,
            };
            
            let client_id = client_id.unwrap_or_else(|| {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64 % 10000
            });
            
            info!("=== Client {} connecting via {:?} ===", client_id, transport);
            
            app.add_plugins(MinimalPlugins);
            app.add_plugins(bevy::log::LogPlugin::default());
            app.add_plugins(ClientPlugins {
                tick_duration: Duration::from_secs_f64(1.0 / FIXED_TIMESTEP_HZ),
            });
            app.add_plugins(SharedPlugin);
            app.insert_resource(ClientConfig {
                transport,
                client_id,
                cert_digest,
            });
            app.add_plugins(client::ExampleClientPlugin);
        }
    }
    
    app.run();
}
