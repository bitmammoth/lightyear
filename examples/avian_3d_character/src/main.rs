//! Multi-Transport Avian 3D Character Example
//!
//! Demonstrates 3D character physics with avian3d across multiple transports.
//! Players control capsule characters in a 3D arena with jumping and movement.
//!
//! Run with:
//! - `cargo run -- server`
//! - `cargo run -- client --transport udp`
//! - `cargo run -- client --transport webtransport --cert <DIGEST>`
//! - `cargo run -- client --transport websocket`

#![allow(unused_imports)]
#![allow(dead_code)]

use bevy::prelude::*;
use clap::{Parser, Subcommand, ValueEnum};

#[cfg(feature = "client")]
mod client;
mod protocol;
mod server;
mod shared;

#[cfg(feature = "client")]
use client::run_client;
use server::run_server;

#[derive(Parser)]
#[command(name = "avian_3d_character")]
#[command(about = "Multi-transport 3D character physics demo with avian3d")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the server (listens on UDP:5000, WebTransport:5001, WebSocket:5002)
    Server,
    /// Run a client
    Client {
        /// Transport protocol to use
        #[arg(short, long, default_value = "udp")]
        transport: TransportArg,
        /// Certificate digest (required for WebTransport)
        #[arg(short, long)]
        cert: Option<String>,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum, Default)]
pub enum TransportArg {
    #[default]
    Udp,
    Webtransport,
    Websocket,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Server => {
            run_server();
        }
        #[cfg(feature = "client")]
        Commands::Client { transport, cert } => {
            run_client(transport, cert);
        }
        #[cfg(not(feature = "client"))]
        Commands::Client { .. } => {
            eprintln!("Client feature not enabled. Compile with --features client");
        }
    }
}
