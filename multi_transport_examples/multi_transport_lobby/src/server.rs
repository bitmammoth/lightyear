//! Server-side code for the lobby example with multi-transport support

use bevy::prelude::*;
use std::net::{Ipv4Addr, SocketAddr};

use crate::protocol::*;
use crate::shared::shared_movement_behaviour;
use crate::{UDP_PORT, WEBTRANSPORT_PORT, WEBSOCKET_PORT};
use lightyear::prelude::server::*;
use lightyear::prelude::*;
use lightyear::input::native::prelude::ActionState;
use lightyear::connection::server::Started;

pub struct ServerPlugin {
    pub is_dedicated: bool,
}

impl Plugin for ServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RoomPlugin);
        
        app.add_observer(handle_new_client);
        app.add_systems(FixedUpdate, game_movement);
        app.add_observer(handle_disconnections);
        
        if self.is_dedicated {
            app.add_systems(Startup, start_dedicated_server);
            app.add_systems(
                Update,
                (
                    handle_lobby_join,
                    handle_lobby_exit,
                    handle_start_game,
                ),
            );
        } else {
            app.add_observer(handle_host_connections);
        }
    }
}

/// Start multi-transport server at app startup
pub fn start_multi_transport_server(app: &mut App) {
    use lightyear::connection::server::Start;
    
    let world = app.world_mut();
    
    // Create main server entity
    let server = world.spawn((
        Server::default(),
        Name::new("MultiTransportServer"),
    )).id();
    info!("Spawned logical Server entity: {:?}", server);
    
    // UDP transport
    let udp_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), UDP_PORT);
    let udp_transport = world.spawn((
        NetcodeServer::new(NetcodeConfig::default()),
        LocalAddr(udp_addr),
        ServerUdpIo::default(),
        TransportOf::new(server),
        Name::new("UdpTransport"),
    )).id();
    world.trigger(Start { entity: udp_transport });
    info!("UDP transport starting on port {} -> Server {:?}", UDP_PORT, server);
    
    // WebTransport
    let wt_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), WEBTRANSPORT_PORT);
    let sans = vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "::1".to_string(),
    ];
    let identity = Identity::self_signed(sans).unwrap();
    let digest = identity.certificate_chain().as_slice()[0].hash();
    info!("WebTransport certificate digest: {}", digest);
    
    let wt_transport = world.spawn((
        NetcodeServer::new(NetcodeConfig::default()),
        LocalAddr(wt_addr),
        WebTransportServerIo { certificate: identity },
        TransportOf::new(server),
        Name::new("WebTransportTransport"),
    )).id();
    world.trigger(Start { entity: wt_transport });
    info!("WebTransport transport starting on port {} -> Server {:?}", WEBTRANSPORT_PORT, server);

    // WebSocket
    let ws_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), WEBSOCKET_PORT);
    let ws_sans = vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "::1".to_string(),
    ];
    let ws_config = lightyear::websocket::server::ServerConfig::builder()
        .with_bind_address(ws_addr)
        .with_identity(lightyear::websocket::server::Identity::self_signed(ws_sans).unwrap());
    let ws_transport = world.spawn((
        NetcodeServer::new(NetcodeConfig::default()),
        LocalAddr(ws_addr),
        WebSocketServerIo { config: ws_config },
        TransportOf::new(server),
        Name::new("WebSocketTransport"),
    )).id();
    world.trigger(Start { entity: ws_transport });
    info!("WebSocket transport starting on port {} -> Server {:?}", WEBSOCKET_PORT, server);

    // Mark server as started
    world.entity_mut(server).insert(Started);
    info!("Multi-transport lobby server started!");
}

fn start_dedicated_server(mut commands: Commands) {
    let mut lobbies = Lobbies::default();
    let room = commands.spawn((Room::default(), Name::from("Room"))).id();
    lobbies.lobbies.push(Lobby::new(room));
    commands.spawn((
        Name::from("Lobbies"),
        lobbies,
        Replicate::to_clients(NetworkTarget::All),
    ));
}

fn handle_new_client(trigger: On<Add, LinkOf>, mut commands: Commands) {
    commands.entity(trigger.entity).insert((
        ReplicationSender::new(SEND_INTERVAL, SendUpdatesMode::SinceLastAck, false),
        Name::from("Client"),
    ));
}

fn spawn_player_entity(
    commands: &mut Commands,
    client_entity: Entity,
    client_id: PeerId,
    dedicated_server: bool,
) -> Entity {
    let h = (((client_id.to_bits().wrapping_mul(30)) % 360) as f32) / 360.0;
    let s = 0.8;
    let l = 0.5;
    let color = Color::hsl(h, s, l);
    let entity = commands
        .spawn((
            PlayerId(client_id),
            PlayerPosition(Vec2::ZERO),
            PlayerColor(color),
            Replicate::to_clients(NetworkTarget::All),
            PredictionTarget::to_clients(NetworkTarget::Single(client_id)),
            InterpolationTarget::to_clients(NetworkTarget::AllExceptSingle(client_id)),
            ControlledBy {
                owner: client_entity,
                lifetime: Default::default(),
            },
            Name::from("Player"),
        ))
        .id();
    if dedicated_server {
        commands.entity(entity).insert(NetworkVisibility);
    }
    info!("Created player entity {:?} for client {:?}", entity, client_id);
    entity
}

/// Handle connections in host-server mode
fn handle_host_connections(
    trigger: On<Add, (Connected, lightyear::connection::host::HostClient)>,
    query: Query<&RemoteId, With<ClientOf>>,
    mut commands: Commands,
) {
    let Ok(remote_id) = query.get(trigger.entity) else {
        return;
    };
    let client_id = remote_id.0;
    info!("HostServer spawn player for client {:?}", client_id);
    spawn_player_entity(&mut commands, trigger.entity, client_id, false);
}

fn handle_disconnections(
    trigger: On<Add, Disconnected>,
    query: Query<&RemoteId, With<ClientOf>>,
    lobbies: Option<Single<&mut Lobbies>>,
    mut commands: Commands,
) {
    if let Ok(remote_id) = query.get(trigger.entity) {
        info!("Client {:?} disconnected", remote_id.0);
        if let Some(mut lobbies) = lobbies {
            lobbies.remove_client(remote_id.0, &mut commands);
        }
    }
}

fn game_movement(
    _server_started: Option<Single<(), (With<Server>, With<Started>)>>,
    mut position_query: Query<(&mut PlayerPosition, Option<&ActionState<Inputs>>), Without<Predicted>>,
) {
    for (position, inputs) in position_query.iter_mut() {
        if let Some(inputs) = inputs {
            shared_movement_behaviour(position, inputs);
        }
    }
}

fn handle_lobby_join(
    mut receiver: Query<(Entity, &RemoteId, &mut MessageReceiver<JoinLobby>)>,
    mut lobbies: Option<Single<&mut Lobbies>>,
    mut commands: Commands,
) {
    let Some(ref mut lobbies) = lobbies else { return };
    
    for (client_entity, remote_id, mut message_receiver) in receiver.iter_mut() {
        let client_id = remote_id.0;
        message_receiver.receive().for_each(|message| {
            let lobby_id = message.lobby_id;
            let lobby = lobbies.lobbies.get_mut(lobby_id).unwrap();
            let room = lobby.room;
            info!("Client {:?} joined lobby {:?}", client_id, lobby_id);
            lobby.players.push(client_id);
            commands.trigger(RoomEvent {
                target: RoomTarget::AddSender(client_entity),
                room,
            });
            if lobby.in_game {
                let entity = spawn_player_entity(&mut commands, client_entity, client_id, true);
                commands.trigger(RoomEvent {
                    target: RoomTarget::AddEntity(entity),
                    room,
                });
            }
            if !lobbies.has_empty_lobby() {
                let room = commands.spawn(Room::default()).id();
                lobbies.lobbies.push(Lobby::new(room));
            }
        });
    }
}

fn handle_lobby_exit(
    mut events: Query<(Entity, &RemoteId, &mut MessageReceiver<ExitLobby>), With<Connected>>,
    mut lobbies: Option<Single<&mut Lobbies>>,
    mut commands: Commands,
) {
    let Some(ref mut lobbies) = lobbies else { return };
    
    for (sender, remote_id, mut receiver) in events.iter_mut() {
        let client_id = remote_id.0;
        for message in receiver.receive() {
            let lobby_id = message.lobby_id;
            info!("Client {:?} exited lobby {:?}", client_id, lobby_id);
            let room = lobbies.lobbies[lobby_id].room;
            commands.trigger(RoomEvent {
                target: RoomTarget::RemoveSender(sender),
                room,
            });
            lobbies.remove_client(client_id, &mut commands);
        }
    }
}

fn handle_start_game(
    server: Option<Single<&Server>>,
    mut events: Query<(Entity, &RemoteId, &mut MessageReceiver<StartGame>), With<Connected>>,
    mut multi_sender: ServerMultiMessageSender,
    mut lobbies: Option<Single<&mut Lobbies>>,
    mut commands: Commands,
) -> Result {
    let Some(ref mut lobbies) = lobbies else { return Ok(()) };
    let Some(server) = server else { return Ok(()) };
    let server = server.into_inner();
    
    for (sender, remote_id, mut receiver) in events.iter_mut() {
        let client_id = remote_id.0;
        for event in receiver.receive() {
            info!("Received start game message! {:?}", event);
            let lobby_id = event.lobby_id;
            let host = event.host;
            let lobby = lobbies.lobbies.get_mut(lobby_id).unwrap();

            if !lobby.in_game {
                lobby.in_game = true;
                lobby.host = host;
            }

            if !lobby.players.contains(&client_id) {
                info!("Player {:?} joining mid-game", client_id);
                lobby.players.push(client_id);
                if host.is_none() {
                    let entity = spawn_player_entity(&mut commands, sender, client_id, true);
                    commands.trigger(RoomEvent {
                        target: RoomTarget::AddEntity(entity),
                        room: lobby.room,
                    });
                    commands.trigger(RoomEvent {
                        target: RoomTarget::AddSender(sender),
                        room: lobby.room,
                    });
                }
                multi_sender.send::<_, Channel1>(
                    &StartGame { lobby_id, host: lobby.host },
                    server,
                    &NetworkTarget::Single(client_id),
                )?;
            } else {
                if host.is_none() {
                    info!("Dedicated server hosting lobby {:?}", lobby_id);
                    for player in &lobby.players {
                        let entity = spawn_player_entity(&mut commands, sender, *player, true);
                        commands.trigger(RoomEvent {
                            target: RoomTarget::AddEntity(entity),
                            room: lobby.room,
                        });
                    }
                }
                multi_sender.send::<_, Channel1>(
                    &StartGame { lobby_id, host: lobby.host },
                    server,
                    &NetworkTarget::Only(lobby.players.clone()),
                )?;
            }
        }
    }
    Ok(())
}
