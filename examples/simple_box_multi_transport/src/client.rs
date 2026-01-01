//! Client - connects via UDP or WebTransport.

use crate::protocol::*;
use crate::shared;
use bevy::color::Hsva;
use bevy::prelude::*;
use core::net::Ipv4Addr;
use std::net::SocketAddr;
use lightyear::connection::client::{Connected, Disconnected, Connecting};
use lightyear::netcode::Key;
use lightyear::prelude::client::*;
use lightyear::prelude::client::input::InputSystems;
use lightyear::prelude::input::native::*;
use lightyear::prelude::{
    Authentication, LocalAddr, PeerAddr, Link, ReplicationReceiver, 
    PredictionManager, UdpIo, Predicted, Interpolated, Replicated,
};

pub const UDP_PORT: u16 = 5000;
pub const WEBTRANSPORT_PORT: u16 = 5001;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Transport {
    #[default]
    Udp,
    WebTransport,
}

/// Resource storing client configuration
#[derive(Resource)]
pub struct ClientConfig {
    pub client_id: u64,
    pub transport: Transport,
    pub cert_digest: Option<String>,
}

pub struct ClientPlugin;

impl Plugin for ClientPlugin {
    fn build(&self, app: &mut App) {
        // Client systems  
        app.add_systems(Startup, startup_client);
        app.add_systems(
            FixedPreUpdate,
            buffer_input.in_set(InputSystems::WriteClientInputs),
        );
        app.add_systems(FixedUpdate, player_movement);
        app.add_observer(on_connecting);
        app.add_observer(on_connected);
        app.add_observer(on_disconnected);
        app.add_observer(handle_predicted_spawn);
        app.add_observer(handle_interpolated_spawn);
        // Debug observers
        app.add_observer(on_replicated);
        app.add_observer(on_player_added);
        app.add_observer(on_predicted_added);
        app.add_observer(on_interpolated_added);
    }
}

fn on_replicated(trigger: On<Add, Replicated>) {
    info!("📥 Replicated marker added to entity {:?}", trigger.entity);
}

fn on_player_added(trigger: On<Add, Player>) {
    info!("🎮 Player component added to entity {:?}", trigger.entity);
}

fn on_predicted_added(trigger: On<Add, Predicted>) {
    info!("🔮 Predicted marker added to entity {:?}", trigger.entity);
}

fn on_interpolated_added(trigger: On<Add, Interpolated>) {
    info!("👤 Interpolated marker added to entity {:?}", trigger.entity);
}

fn on_connecting(trigger: On<Add, Connecting>, names: Query<&Name>) {
    if let Ok(name) = names.get(trigger.entity) {
        info!("🔄 {} is connecting...", name);
    }
}

fn on_connected(trigger: On<Add, Connected>, names: Query<&Name>) {
    if let Ok(name) = names.get(trigger.entity) {
        info!("✅ {} connected to server!", name);
    }
}

fn on_disconnected(trigger: On<Add, Disconnected>, names: Query<&Name>) {
    if let Ok(name) = names.get(trigger.entity) {
        info!("❌ {} disconnected from server", name);
    }
}

/// Connect to server using specified transport
fn startup_client(mut commands: Commands, config: Res<ClientConfig>) -> Result {
    info!("\n=== Simple Box Multi-Transport Client ===\n");
    info!("Client ID: {}", config.client_id);
    
    let client_port = 4000 + (config.client_id % 100) as u16;
    let local_addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), client_port);
    
    match config.transport {
        Transport::Udp => {
            let server_addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), UDP_PORT);
            info!("📡 Connecting via UDP to {}", server_addr);
            
            let auth = Authentication::Manual {
                server_addr,
                client_id: config.client_id,
                private_key: Key::default(),
                protocol_id: 0,
            };
            
            let client = commands
                .spawn((
                    Client::default(),
                    LocalAddr(local_addr),
                    PeerAddr(server_addr),
                    Link::new(None),
                    ReplicationReceiver::default(),
                    PredictionManager::default(),
                    NetcodeClient::new(auth, NetcodeConfig::default())?,
                    UdpIo::default(),
                    Name::new("UdpClient"),
                ))
                .id();
            commands.trigger(Connect { entity: client });
        }
        Transport::WebTransport => {
            let server_addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), WEBTRANSPORT_PORT);
            info!("🌐 Connecting via WebTransport to {}", server_addr);
            
            let auth = Authentication::Manual {
                server_addr,
                client_id: config.client_id,
                private_key: Key::default(),
                protocol_id: 0,
            };
            
            let cert_digest = config.cert_digest.clone().unwrap_or_else(|| {
                warn!("⚠️  No certificate digest provided! Copy digest from server output and use: --cert <DIGEST>");
                String::new()
            });
            
            let client = commands
                .spawn((
                    Client::default(),
                    LocalAddr(local_addr),
                    PeerAddr(server_addr),
                    Link::new(None),
                    ReplicationReceiver::default(),
                    PredictionManager::default(),
                    NetcodeClient::new(auth, NetcodeConfig::default())?,
                    WebTransportClientIo {
                        certificate_digest: cert_digest,
                    },
                    Name::new("WebTransportClient"),
                ))
                .id();
            commands.trigger(Connect { entity: client });
        }
    }
    
    info!("\nUse WASD or arrow keys to move\n");
    Ok(())
}

/// Read keyboard input and buffer it for sending to server
fn buffer_input(
    mut query: Query<(Entity, &mut ActionState<Inputs>), With<InputMarker<Inputs>>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut logged: Local<bool>,
) {
    if !*logged {
        let count = query.iter().count();
        info!("🎮 buffer_input: Found {} entities with ActionState+InputMarker", count);
        if count == 0 {
            info!("   ⚠️ No entities to buffer inputs for!");
        }
        *logged = true;
    }
    
    for (entity, mut action_state) in query.iter_mut() {
        let mut direction = Direction {
            up: false,
            down: false,
            left: false,
            right: false,
        };
        
        if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
            direction.up = true;
        }
        if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
            direction.down = true;
        }
        if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
            direction.left = true;
        }
        if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
            direction.right = true;
        }
        
        // Always set a value - None means "missing", not "no keys pressed"
        action_state.0 = Inputs::Direction(direction);
    }
}

/// Apply inputs to predicted entities for client-side prediction
fn player_movement(
    mut position_query: Query<(&mut PlayerPosition, &ActionState<Inputs>), With<Predicted>>,
) {
    for (position, input) in position_query.iter_mut() {
        // NOTE: be careful to directly pass Mut<PlayerPosition>
        // getting a mutable reference triggers change detection
        shared::shared_movement_behaviour(position, input);
    }
}


/// When we receive a predicted entity (our player), add input marker and adjust color
fn handle_predicted_spawn(
    trigger: On<Add, PlayerId>,
    mut predicted: Query<&mut PlayerColor, With<Predicted>>,
    mut commands: Commands,
) {
    let entity = trigger.entity;
    if let Ok(mut color) = predicted.get_mut(entity) {
        // Make predicted entities more saturated
        let hsva = Hsva {
            saturation: 0.4,
            ..Hsva::from(color.0)
        };
        color.0 = Color::from(hsva);
        // Add InputMarker AND ActionState so this entity can receive and buffer inputs
        commands.entity(entity).insert((
            InputMarker::<Inputs>::default(),
            ActionState::<Inputs>::default(),
        ));
    }
}

/// When we receive an interpolated entity (other players), adjust color
fn handle_interpolated_spawn(
    trigger: On<Add, PlayerColor>,
    mut interpolated: Query<&mut PlayerColor, With<Interpolated>>,
) {
    if let Ok(mut color) = interpolated.get_mut(trigger.entity) {
        // Make interpolated entities less saturated
        let hsva = Hsva {
            saturation: 0.1,
            ..Hsva::from(color.0)
        };
        color.0 = Color::from(hsva);
    }
}
