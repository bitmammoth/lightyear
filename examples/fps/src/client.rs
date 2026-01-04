//! Client-side code for the FPS example

use crate::protocol::*;
use crate::{TransportArg, SERVER_ADDR, UDP_PORT, WEBTRANSPORT_PORT, WEBSOCKET_PORT};
use bevy::prelude::*;
use leafwing_input_manager::plugin::InputManagerSystem;
use leafwing_input_manager::prelude::*;
use lightyear::input::client::InputSystems;
use lightyear::netcode::Key;
use lightyear::prelude::client::*;
use lightyear::prelude::*;
use lightyear::websocket::prelude::client::ClientConfig as WebSocketClientConfig;
use std::net::{Ipv4Addr, SocketAddr};

const PROTOCOL_ID: u64 = 0;
const PRIVATE_KEY: Key = [0; 32];

pub struct ClientPlugin;

impl Plugin for ClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedPreUpdate,
            update_cursor_state_from_window
                .before(InputSystems::BufferClientInputs)
                .in_set(InputManagerSystem::ManualControl),
        );
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
    
    info!("Client {} connecting via {:?} to {}", client_id, transport, server_addr);
    
    let client = match transport {
        TransportArg::Udp => {
            world.spawn((
                Client::default(),
                LocalAddr(client_addr),
                PeerAddr(server_addr),
                Link::new(None),
                ReplicationReceiver::default(),
                NetcodeClient::new(auth, NetcodeConfig::default()).unwrap(),
                UdpIo::default(),
                Name::new("UdpClient"),
            )).id()
        }
        TransportArg::WebTransport => {
            let certificate_digest = include_str!("../../../certificates/digest.txt")
                .trim()
                .replace(":", "")
                .to_string();
            world.spawn((
                Client::default(),
                LocalAddr(client_addr),
                PeerAddr(server_addr),
                Link::new(None),
                ReplicationReceiver::default(),
                NetcodeClient::new(auth, NetcodeConfig::default()).unwrap(),
                WebTransportClientIo { certificate_digest },
                Name::new("WebTransportClient"),
            )).id()
        }
        TransportArg::WebSocket => {
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
                Name::new("WebSocketClient"),
            )).id()
        }
    };
    
    world.trigger(Connect { entity: client });
    info!("Client {} connection initiated", client_id);
}

/// Compute the world-position of the cursor and set it in the DualAxis input
fn update_cursor_state_from_window(
    window: Single<&Window>,
    q_camera: Query<(&Camera, &GlobalTransform)>,
    mut action_state_query: Query<&mut ActionState<PlayerActions>, With<Predicted>>,
) {
    let Ok((camera, camera_transform)) = q_camera.single() else {
        return;
    };
    if let Some(world_position) = window
        .cursor_position()
        .and_then(|cursor| Some(camera.viewport_to_world(camera_transform, cursor).unwrap()))
        .map(|ray| ray.origin.truncate())
    {
        for mut action_state in action_state_query.iter_mut() {
            action_state.set_axis_pair(&PlayerActions::MoveCursor, world_position);
        }
    }
}

/// When the predicted copy of the client-owned entity is spawned:
/// - Assign a different saturation
/// - Add input mapping
fn handle_predicted_spawn(
    trigger: On<Add, (PlayerId, Predicted)>,
    mut commands: Commands,
    mut player_query: Query<&mut ColorComponent, With<Predicted>>,
) {
    if let Ok(mut color) = player_query.get_mut(trigger.entity) {
        let hsva = Hsva {
            saturation: 0.4,
            ..Hsva::from(color.0)
        };
        color.0 = Color::from(hsva);
        info!("Predicted entity spawned: {:?}, adding InputMap", trigger.entity);
        commands.entity(trigger.entity).insert(InputMap::new([
            (PlayerActions::Up, KeyCode::KeyW),
            (PlayerActions::Down, KeyCode::KeyS),
            (PlayerActions::Left, KeyCode::KeyA),
            (PlayerActions::Right, KeyCode::KeyD),
            (PlayerActions::Shoot, KeyCode::Space),
        ]));
    }
}

/// When interpolated copy of other players' entities spawns:
/// - Change saturation
fn handle_interpolated_spawn(
    trigger: On<Add, ColorComponent>,
    mut interpolated: Query<&mut ColorComponent, Added<Interpolated>>,
) {
    if let Ok(mut color) = interpolated.get_mut(trigger.entity) {
        let hsva = Hsva {
            saturation: 0.1,
            ..Hsva::from(color.0)
        };
        color.0 = Color::from(hsva);
    }
}
