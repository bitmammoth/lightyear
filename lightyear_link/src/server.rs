use crate::{LinkPlugin, Linked, Linking, Unlink, Unlinked};
use alloc::{format, string::String, vec::Vec};
use bevy_app::{App, Plugin};
use bevy_ecs::lifecycle::HookContext;
use bevy_ecs::prelude::*;
use bevy_ecs::{
    relationship::{
        Relationship, RelationshipHookMode, RelationshipSourceCollection, RelationshipTarget,
    },
    world::DeferredWorld,
};
use bevy_reflect::Reflect;
use bevy_utils::prelude::DebugName;
#[allow(unused_imports)]
use tracing::{trace, warn};

// ============================================================================
// Server - The logical server that all clients connect to
// ============================================================================

/// The logical server entity that all client connections (LinkOf) point to.
///
/// In a multi-transport setup, you spawn ONE Server entity, and multiple
/// transport entities (UDP, WebTransport, etc.) that feed connections to it.
///
/// # Example
/// ```ignore
/// // Spawn the logical server
/// let server = commands.spawn(Server::default()).id();
///
/// // Spawn transports that connect to the server
/// commands.spawn((ServerUdpIo::default(), TransportOf { server }));
/// commands.spawn((WebTransportServerIo { .. }, TransportOf { server }));
/// ```
#[derive(Component, Default, Debug, PartialEq, Eq, Reflect)]
#[component(on_add = Server::on_add)]
#[relationship_target(relationship = LinkOf, linked_spawn)]
pub struct Server {
    links: Vec<Entity>,
}

impl Server {
    fn on_add(mut world: DeferredWorld, context: HookContext) {
        let entity_ref = world.entity(context.entity);
        if !entity_ref.contains::<Unlinked>()
            && !entity_ref.contains::<Linked>()
            && !entity_ref.contains::<Linking>()
        {
            trace!("Inserting Unlinked because Server was added");
            world.commands().entity(context.entity).insert(Unlinked {
                reason: String::new(),
            });
        };
    }

    fn unlinked(
        trigger: On<Add, Unlinked>,
        mut query: Query<(&Server, &Unlinked)>,
        mut commands: Commands,
    ) {
        if let Ok((server_link, unlinked)) = query.get_mut(trigger.entity) {
            for link_of in server_link.collection() {
                commands.trigger(Unlink {
                    entity: trigger.entity,
                    reason: unlinked.reason.clone(),
                });
                if let Ok(mut c) = commands.get_entity(*link_of) {
                    trace!("d");
                    // cannot simply insert Unlinked because then we wouldn't close aeronet sessions...
                    c.try_despawn();
                }
            }
        }
    }
}

// We add our own relationship hooks instead of deriving relationship
// because we don't want to despawn Server if there are no more LinkOfs.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug, Reflect)]
#[component(on_insert = LinkOf::on_insert_hook)]
#[component(on_replace = LinkOf::on_replace)]
pub struct LinkOf {
    pub server: Entity,
}

impl Relationship for LinkOf {
    type RelationshipTarget = Server;
    #[inline(always)]
    fn get(&self) -> Entity {
        self.server
    }
    #[inline]
    fn from(entity: Entity) -> Self {
        Self { server: entity }
    }

    fn set_risky(&mut self, entity: Entity) {
        self.server = entity;
    }
}

impl LinkOf {
    fn on_insert_hook(
        mut world: DeferredWorld,
        HookContext {
            entity,
            caller,
            relationship_hook_mode,
            ..
        }: HookContext,
    ) {
        match relationship_hook_mode {
            RelationshipHookMode::Run => {}
            RelationshipHookMode::Skip => return,
            RelationshipHookMode::RunIfNotLinked => return,
        }
        let target_entity = world.entity(entity).get::<Self>().unwrap().get();
        if target_entity == entity {
            warn!(
                "{}The {}({target_entity:?}) relationship on entity {entity:?} points to itself. The invalid {} relationship has been removed.",
                caller
                    .map(|location| format!("{location}: "))
                    .unwrap_or_default(),
                DebugName::type_name::<Self>(),
                DebugName::type_name::<Self>()
            );
            world.commands().entity(entity).remove::<Self>();
            return;
        }
        if let Ok(mut target_entity_mut) = world.get_entity_mut(target_entity) {
            if let Some(mut relationship_target) = target_entity_mut.get_mut::<Server>() {
                relationship_target.collection_mut_risky().add(entity);
            } else {
                let mut target = <Server as RelationshipTarget>::with_capacity(1);
                target.collection_mut_risky().add(entity);
                world.commands().entity(target_entity).insert(target);
            }
        } else {
            warn!(
                "{}The {}({target_entity:?}) relationship on entity {entity:?} relates to an entity that does not exist. The invalid {} relationship has been removed.",
                caller
                    .map(|location| format!("{location}: "))
                    .unwrap_or_default(),
                DebugName::type_name::<Self>(),
                DebugName::type_name::<Self>()
            );
            world.commands().entity(entity).remove::<Self>();
        }
    }

    fn on_replace(
        mut world: DeferredWorld,
        HookContext {
            entity,
            relationship_hook_mode,
            ..
        }: HookContext,
    ) {
        match relationship_hook_mode {
            RelationshipHookMode::Run => {}
            RelationshipHookMode::Skip => return,
            RelationshipHookMode::RunIfNotLinked => {
                if <Server as RelationshipTarget>::LINKED_SPAWN {
                    return;
                }
            }
        }
        let target_entity = world.entity(entity).get::<Self>().unwrap().get();
        if let Ok(mut target_entity_mut) = world.get_entity_mut(target_entity)
            && let Some(mut relationship_target) = target_entity_mut.get_mut::<Server>()
        {
            RelationshipSourceCollection::remove(
                relationship_target.collection_mut_risky(),
                entity,
            );
        }
    }
}

// ============================================================================
// TransportOf - Simple component linking transport to Server
// ============================================================================

/// Component that links a transport entity to its Server.
///
/// When a transport (UDP, WebTransport, etc.) receives a new client connection,
/// it should spawn a `LinkOf { server: transport_of.server }` pointing to the
/// Server entity, NOT the transport entity itself.
///
/// # Example
/// ```ignore
/// // Transport entity reads its TransportOf to know which Server to use
/// fn on_new_connection(
///     transport_query: Query<&TransportOf>,
/// ) {
///     let transport_of = transport_query.get(transport_entity).unwrap();
///     commands.spawn(LinkOf { server: transport_of.server });
/// }
/// ```
#[derive(Component, Clone, Copy, Debug, Reflect)]
pub struct TransportOf {
    /// The Server entity this transport feeds connections to
    pub server: Entity,
}

impl TransportOf {
    pub fn new(server: Entity) -> Self {
        Self { server }
    }
}

// ============================================================================
// ViaTransport - Tracks which transport entity spawned a client LinkOf
// ============================================================================

/// Component added to `LinkOf` entities to track which transport they came from.
///
/// This is essential for multi-transport setups where multiple transports
/// (UDP, WebTransport, etc.) feed into a single Server. Each transport's
/// protocol layer (e.g., NetcodeServer) needs to know which clients belong to it.
///
/// # Example
/// ```ignore
/// // When spawning a new client connection from a transport:
/// commands.spawn((
///     LinkOf { server: transport_of.server },
///     ViaTransport { transport: transport_entity },
///     Link::new(None),
///     UdpLinkOfIO,  // Transport-specific marker
/// ));
///
/// // Protocol layers can filter clients by transport:
/// fn send(
///     transport_query: Query<(Entity, &NetcodeServer, &TransportOf)>,
///     link_query: Query<(&Link, &ViaTransport), With<LinkOf>>,
/// ) {
///     for (transport_entity, netcode, transport_of) in transport_query.iter() {
///         // Only process clients that came through THIS transport
///         for (link, via) in link_query.iter() {
///             if via.transport == transport_entity {
///                 // This client belongs to this transport
///             }
///         }
///     }
/// }
/// ```
#[derive(Component, Clone, Copy, Debug, Reflect)]
pub struct ViaTransport {
    /// The transport entity that this client connected through
    pub transport: Entity,
}

impl ViaTransport {
    pub fn new(transport: Entity) -> Self {
        Self { transport }
    }
}

pub struct ServerLinkPlugin;
impl Plugin for ServerLinkPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<LinkPlugin>() {
            app.add_plugins(LinkPlugin);
        }
        app.add_observer(Server::unlinked);
    }
}
