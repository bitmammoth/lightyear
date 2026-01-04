//! Client-side code for the lobby example with multi-transport support

use std::net::{Ipv4Addr, SocketAddr};

use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use egui_extras::{Column, TableBuilder};

use crate::protocol::*;
use crate::shared::shared_movement_behaviour;
use crate::{TransportArg, UDP_PORT, WEBTRANSPORT_PORT, WEBSOCKET_PORT, HOST_SERVER_PORT, SERVER_ADDR};
use lightyear::connection::client::ClientState;
use lightyear::input::client::InputSystems;
use lightyear::input::native::prelude::{ActionState, InputMarker};
use lightyear::netcode::Key;
use lightyear::prelude::client::*;
use lightyear::prelude::*;
use lightyear::websocket::prelude::client::ClientConfig as WebSocketClientConfig;

const PROTOCOL_ID: u64 = 0;
const PRIVATE_KEY: Key = [0; 32];

pub struct ClientPlugin {
    pub transport: TransportArg,
}

impl Plugin for ClientPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LobbyTable>();
        app.init_state::<AppState>();
        
        app.add_systems(
            FixedPreUpdate,
            buffer_input
                .in_set(InputSystems::WriteClientInputs)
                .run_if(in_state(AppState::Game)),
        );
        app.add_systems(
            FixedUpdate,
            player_movement.run_if(in_state(AppState::Game)),
        );
        app.add_systems(Update, debug_input_state.run_if(in_state(AppState::Game)));
        app.add_systems(EguiPrimaryContextPass, lobby_ui);
        app.add_systems(
            PreUpdate,
            receive_start_game_message.after(MessageSystems::Receive),
        );
        app.add_observer(on_disconnect);
        app.add_observer(handle_predicted_spawn);
        app.add_observer(handle_interpolated_spawn);
    }
}

/// Connect client to the dedicated server
pub fn connect_client(app: &mut App, client_id: u64, transport: TransportArg) {
    let world = app.world_mut();
    
    let client_port = 4000 + (client_id % 100) as u16;
    let client_addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), client_port);
    
    let (server_addr, _port) = match transport {
        TransportArg::Udp => (SocketAddr::new(SERVER_ADDR.into(), UDP_PORT), UDP_PORT),
        TransportArg::WebTransport => (SocketAddr::new(SERVER_ADDR.into(), WEBTRANSPORT_PORT), WEBTRANSPORT_PORT),
        TransportArg::WebSocket => (SocketAddr::new(SERVER_ADDR.into(), WEBSOCKET_PORT), WEBSOCKET_PORT),
    };
    
    let auth = Authentication::Manual {
        server_addr,
        client_id,
        private_key: PRIVATE_KEY,
        protocol_id: PROTOCOL_ID,
    };
    
    // Create client entity with transport-specific components
    let client = match transport {
        TransportArg::Udp => {
            info!("Client {} connecting via UDP to {}", client_id, server_addr);
            world.spawn((
                Client::default(),
                LocalAddr(client_addr),
                PeerAddr(server_addr),
                Link::new(None),
                ReplicationReceiver::default(),
                NetcodeClient::new(auth, NetcodeConfig::default()).unwrap(),
                UdpIo::default(),
                LocalId(PeerId::Netcode(client_id)),
                Name::new("UdpClient"),
            )).id()
        }
        TransportArg::WebTransport => {
            info!("Client {} connecting via WebTransport to {}", client_id, server_addr);
            world.spawn((
                Client::default(),
                LocalAddr(client_addr),
                PeerAddr(server_addr),
                Link::new(None),
                ReplicationReceiver::default(),
                NetcodeClient::new(auth, NetcodeConfig::default()).unwrap(),
                WebTransportClientIo {
                    // Use empty string to allow self-signed certs in development
                    certificate_digest: String::new(),
                },
                LocalId(PeerId::Netcode(client_id)),
                Name::new("WebTransportClient"),
            )).id()
        }
        TransportArg::WebSocket => {
            info!("Client {} connecting via WebSocket to {}", client_id, server_addr);
            world.spawn((
                Client::default(),
                LocalAddr(client_addr),
                PeerAddr(server_addr),
                Link::new(None),
                ReplicationReceiver::default(),
                NetcodeClient::new(auth, NetcodeConfig::default()).unwrap(),
                WebSocketClientIo {
                    config: WebSocketClientConfig::builder()
                        .with_no_cert_validation(),
                    scheme: WebSocketScheme::Secure,
                },
                LocalId(PeerId::Netcode(client_id)),
                Name::new("WebSocketClient"),
            )).id()
        }
    };
    
    // Store transport type for reconnection
    world.insert_resource(CurrentTransport(transport));
    
    // Connect
    world.trigger(Connect { entity: client });
    info!("Client {} connection initiated", client_id);
}

#[derive(Resource)]
struct CurrentTransport(TransportArg);

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    Lobby,
    LobbyJoined { lobby_id: usize },
    Game,
}

#[derive(Resource, Default, Debug)]
pub struct LobbyTable {
    clients: HashMap<PeerId, bool>,
}

impl LobbyTable {
    pub fn get_host(&self) -> Option<PeerId> {
        self.clients
            .iter()
            .find_map(|(client_id, is_host)| if *is_host { Some(*client_id) } else { None })
    }
}

fn on_disconnect(
    trigger: On<Add, Disconnected>,
    local_id: Option<Single<&LocalId>>,
    entities: Query<Entity, Or<(With<Lobbies>, With<PlayerId>)>>,
    mut commands: Commands,
    current_transport: Option<Res<CurrentTransport>>,
) {
    // Despawn game entities
    for entity in entities.iter() {
        commands.entity(entity).despawn();
    }
    
    // Stop server if running as host
    commands.trigger(lightyear::connection::server::Stop {
        entity: trigger.entity,
    });
    
    // Reset netcode config to connect to lobby server
    let Some(local_id) = local_id else { return };
    let port = match current_transport.as_ref().map(|t| t.0) {
        Some(TransportArg::Udp) => UDP_PORT,
        Some(TransportArg::WebTransport) => WEBTRANSPORT_PORT,
        Some(TransportArg::WebSocket) | None => WEBSOCKET_PORT,
    };
    
    let host_addr = SocketAddr::new(SERVER_ADDR.into(), port);
    let auth = Authentication::Manual {
        server_addr: host_addr,
        client_id: local_id.0.to_bits(),
        private_key: PRIVATE_KEY,
        protocol_id: PROTOCOL_ID,
    };
    if let Ok(netcode) = NetcodeClient::new(auth, NetcodeConfig::default()) {
        commands.entity(trigger.entity).insert(netcode);
    }
}

fn buffer_input(
    mut query: Query<&mut ActionState<Inputs>, With<InputMarker<Inputs>>>,
    keypress: Res<ButtonInput<KeyCode>>,
) {
    if let Ok(mut action_state) = query.single_mut() {
        let mut direction = Direction {
            up: false,
            down: false,
            left: false,
            right: false,
        };
        if keypress.pressed(KeyCode::KeyW) || keypress.pressed(KeyCode::ArrowUp) {
            direction.up = true;
        }
        if keypress.pressed(KeyCode::KeyS) || keypress.pressed(KeyCode::ArrowDown) {
            direction.down = true;
        }
        if keypress.pressed(KeyCode::KeyA) || keypress.pressed(KeyCode::ArrowLeft) {
            direction.left = true;
        }
        if keypress.pressed(KeyCode::KeyD) || keypress.pressed(KeyCode::ArrowRight) {
            direction.right = true;
        }
        action_state.0 = Inputs::Direction(direction);
    }
}

fn player_movement(
    mut position_query: Query<(&mut PlayerPosition, &ActionState<Inputs>), With<Predicted>>,
) {
    for (position, input) in position_query.iter_mut() {
        shared_movement_behaviour(position, input);
    }
}

fn debug_input_state(
    query: Query<(Entity, Has<ActionState<Inputs>>, Has<InputMarker<Inputs>>), With<Predicted>>,
    mut timer: Local<Option<Timer>>,
    time: Res<Time>,
) {
    let timer = timer.get_or_insert_with(|| Timer::from_seconds(2.0, TimerMode::Repeating));
    timer.tick(time.delta());
    
    if timer.just_finished() {
        for (entity, has_action_state, has_marker) in query.iter() {
            info!(
                "🔍 Predicted {:?} - InputMarker: {}, ActionState: {}",
                entity, has_marker, has_action_state
            );
        }
        if query.is_empty() {
            info!("🔍 No Predicted entities found");
        }
    }
}

fn handle_predicted_spawn(
    trigger: On<Add, PlayerId>,
    mut predicted: Query<&mut PlayerColor, With<Predicted>>,
    mut commands: Commands,
) {
    let entity = trigger.entity;
    if let Ok(mut color) = predicted.get_mut(entity) {
        let hsva = Hsva {
            saturation: 0.4,
            ..Hsva::from(color.0)
        };
        color.0 = Color::from(hsva);
        info!("Predicted entity spawned: {:?}, adding InputMarker", entity);
        commands.entity(entity).insert(InputMarker::<Inputs>::default());
    }
}

fn handle_interpolated_spawn(
    trigger: On<Add, PlayerColor>,
    mut interpolated: Query<&mut PlayerColor, With<Interpolated>>,
) {
    if let Ok(mut color) = interpolated.get_mut(trigger.entity) {
        let hsva = Hsva {
            saturation: 0.1,
            ..Hsva::from(color.0)
        };
        color.0 = Color::from(hsva);
    }
}

fn lobby_ui(
    mut commands: Commands,
    mut contexts: EguiContexts,
    mut lobby_table: ResMut<LobbyTable>,
    lobbies: Option<Single<&Lobbies>>,
    message_sender: Option<Single<(
        Entity,
        &Client,
        &mut MessageSender<StartGame>,
        &mut MessageSender<JoinLobby>,
        &mut MessageSender<ExitLobby>,
    )>>,
    app_state: Res<State<AppState>>,
    mut next_app_state: ResMut<NextState<AppState>>,
) -> Result {
    let Some(message_sender) = message_sender else { return Ok(()) };
    let (client_entity, client, mut send_start_game, mut send_join_lobby, mut exit_lobby) =
        message_sender.into_inner();
    
    let window_name = match app_state.get() {
        AppState::Lobby => "Lobby List".to_string(),
        AppState::LobbyJoined { lobby_id } => format!("Lobby {}", lobby_id),
        AppState::Game => "Game".to_string(),
    };
    
    egui::Window::new(window_name)
        .anchor(egui::Align2::LEFT_TOP, [30.0, 30.0])
        .show(contexts.ctx_mut()?, |ui| {
            match app_state.get() {
                AppState::Lobby => {
                    // Show lobby list
                    let table = TableBuilder::new(ui)
                        .resizable(false)
                        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                        .column(Column::auto())
                        .column(Column::auto())
                        .column(Column::auto())
                        .column(Column::auto());
                    
                    table
                        .header(20.0, |mut header| {
                            header.col(|ui| { ui.strong("Lobby ID"); });
                            header.col(|ui| { ui.strong("Players"); });
                            header.col(|ui| { ui.strong("In Game?"); });
                            header.col(|_ui| {});
                        })
                        .body(|mut body| {
                            body.row(30.0, |mut row| {
                                row.col(|ui| { ui.label("Server"); });
                                row.col(|_ui| {});
                            });
                            
                            if let Some(lobbies) = &lobbies {
                                for (lobby_id, lobby) in lobbies.lobbies.iter().enumerate() {
                                    body.row(30.0, |mut row| {
                                        row.col(|ui| {
                                            ui.label(format!("Lobby {}", lobby_id));
                                        });
                                        row.col(|ui| {
                                            ui.label(format!("{}", lobby.players.len()));
                                        });
                                        row.col(|ui| {
                                            ui.checkbox(&mut { lobby.in_game }, "");
                                        });
                                        row.col(|ui| {
                                            if lobby.in_game {
                                                if ui.button("Join Game").clicked() {
                                                    let host = lobby_table.get_host();
                                                    info!("Joining game in lobby {}", lobby_id);
                                                    send_start_game.send::<Channel1>(StartGame {
                                                        lobby_id,
                                                        host,
                                                    });
                                                }
                                            } else if ui.button("Join Lobby").clicked() {
                                                info!("Joining lobby {}", lobby_id);
                                                send_join_lobby.send::<Channel1>(JoinLobby { lobby_id });
                                                next_app_state.set(AppState::LobbyJoined { lobby_id });
                                            }
                                        });
                                    });
                                }
                            }
                        });
                }
                AppState::LobbyJoined { lobby_id } => {
                    // Show lobby details
                    if let Some(lobbies) = &lobbies {
                        if let Some(lobby) = lobbies.lobbies.get(*lobby_id) {
                            let table = TableBuilder::new(ui)
                                .resizable(false)
                                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                                .column(Column::auto())
                                .column(Column::auto());
                            
                            table
                                .header(20.0, |mut header| {
                                    header.col(|ui| { ui.strong("Client ID"); });
                                    header.col(|ui| { ui.strong("Host"); });
                                })
                                .body(|mut body| {
                                    body.row(30.0, |mut row| {
                                        row.col(|ui| { ui.label("Server"); });
                                        row.col(|_ui| {});
                                    });
                                    
                                    for client_id in &lobby.players {
                                        lobby_table.clients.entry(*client_id).or_insert(false);
                                        body.row(30.0, |mut row| {
                                            row.col(|ui| {
                                                ui.label(format!("{:?}", client_id));
                                            });
                                            row.col(|ui| {
                                                ui.checkbox(
                                                    lobby_table.clients.get_mut(client_id).unwrap(),
                                                    "",
                                                );
                                            });
                                        });
                                    }
                                });
                        }
                    }
                    ui.add(egui::Separator::default().horizontal());
                }
                AppState::Game => {}
            }
            
            // Connection buttons
            match client.state {
                ClientState::Disconnected | ClientState::Disconnecting => {
                    if ui.button("Join lobby list").clicked() {
                        commands.trigger(Connect { entity: client_entity });
                    }
                }
                ClientState::Connecting => {
                    let _ = ui.button("Connecting...");
                }
                ClientState::Connected => {
                    match app_state.get() {
                        AppState::Lobby => {
                            if ui.button("Exit lobby list").clicked() {
                                commands.trigger(Disconnect { entity: client_entity });
                            }
                        }
                        AppState::LobbyJoined { lobby_id } => {
                            if ui.button("Exit lobby").clicked() {
                                info!("Exiting lobby {}", lobby_id);
                                exit_lobby.send::<Channel1>(ExitLobby { lobby_id: *lobby_id });
                                next_app_state.set(AppState::Lobby);
                            }
                            if ui.button("Start game").clicked() {
                                let host = lobby_table.get_host();
                                info!("Starting game for lobby {}! Host: {:?}", lobby_id, host);
                                send_start_game.send::<Channel1>(StartGame {
                                    lobby_id: *lobby_id,
                                    host,
                                });
                            }
                        }
                        AppState::Game => {
                            if ui.button("Exit game").clicked() {
                                next_app_state.set(AppState::Lobby);
                                commands.trigger(Disconnect { entity: client_entity });
                            }
                        }
                    }
                }
            }
        });
    
    Ok(())
}

fn receive_start_game_message(
    mut commands: Commands,
    local_client: Option<Single<(Entity, &mut MessageReceiver<StartGame>, &LocalId)>>,
    _lobby_table: Res<LobbyTable>,
    mut next_app_state: ResMut<NextState<AppState>>,
    server: Option<Single<Entity, With<Server>>>,
    _current_transport: Option<Res<CurrentTransport>>,
) -> Result {
    let Some(local_client) = local_client else { return Ok(()) };
    let (local_client_entity, mut receiver, local_id) = local_client.into_inner();
    
    for message in receiver.receive() {
        info!("Received start_game message! {:?}", message);
        let host = message.host;
        
        next_app_state.set(AppState::Game);
        
        if let Some(host) = host {
            // Need Server entity for host mode
            let Some(server) = &server else {
                warn!("No Server entity found for host mode");
                continue;
            };
            let server = server.entity();
            
            if host == local_id.0 {
                info!("We are the host of the game!");
                
                // Unlink from dedicated server
                commands.trigger(Unlink {
                    entity: local_client_entity,
                    reason: "Client becoming Host".to_string(),
                });
                
                // Remove netcode client
                commands.entity(local_client_entity).remove::<NetcodeClient>();
                
                // Become host-client
                commands.entity(local_client_entity).insert(LinkOf { server });
                info!("Connected as Host Client");
            } else {
                info!("Game hosted by {:?}. Connecting to host...", host);
                
                // Unlink from dedicated server
                commands.trigger(Unlink {
                    entity: local_client_entity,
                    reason: "Connecting to host-client".to_string(),
                });
                
                // Connect to host player's server
                let host_addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), HOST_SERVER_PORT);
                let auth = Authentication::Manual {
                    server_addr: host_addr,
                    client_id: local_id.0.to_bits(),
                    private_key: PRIVATE_KEY,
                    protocol_id: PROTOCOL_ID,
                };
                commands.entity(local_client_entity).insert((
                    NetcodeClient::new(auth, NetcodeConfig::default())?,
                    PeerAddr(host_addr),
                ));
            }
            
            // Trigger connection
            commands.trigger(Connect { entity: local_client_entity });
        }
    }
    
    Ok(())
}
