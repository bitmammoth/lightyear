//! Server-side code for the FPS example

use crate::protocol::*;
use crate::shared::{color_from_id, BOT_RADIUS};
use crate::{UDP_PORT, WEBTRANSPORT_PORT, WEBSOCKET_PORT};
use avian2d::prelude::*;
use bevy::prelude::*;
use core::time::Duration;
use core::net::Ipv4Addr;
use leafwing_input_manager::prelude::*;
use lightyear::connection::server::Started;
use lightyear::prelude::server::*;
use lightyear::prelude::*;
use lightyear_avian2d::prelude::{
    LagCompensationHistory, LagCompensationPlugin, LagCompensationSpatialQuery,
    LagCompensationSystems,
};
use std::net::SocketAddr;

const SEND_INTERVAL: Duration = Duration::from_millis(40);
const BULLET_COLLISION_DISTANCE_CHECK: f32 = 4.0;

pub struct ServerPlugin;

impl Plugin for ServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(LagCompensationPlugin);
        app.add_systems(Startup, (start_server, spawn_bots));
        app.add_observer(handle_new_client);
        app.add_observer(spawn_player);
        
        app.add_systems(FixedUpdate, interpolated_bot_movement);
        app.add_systems(
            PhysicsSchedule,
            compute_hit_lag_compensation.in_set(LagCompensationSystems::Collisions),
        );
        app.add_systems(
            FixedPostUpdate,
            compute_hit_prediction.after(PhysicsSystems::StepSimulation),
        );
    }
}

/// Start the multi-transport server with UDP, WebTransport, and WebSocket
fn start_server(mut commands: Commands) -> Result {
    // 1. Spawn the logical Server entity
    let server = commands
        .spawn((
            Server::default(),
            Name::new("Server"),
        ))
        .id();
    info!("Spawned logical Server entity: {:?}", server);
    
    // 2. UDP Transport
    let udp_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), UDP_PORT);
    let udp_transport = commands
        .spawn((
            NetcodeServer::new(NetcodeConfig::default()),
            LocalAddr(udp_addr),
            ServerUdpIo::default(),
            TransportOf::new(server),
            Name::new("UdpTransport"),
        ))
        .id();
    commands.trigger(Start { entity: udp_transport });
    info!("UDP transport starting on port {} -> Server {:?}", UDP_PORT, server);

    // 3. WebTransport
    let wt_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), WEBTRANSPORT_PORT);
    let sans = vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "::1".to_string(),
    ];
    let identity = Identity::self_signed(sans).unwrap();
    let digest = identity.certificate_chain().as_slice()[0].hash();
    info!("WebTransport certificate digest: {}", digest);
    
    let wt_transport = commands
        .spawn((
            NetcodeServer::new(NetcodeConfig::default()),
            LocalAddr(wt_addr),
            WebTransportServerIo { certificate: identity },
            TransportOf::new(server),
            Name::new("WebTransportTransport"),
        ))
        .id();
    commands.trigger(Start { entity: wt_transport });
    info!("WebTransport transport starting on port {} -> Server {:?}", WEBTRANSPORT_PORT, server);

    // 4. WebSocket
    let ws_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), WEBSOCKET_PORT);
    let ws_sans = vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "::1".to_string(),
    ];
    let ws_config = lightyear::websocket::server::ServerConfig::builder()
        .with_bind_address(ws_addr)
        .with_identity(lightyear::websocket::server::Identity::self_signed(ws_sans).unwrap());
    let ws_transport = commands
        .spawn((
            NetcodeServer::new(NetcodeConfig::default()),
            LocalAddr(ws_addr),
            WebSocketServerIo { config: ws_config },
            TransportOf::new(server),
            Name::new("WebSocketTransport"),
        ))
        .id();
    commands.trigger(Start { entity: ws_transport });
    info!("WebSocket transport starting on port {} -> Server {:?}", WEBSOCKET_PORT, server);

    // 5. Mark server as Started
    commands.entity(server).insert(Started);
    info!("Multi-transport FPS server started!");
    
    Ok(())
}

pub fn handle_new_client(trigger: On<Add, LinkOf>, mut commands: Commands) {
    commands
        .entity(trigger.entity)
        .insert(ReplicationSender::new(
            SEND_INTERVAL,
            SendUpdatesMode::SinceLastAck,
            false,
        ));
}

pub fn spawn_player(
    trigger: On<Add, Connected>,
    query: Query<&RemoteId, With<ClientOf>>,
    mut commands: Commands,
) {
    let Ok(client_id) = query.get(trigger.entity) else {
        return;
    };
    let client_id = client_id.0;
    let y = (client_id.to_bits() as f32 * 50.0) % 500.0 - 250.0;
    let color = color_from_id(client_id);
    info!("Spawning player with id: {:?}", client_id);
    commands.spawn((
        Replicate::to_clients(NetworkTarget::All),
        PredictionTarget::to_clients(NetworkTarget::Single(client_id)),
        InterpolationTarget::to_clients(NetworkTarget::AllExceptSingle(client_id)),
        ControlledBy {
            owner: trigger.entity,
            lifetime: Default::default(),
        },
        Score(0),
        PlayerId(client_id),
        RigidBody::Kinematic,
        Transform::from_xyz(0.0, y, 0.0),
        ColorComponent(color),
        ActionState::<PlayerActions>::default(),
        PlayerMarker,
        Name::new("Player"),
    ));
}

/// Spawn bots (one predicted, one interpolated)
pub fn spawn_bots(mut commands: Commands) {
    commands.spawn((
        InterpolatedBot,
        Name::new("InterpolatedBot"),
        Replicate::to_clients(NetworkTarget::All),
        InterpolationTarget::to_clients(NetworkTarget::All),
        DisableReplicateHierarchy,
        Transform::from_xyz(-200.0, 10.0, 0.0),
        RigidBody::Kinematic,
        Collider::circle(BOT_RADIUS),
        LagCompensationHistory::default(),
    ));
    commands.spawn((
        PredictedBot,
        Name::new("PredictedBot"),
        Replicate::to_clients(NetworkTarget::All),
        PredictionTarget::to_clients(NetworkTarget::All),
        DisableReplicateHierarchy,
        Transform::from_xyz(200.0, 10.0, 0.0),
        RigidBody::Kinematic,
        Collider::circle(BOT_RADIUS),
    ));
}

fn interpolated_bot_movement(
    timeline: Res<LocalTimeline>,
    mut query: Query<&mut Position, With<InterpolatedBot>>,
) {
    let tick = timeline.tick();
    query.iter_mut().for_each(|mut position| {
        let direction = if (tick.0 / 200) % 2 == 0 { -1.0 } else { 1.0 };
        position.y += crate::shared::BOT_MOVE_SPEED * direction;
    });
}

/// Compute hits using lag compensation for interpolated bots
pub fn compute_hit_lag_compensation(
    mut commands: Commands,
    timeline: Res<LocalTimeline>,
    query: LagCompensationSpatialQuery,
    bullets: Query<
        (Entity, &PlayerId, &Position, &LinearVelocity, &ControlledBy),
        With<BulletMarker>,
    >,
    client_query: Query<&lightyear::interpolation::plugin::InterpolationDelay, With<ClientOf>>,
    mut player_query: Query<(&mut Score, &PlayerId), With<PlayerMarker>>,
) {
    let tick = timeline.tick();
    bullets
        .iter()
        .for_each(|(entity, id, position, velocity, controlled_by)| {
            let Ok(delay) = client_query.get(controlled_by.owner) else {
                return;
            };
            if let Some(hit_data) = query.cast_ray(
                *delay,
                position.0,
                Dir2::new_unchecked(velocity.0.normalize()),
                BULLET_COLLISION_DISTANCE_CHECK,
                false,
                &mut SpatialQueryFilter::default(),
            ) {
                info!(
                    ?tick,
                    ?hit_data,
                    ?entity,
                    "Collision with interpolated bot! Despawning bullet"
                );
                player_query
                    .iter_mut()
                    .find(|(_, player_id)| player_id.0 == id.0)
                    .map(|(mut score, _)| {
                        score.0 += 1;
                    });
                commands.entity(entity).despawn();
            }
        })
}

/// Compute hits for predicted bots (no lag compensation needed)
pub fn compute_hit_prediction(
    mut commands: Commands,
    timeline: Res<LocalTimeline>,
    query: SpatialQuery,
    bullets: Query<(Entity, &PlayerId, &Position, &LinearVelocity), With<BulletMarker>>,
    bot_query: Query<(), With<PredictedBot>>,
    mut player_query: Query<(&mut Score, &PlayerId), With<PlayerMarker>>,
) {
    let tick = timeline.tick();
    bullets.iter().for_each(|(entity, id, position, velocity)| {
        if let Some(hit_data) = query.cast_ray_predicate(
            position.0,
            Dir2::new_unchecked(velocity.0.normalize()),
            BULLET_COLLISION_DISTANCE_CHECK,
            false,
            &SpatialQueryFilter::default(),
            &|entity| bot_query.get(entity).is_ok(),
        ) {
            info!(
                ?tick,
                ?hit_data,
                ?entity,
                "Collision with predicted bot! Despawning bullet"
            );
            player_query
                .iter_mut()
                .find(|(_, player_id)| player_id.0 == id.0)
                .map(|(mut score, _)| {
                    score.0 += 1;
                });
            commands.entity(entity).despawn();
        }
    })
}
