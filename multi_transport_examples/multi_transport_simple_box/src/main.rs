//! Multi-Transport Entity Replication Example
//!
//! Demonstrates:
//! - Server running UDP, WebTransport, and WebSocket transports simultaneously
//! - Server spawning player entities when clients connect
//! - Entity replication to all connected clients
//! - Clients can connect via any transport
//! - Host-client mode (server + client in same app)
//!
//! Usage:
//!   # Start the server (runs UDP:5000, WebTransport:5001, WebSocket:5002)
//!   cargo run -p multi_transport_simple_box -- server
//!
//!   # Connect via UDP (in another terminal)
//!   cargo run -p multi_transport_simple_box -- client --transport udp
//!
//!   # Connect via WebTransport
//!   cargo run -p multi_transport_simple_box -- client --transport webtransport
//!
//!   # Connect via WebSocket
//!   cargo run -p multi_transport_simple_box -- client --transport websocket
//!
//!   # Host-client mode (server + local client in same app)
//!   cargo run -p multi_transport_simple_box -- host-client

use bevy::prelude::*;
use clap::{Parser, Subcommand, ValueEnum};
use core::time::Duration;
use lightyear::prelude::DebugUIPlugin;

mod client;
mod renderer;
mod server;
mod shared;

pub use client::{ExampleClientPlugin, ClientConfig, Transport};
pub use server::{ExampleServerPlugin, ServerTransportConfig};
pub use shared::*;

// Re-alias to avoid confusion with lightyear's ClientConfig
use client::ClientConfig as MyClientConfig;
use client::Transport as MyTransport;

#[derive(Parser)]
#[command(name = "multi_transport_simple_box")]
#[command(about = "Multi-transport entity replication example")]
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
        /// Client ID (optional, random if not specified)
        #[arg(short = 'i', long)]
        client_id: Option<u64>,
    },
    /// Run as host-client (server + client in same app, client uses local channels)
    HostClient {
        /// Client ID (optional, defaults to 0)
        #[arg(short, long, default_value = "0")]
        client_id: u64,
    },
    /// Run server and client in separate threads within same process
    Separate {
        /// Transport for the client to use
        #[arg(short, long, default_value = "udp")]
        transport: TransportArg,
        /// Client ID (optional, random if not specified)
        #[arg(short = 'i', long)]
        client_id: Option<u64>,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum)]
pub enum TransportArg {
    Udp,
    Webtransport,
    Websocket,
}

fn main() {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Server => run_server(),
        Commands::Client { transport, cert, client_id } => run_client(transport, cert, client_id),
        Commands::HostClient { client_id } => run_host_client(client_id),
        Commands::Separate { transport, client_id } => run_separate(transport, client_id),
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
        .add_systems(Startup, server::startup)  // Add server startup system
        .add_plugins(renderer::ExampleRendererPlugin)
        .add_plugins(DebugUIPlugin)
        .run();
}

fn run_client(transport: TransportArg, cert: Option<String>, client_id: Option<u64>) {
    // Generate a random client ID if not specified
    let client_id = client_id.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64 % 100000
    });
    
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
        .insert_resource(MyClientConfig {
            client_id,
            transport: transport_enum,
            cert_digest: cert,
            server_addr: None,
        })
        .add_plugins(lightyear::prelude::client::ClientPlugins {
            tick_duration: Duration::from_secs_f64(1.0 / FIXED_TIMESTEP_HZ),
        })
        .add_plugins(SharedPlugin)
        .add_plugins(client::ExampleClientPlugin)
        .add_systems(Startup, client::startup)  // Add client startup system
        .add_plugins(renderer::ExampleRendererPlugin)
        .add_plugins(DebugUIPlugin)
        .run();
}

/// Host-client mode: server + client in same app, client connects via local channels
fn run_host_client(client_id: u64) {
    let mut app = App::new();
    
    app.add_plugins(DefaultPlugins.set(bevy::log::LogPlugin {
        level: bevy::log::Level::INFO,
        filter: "wgpu=error,naga=error,bevy_render=error,bevy_ecs=warn,bevy_app=warn,bevy_winit=warn,bevy_asset=warn".to_string(),
        ..default()
    }));
    
    // Add both client and server plugins
    app.add_plugins(lightyear::prelude::client::ClientPlugins {
        tick_duration: Duration::from_secs_f64(1.0 / FIXED_TIMESTEP_HZ),
    });
    app.add_plugins(lightyear::prelude::server::ServerPlugins {
        tick_duration: Duration::from_secs_f64(1.0 / FIXED_TIMESTEP_HZ),
    });
    
    app.add_plugins(SharedPlugin);
    app.add_plugins(server::ExampleServerPlugin);
    app.add_plugins(client::ExampleClientPlugin);
    
    // Add both startup systems - server runs first, then client (which needs the Server entity)
    app.add_systems(Startup, (server::startup, client::startup.after(server::startup)));
    
    app.add_plugins(renderer::ExampleRendererPlugin);
    app.add_plugins(DebugUIPlugin);
    
    // Insert host-client config using our custom ClientConfig type
    app.insert_resource(MyClientConfig {
        client_id,
        transport: MyTransport::Local,
        cert_digest: None,
        server_addr: None,
    });
    
    app.run();
}

/// Separate mode: server and client in separate threads
fn run_separate(transport: TransportArg, client_id: Option<u64>) {
    use std::thread;
    
    let client_id = client_id.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64 % 100000
    });
    
    // Start server in a separate thread
    let server_thread = thread::spawn(|| {
        run_server();
    });
    
    // Give server time to start
    thread::sleep(Duration::from_millis(500));
    
    // Run client in main thread
    run_client(transport, None, Some(client_id));
    
    // Wait for server thread (this won't actually happen since client blocks)
    let _ = server_thread.join();
}
