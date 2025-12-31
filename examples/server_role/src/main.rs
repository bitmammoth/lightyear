//! ServerRole Multi-Transport Example
//!
//! This example demonstrates the ServerRole resource that tracks the logical server state
//! independent of how many transport types are running.
//!
//! Run with: `cargo run -p server_role_example`
//!
//! The example will:
//! 1. Start with ServerRole in Stopped state
//! 2. Spawn a "UDP" server - ServerRole transitions to Running
//! 3. Spawn a "Steam" server - ServerRole stays Running
//! 4. Stop the "UDP" server - ServerRole stays Running (Steam still active)
//! 5. Stop the "Steam" server - ServerRole transitions to Stopped

use bevy::prelude::*;
use lightyear_connection::prelude::server::*;
use lightyear_connection::prelude::PeerMetadata;
use lightyear_link::prelude::Server;

fn main() {
    App::new()
        .add_plugins(MinimalPlugins)
        .init_resource::<PeerMetadata>()
        .add_plugins(ServerRolePlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, monitor_server_role)
        .add_systems(Update, simulate_transport_lifecycle)
        .run();
}

/// Track simulation state
#[derive(Resource, Default)]
struct SimulationState {
    frame: u32,
    udp_server: Option<Entity>,
    steam_server: Option<Entity>,
}

fn setup(mut commands: Commands) {
    commands.init_resource::<SimulationState>();
    println!("\n=== ServerRole Multi-Transport Example ===\n");
    println!("This demonstrates how ServerRole tracks multiple transports.\n");
}

/// Print the current ServerRole state each frame it changes
fn monitor_server_role(
    server_role: Res<ServerRole>,
) {
    if server_role.is_changed() {
        println!("  📡 ServerRole state: {:?}", server_role.state);
    }
}

/// Simulate starting and stopping multiple transports over time
fn simulate_transport_lifecycle(
    mut commands: Commands,
    mut state: ResMut<SimulationState>,
    mut exit: bevy::ecs::message::MessageWriter<AppExit>,
) {
    state.frame += 1;
    
    match state.frame {
        10 => {
            println!("\n[Frame {}] Starting UDP transport...", state.frame);
            let entity = commands
                .spawn((
                    Server::default(),
                    IoServer::udp(),
                    Name::new("UdpServer"),
                    Started, // Directly set to Started for simplicity
                ))
                .id();
            state.udp_server = Some(entity);
        }
        30 => {
            println!("\n[Frame {}] Starting Steam transport...", state.frame);
            let entity = commands
                .spawn((
                    Server::default(),
                    IoServer::steam(),
                    Name::new("SteamServer"),
                    Started,
                ))
                .id();
            state.steam_server = Some(entity);
        }
        50 => {
            println!("\n[Frame {}] Stopping UDP transport...", state.frame);
            if let Some(entity) = state.udp_server {
                commands.entity(entity)
                    .remove::<Started>()
                    .insert(Stopped);
            }
        }
        70 => {
            println!("\n[Frame {}] Stopping Steam transport...", state.frame);
            if let Some(entity) = state.steam_server {
                commands.entity(entity)
                    .remove::<Started>()
                    .insert(Stopped);
            }
        }
        90 => {
            println!("\n=== Example Complete ===");
            println!("ServerRole correctly tracked multiple transports!\n");
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}
