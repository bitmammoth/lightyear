use crate::WebSocketError;
use aeronet_io::Session;
use aeronet_io::connection::{LocalAddr, PeerAddr};
pub use aeronet_websocket::server::{
    Identity, ServerConfig, WebSocketServer, WebSocketServerClient,
};
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use lightyear_aeronet::server::ServerAeronetPlugin;
use lightyear_aeronet::{AeronetLinkOf, AeronetPlugin};
use lightyear_connection::prelude::server::IoServer;
use lightyear_link::prelude::{LinkOf, TransportOf};
use lightyear_link::{Link, LinkStart, Linked, Linking};
use tracing::{info, warn};

/// Allows using [`WebSocketServer`].
pub struct WebSocketServerPlugin;

impl Plugin for WebSocketServerPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<AeronetPlugin>() {
            app.add_plugins(AeronetPlugin);
        }
        if !app.is_plugin_added::<ServerAeronetPlugin>() {
            app.add_plugins(ServerAeronetPlugin);
        }
        app.add_plugins(aeronet_websocket::server::WebSocketServerPlugin);

        app.add_observer(Self::link);
        app.add_observer(Self::on_connection);
    }
}

/// Marker component to identify this LinkOf as coming from WebSocket
#[derive(Component)]
pub struct WebSocketLinkOfIO;

/// WebSocket server IO component.
///
/// This is a transport-only component. It does NOT have a `Server` component.
/// Instead, it should have a `TransportOf { server }` component pointing to
/// the logical Server entity that all client LinkOfs should connect to.
///
/// The [`LocalAddr`] component must be inserted to specify the server_addr.
#[derive(Component)]
#[require(IoServer::websocket())]
pub struct WebSocketServerIo {
    pub config: ServerConfig,
}

impl WebSocketServerPlugin {
    fn link(
        trigger: On<LinkStart>,
        query: Query<
            (Entity, &WebSocketServerIo, Option<&LocalAddr>),
            (Without<Linking>, Without<Linked>),
        >,
        mut commands: Commands,
    ) -> Result {
        if let Ok((entity, io, local_addr)) = query.get(trigger.entity) {
            let server_addr = local_addr.ok_or(WebSocketError::LocalAddrMissing)?.0;
            let config = io.config.clone();
            commands.queue(move |world: &mut World| {
                info!("Server WebSocket starting at {}", server_addr);
                let child = world.spawn((AeronetLinkOf(entity), Name::from("WebSocketServer")));
                WebSocketServer::open(config).apply(child);
            });
        }
        Ok(())
    }

    // TODO: should also add on_connecting? Or maybe it's handled automatically
    //  because the connecting entity adds SessionEndpoint? (and lightyear_aeronet handles that)
    fn on_connection(
        trigger: On<Add, Session>,
        aeronet_query: Query<&AeronetLinkOf>,
        transport_query: Query<&TransportOf>,
        child_query: Query<(&ChildOf, &PeerAddr), With<WebSocketServerClient>>,
        mut commands: Commands,
    ) {
        if let Ok((child_of, peer_addr)) = child_query.get(trigger.entity)
            && let Ok(aeronet_link) = aeronet_query.get(child_of.parent())
        {
            // Get the Server entity from TransportOf
            let server_entity = if let Ok(transport_of) = transport_query.get(aeronet_link.0) {
                transport_of.server
            } else {
                warn!("WebSocketServerIo entity {:?} missing TransportOf component", aeronet_link.0);
                return;
            };
            
            let link_entity = commands
                .spawn((
                    LinkOf { server: server_entity },
                    Link::new(None),
                    PeerAddr(peer_addr.0),
                    WebSocketLinkOfIO,
                ))
                .id();
            commands
                .entity(trigger.entity)
                .insert((AeronetLinkOf(link_entity), Name::from("WebSocketClientOf")));
        }
    }
}
