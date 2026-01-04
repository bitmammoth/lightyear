//! Multi-Transport Spaceships Demo
//!
//! This is based on the demos/spaceships example but supports multiple transports.
//!
//! Usage:
//!   # Start the server (runs UDP:5000, WebTransport:5001, WebSocket:5002)
//!   cargo run -p multi_transport_spaceships -- server
//!
//!   # Connect via UDP (in another terminal)
//!   cargo run -p multi_transport_spaceships -- client --transport udp
//!
//!   # Connect via WebTransport (copy cert digest from server output)
//!   cargo run -p multi_transport_spaceships -- client --transport webtransport --cert <DIGEST>
//!
//!   # Connect via WebSocket
//!   cargo run -p multi_transport_spaceships -- client --transport websocket
//!
//!   # Host-client mode with transport selection
//!   cargo run -p multi_transport_spaceships -- host-client --transport udp

#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
use bevy::prelude::*;
use clap::{Parser, Subcommand, ValueEnum};
use core::time::Duration;
use lightyear::prelude::*;

#[cfg(feature = "client")]
use crate::client::ExampleClientPlugin;
#[cfg(feature = "server")]
use crate::server::ExampleServerPlugin;
use crate::shared::SharedPlugin;
use crate::shared::FIXED_TIMESTEP_HZ;

#[cfg(feature = "client")]
mod client;
mod protocol;

#[cfg(feature = "gui")]
mod entity_label;
#[cfg(feature = "gui")]
mod renderer;
#[cfg(feature = "server")]
mod server;
mod shared;

#[derive(Parser)]
#[command(name = "multi_transport_spaceships")]
#[command(about = "Multi-transport spaceships demo")]
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
    /// Run as host-client (server + client)
    #[cfg(all(feature = "client", feature = "server"))]
    HostClient {
        /// Transport to use for the client connection
        #[arg(short, long, default_value = "udp")]
        transport: TransportArg,
        /// Certificate digest (required for WebTransport)
        #[arg(short, long)]
        cert: Option<String>,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum)]
pub enum TransportArg {
    Udp,
    Webtransport,
    Websocket,
    /// Local connection (for host-client mode)
    Local,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Server => run_server(),
        #[cfg(feature = "client")]
        Commands::Client { transport, cert } => run_client(transport, cert),
        #[cfg(all(feature = "client", feature = "server"))]
        Commands::HostClient { transport, cert } => run_host_client(transport, cert),
    }
}

#[cfg(feature = "server")]
fn run_server() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(bevy::log::LogPlugin {
            level: bevy::log::Level::INFO,
            filter: "wgpu=error,naga=error,bevy_render=error,bevy_ecs=warn,bevy_app=warn,bevy_winit=warn,bevy_asset=warn".to_string(),
            ..default()
        }))
        .add_plugins(lightyear::prelude::server::ServerPlugins {
            tick_duration: Duration::from_secs_f64(1.0 / FIXED_TIMESTEP_HZ),
        })
        .add_plugins(SharedPlugin {
            show_confirmed: false,
        })
        .add_plugins(ExampleServerPlugin);
    
    #[cfg(feature = "gui")]
    app.add_plugins(renderer::ExampleRendererPlugin);
    
    app.run();
}

#[cfg(feature = "client")]
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
        TransportArg::Local => client::Transport::Local,
    };

    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(bevy::log::LogPlugin {
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
    .add_plugins(SharedPlugin {
        show_confirmed: false,
    })
    .add_plugins(ExampleClientPlugin);

    #[cfg(feature = "gui")]
    app.add_plugins(renderer::ExampleRendererPlugin);

    // Add observer to set up input delay when the Client entity is created
    app.add_observer(setup_input_delay);

    app.run();
}

#[cfg(all(feature = "client", feature = "server"))]
fn run_host_client(transport: TransportArg, cert: Option<String>) {
    // Generate a random client ID
    let client_id: u64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64 % 100000;

    let transport_enum = match transport {
        TransportArg::Udp => client::Transport::Udp,
        TransportArg::Webtransport => client::Transport::WebTransport,
        TransportArg::Websocket => client::Transport::WebSocket,
        TransportArg::Local => client::Transport::Local,
    };

    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(bevy::log::LogPlugin {
        level: bevy::log::Level::INFO,
        filter: "wgpu=error,naga=error,bevy_render=error,bevy_ecs=warn,bevy_app=warn,bevy_winit=warn,bevy_asset=warn".to_string(),
        ..default()
    }))
    .insert_resource(client::ClientConfig {
        client_id,
        transport: transport_enum,
        cert_digest: cert,
    })
    // Server plugins
    .add_plugins(lightyear::prelude::server::ServerPlugins {
        tick_duration: Duration::from_secs_f64(1.0 / FIXED_TIMESTEP_HZ),
    })
    // Client plugins
    .add_plugins(lightyear::prelude::client::ClientPlugins {
        tick_duration: Duration::from_secs_f64(1.0 / FIXED_TIMESTEP_HZ),
    })
    .add_plugins(SharedPlugin {
        show_confirmed: false,
    })
    .add_plugins(ExampleServerPlugin)
    .add_plugins(ExampleClientPlugin);

    #[cfg(feature = "gui")]
    app.add_plugins(renderer::ExampleRendererPlugin);

    // Add observer to set up input delay when the Client entity is created
    app.add_observer(setup_input_delay);

    app.run();
}

/// Observer to add input delay configuration when a Client entity is spawned
#[cfg(feature = "client")]
fn setup_input_delay(
    trigger: On<Add, lightyear::prelude::Client>,
    mut commands: bevy::ecs::system::Commands,
) {
    use lightyear::prelude::client::InputDelayConfig;

    // set some input-delay since we are predicting all entities
    commands.entity(trigger.entity).insert(
        InputTimelineConfig::default().with_input_delay(InputDelayConfig::fixed_input_delay(10)),
    );
}
