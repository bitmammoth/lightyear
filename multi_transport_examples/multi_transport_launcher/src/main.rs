//! Multi-Transport Example Launcher
//!
//! A GUI launcher for running multi-transport examples.
//! Supports launching servers with UDP, WebTransport, and WebSocket simultaneously,
//! and clients that connect via any single transport.

mod config;

use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiPrimaryContextPass};
use clap::Parser;
use strum::IntoEnumIterator;

use crate::config::*;
use std::{
    env, fs,
    io::Write,
    path::PathBuf,
    process::{exit, Command},
};

/// Command-line arguments
#[derive(Parser, Debug)]
#[command(author, version, about = "Multi-Transport Example Launcher")]
struct CliArgs {
    /// Path to the RON configuration file (for direct run mode)
    #[arg(long)]
    config_path: Option<PathBuf>,
}

fn main() {
    let cli_args = CliArgs::parse();

    // --- Direct Run Mode (using config file) ---
    if let Some(config_path) = cli_args.config_path {
        println!(
            "Direct run mode with config file: {:?}",
            config_path
        );

        // Load config from file
        let config = match fs::read_to_string(&config_path) {
            Ok(ron_data) => match ron::from_str::<LauncherConfig>(&ron_data) {
                Ok(cfg) => cfg,
                Err(e) => {
                    eprintln!("Failed to deserialize config from {:?}: {}", config_path, e);
                    exit(1);
                }
            },
            Err(e) => {
                eprintln!("Failed to read config file {:?}: {}", config_path, e);
                exit(1);
            }
        };
        
        // Delete the temp config file
        let _ = fs::remove_file(&config_path);

        println!("Loaded config: {:?}", config);

        // Build and run the appropriate app based on mode
        match config.mode {
            NetworkingMode::ClientOnly => {
                println!("Launching {} Client (ID: {}, Transport: {})...", 
                    config.example, config.client_id, config.client_transport);
                run_client_app(config);
            }
            NetworkingMode::ServerOnly => {
                println!("Launching {} Server...", config.example);
                run_server_app(config);
            }
            NetworkingMode::HostServer => {
                println!("Launching {} HostServer...", config.example);
                run_host_server_app(config);
            }
        }
        exit(0);
    }

    // --- UI Mode (Default) ---
    info!("Starting launcher in UI mode...");
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Multi-Transport Example Launcher".into(),
                        resolution: (600, 500).into(),
                        ..default()
                    }),
                    ..default()
                })
                .disable::<LogPlugin>(),
        )
        .add_plugins(EguiPlugin::default())
        .add_plugins(LogPlugin {
            level: bevy::log::Level::INFO,
            filter: "wgpu=error,bevy_render=info,bevy_ecs=warn,lightyear=info".to_string(),
            ..default()
        })
        .init_resource::<LauncherConfig>()
        .add_systems(Startup, setup_system)
        .add_systems(EguiPrimaryContextPass, ui_system)
        .run();
}

fn setup_system(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn ui_system(mut contexts: EguiContexts, mut config: ResMut<LauncherConfig>) -> Result {
    egui::CentralPanel::default().show(contexts.ctx_mut()?, |ui| {
        ui.heading("🚀 Multi-Transport Example Launcher");
        ui.separator();

        // === Example Selection ===
        ui.horizontal(|ui| {
            ui.label("Example:");
            egui::ComboBox::from_id_salt("example")
                .selected_text(config.example.to_string())
                .show_ui(ui, |ui| {
                    for example in Example::iter() {
                        ui.selectable_value(&mut config.example, example, example.to_string());
                    }
                });
        });

        ui.separator();

        // === Mode Selection ===
        ui.horizontal(|ui| {
            ui.label("Mode:");
            egui::ComboBox::from_id_salt("mode")
                .selected_text(config.mode.to_string())
                .show_ui(ui, |ui| {
                    for mode in NetworkingMode::iter() {
                        ui.selectable_value(&mut config.mode, mode, mode.to_string());
                    }
                });
        });

        ui.separator();

        // === Client Settings (if ClientOnly or HostServer) ===
        if config.mode == NetworkingMode::ClientOnly || config.mode == NetworkingMode::HostServer {
            ui.group(|ui| {
                ui.heading("Client Settings");

                // Client ID
                ui.horizontal(|ui| {
                    ui.label("Client ID:");
                    let mut id_str = config.client_id.to_string();
                    if ui.text_edit_singleline(&mut id_str).changed() {
                        if let Ok(id) = id_str.parse::<u64>() {
                            config.client_id = id;
                        }
                    }
                });

                // Server IP
                ui.horizontal(|ui| {
                    ui.label("Server IP:");
                    ui.text_edit_singleline(&mut config.server_ip);
                });

                // Transport selection
                ui.horizontal(|ui| {
                    ui.label("Transport:");
                    egui::ComboBox::from_id_salt("client_transport")
                        .selected_text(config.client_transport.to_string())
                        .show_ui(ui, |ui| {
                            for transport in ClientTransport::iter() {
                                ui.selectable_value(&mut config.client_transport, transport, transport.to_string());
                            }
                        });
                });

                // Show connection info
                let port = config.client_transport.default_port();
                ui.label(format!("Will connect to: {}:{}", config.server_ip, port));
            });
        }

        // === Server Settings (if ServerOnly or HostServer) ===
        if config.mode == NetworkingMode::ServerOnly || config.mode == NetworkingMode::HostServer {
            ui.group(|ui| {
                ui.heading("Server Settings (Multi-Transport)");
                ui.label("Enable transports:");

                ui.horizontal(|ui| {
                    ui.checkbox(&mut config.enable_udp, "UDP");
                    if config.enable_udp {
                        ui.label(format!("port {}", config.udp_port));
                    }
                });

                ui.horizontal(|ui| {
                    ui.checkbox(&mut config.enable_webtransport, "WebTransport");
                    if config.enable_webtransport {
                        ui.label(format!("port {}", config.webtransport_port));
                    }
                });

                ui.horizontal(|ui| {
                    ui.checkbox(&mut config.enable_websocket, "WebSocket");
                    if config.enable_websocket {
                        ui.label(format!("port {}", config.websocket_port));
                    }
                });

                if !config.enable_udp && !config.enable_webtransport && !config.enable_websocket {
                    ui.colored_label(egui::Color32::RED, "⚠ At least one transport required!");
                }
            });
        }

        ui.separator();

        // === Launch Button ===
        let launch_enabled = config.is_valid();
        
        ui.horizontal(|ui| {
            if ui.add_enabled(launch_enabled, egui::Button::new("🚀 Launch")).clicked() {
                info!("Launching with config: {:?}", *config);
                launch_app(&config);
            }

            if !launch_enabled {
                ui.colored_label(egui::Color32::YELLOW, "Please complete configuration");
            }
        });

        ui.separator();

        // === Quick Launch Buttons ===
        ui.heading("Quick Launch");
        
        ui.horizontal(|ui| {
            if ui.button("Launch Server").clicked() {
                let mut server_config = config.clone();
                server_config.mode = NetworkingMode::ServerOnly;
                launch_app(&server_config);
            }

            if ui.button("Launch UDP Client").clicked() {
                let mut client_config = config.clone();
                client_config.mode = NetworkingMode::ClientOnly;
                client_config.client_transport = ClientTransport::Udp;
                client_config.client_id = rand::random::<u64>() % 1000;
                launch_app(&client_config);
            }

            if ui.button("Launch WebSocket Client").clicked() {
                let mut client_config = config.clone();
                client_config.mode = NetworkingMode::ClientOnly;
                client_config.client_transport = ClientTransport::WebSocket;
                client_config.client_id = rand::random::<u64>() % 1000;
                launch_app(&client_config);
            }
        });

        // Info text
        ui.separator();
        ui.label("Multi-transport servers listen on multiple ports simultaneously:");
        ui.label("• UDP: 5000  • WebTransport: 5001  • WebSocket: 5002");
    });
    
    Ok(())
}

/// Run client app for the selected example
fn run_client_app(config: LauncherConfig) {
    match config.example {
        Example::SimpleBox => {
            multi_transport_simple_box::run_client(
                config.client_id,
                config.server_addr(),
                match config.client_transport {
                    ClientTransport::Udp => multi_transport_simple_box::TransportType::Udp,
                    ClientTransport::WebTransport => multi_transport_simple_box::TransportType::WebTransport,
                    ClientTransport::WebSocket => multi_transport_simple_box::TransportType::WebSocket,
                },
            );
        }
    }
}

/// Run server app for the selected example
fn run_server_app(config: LauncherConfig) {
    match config.example {
        Example::SimpleBox => {
            multi_transport_simple_box::run_server(
                config.enable_udp,
                config.enable_webtransport,
                config.enable_websocket,
            );
        }
    }
}

/// Run host server app (server + client in same process)
fn run_host_server_app(config: LauncherConfig) {
    // For host server, we run a server with a local client
    // For simplicity, just run as server - user can launch separate client
    warn!("HostServer mode: Running as server. Launch a separate client to connect.");
    run_server_app(config);
}

/// Launch the app in a new process
fn launch_app(config: &LauncherConfig) {
    let launch_config = config.clone();
    info!("Launching app with config: {:?}", launch_config);

    // Serialize the configuration
    let config_data = match ron::ser::to_string_pretty(&launch_config, ron::ser::PrettyConfig::default()) {
        Ok(data) => data,
        Err(e) => {
            error!("Failed to serialize config: {}", e);
            return;
        }
    };

    // Create a temporary file
    let mut temp_file = match tempfile::Builder::new()
        .prefix("multi_transport_launcher_cfg_")
        .suffix(".ron")
        .tempfile()
    {
        Ok(file) => file,
        Err(e) => {
            error!("Failed to create temp file: {}", e);
            return;
        }
    };

    // Write the config
    if let Err(e) = temp_file.write_all(config_data.as_bytes()) {
        error!("Failed to write config to temp file: {}", e);
        return;
    }

    // Persist the temp file
    let temp_path = temp_file.into_temp_path();

    // Get current executable path
    let current_exe = match env::current_exe() {
        Ok(path) => path,
        Err(e) => {
            error!("Failed to get current executable path: {}", e);
            return;
        }
    };

    // Spawn new process
    let mut command = Command::new(current_exe);
    command.arg("--config-path").arg(&*temp_path);

    info!("Spawning: {:?} --config-path {:?}", command.get_program(), &*temp_path);

    // Keep the temp file around for the child process
    temp_path.keep().expect("Failed to persist temp file");

    match command.spawn() {
        Ok(child) => {
            info!("Process spawned with PID: {:?}", child.id());
        }
        Err(e) => {
            error!("Failed to spawn process: {}", e);
        }
    }
}
