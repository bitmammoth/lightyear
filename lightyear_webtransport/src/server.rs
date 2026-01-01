use crate::WebTransportError;
use aeronet_io::Session;
use aeronet_io::connection::{LocalAddr, PeerAddr};
use aeronet_webtransport::server::{
    ServerConfig, SessionRequest, SessionResponse, WebTransportServer, WebTransportServerClient,
};
use aeronet_webtransport::wtransport::Identity;
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use core::time::Duration;
use lightyear_aeronet::server::ServerAeronetPlugin;
use lightyear_aeronet::{AeronetLinkOf, AeronetPlugin};
use lightyear_connection::prelude::server::IoServer;
use lightyear_link::prelude::{LinkOf, TransportOf, ViaTransport};
use lightyear_link::{Link, LinkStart, Linked, Linking};
use tracing::{info, warn};

/// Allows using [`WebTransportServer`].
pub struct WebTransportServerPlugin;

impl Plugin for WebTransportServerPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<AeronetPlugin>() {
            app.add_plugins(AeronetPlugin);
        }
        if !app.is_plugin_added::<ServerAeronetPlugin>() {
            app.add_plugins(ServerAeronetPlugin);
        }
        app.add_plugins(aeronet_webtransport::server::WebTransportServerPlugin);

        app.add_observer(Self::link);
        app.add_observer(Self::on_session_request);
        app.add_observer(Self::on_connection);
    }
}

/// Marker component to identify this LinkOf as coming from WebTransport
#[derive(Component)]
pub struct WebTransportLinkOfIO;

/// WebTransport server IO component.
///
/// This is a transport-only component. It does NOT have a `Server` component.
/// Instead, it should have a `TransportOf { server }` component pointing to
/// the logical Server entity that all client LinkOfs should connect to.
///
/// The [`LocalAddr`] component must be inserted to specify the server_addr.
///
/// When a client attempts to connect, the server will trigger a
/// [`SessionRequest`]. Your app **must** observe this, and use
/// [`SessionRequest::respond`] to set how the server should respond to this
/// connection attempt.
///
/// # Example
/// ```ignore
/// // Spawn the logical server first
/// let server = commands.spawn(Server::default()).id();
///
/// // Then spawn the WebTransport transport pointing to it
/// commands.spawn((
///     WebTransportServerIo { certificate },
///     TransportOf::new(server),
///     LocalAddr(addr),
/// ));
/// ```
#[derive(Debug, Component)]
#[require(IoServer::webtransport())]
pub struct WebTransportServerIo {
    pub certificate: Identity,
}

impl WebTransportServerPlugin {
    fn link(
        trigger: On<LinkStart>,
        query: Query<
            (Entity, &WebTransportServerIo, Option<&LocalAddr>),
            (Without<Linking>, Without<Linked>),
        >,
        mut commands: Commands,
    ) -> Result {
        if let Ok((entity, io, local_addr)) = query.get(trigger.entity) {
            let server_addr = local_addr.ok_or(WebTransportError::LocalAddrMissing)?.0;
            let certificate = io.certificate.clone_identity();
            commands.queue(move |world: &mut World| {
                let config = ServerConfig::builder()
                    .with_bind_address(server_addr)
                    .with_identity(certificate)
                    .keep_alive_interval(Some(Duration::from_secs(1)))
                    .max_idle_timeout(Some(Duration::from_secs(5)))
                    .expect("should be a valid idle timeout")
                    .build();
                info!("Server WebTransport starting at {}", server_addr);
                let child = world.spawn((AeronetLinkOf(entity), Name::from("WebTransportServer")));
                WebTransportServer::open(config).apply(child);
            });
        }
        Ok(())
    }

    fn on_session_request(mut request: On<SessionRequest>) {
        request.respond(SessionResponse::Accepted);
    }

    // TODO: should also add on_connecting? Or maybe it's handled automatically
    //  because the connecting entity adds SessionEndpoint? (and lightyear_aeronet handles that)
    fn on_connection(
        trigger: On<Add, Session>,
        aeronet_query: Query<&AeronetLinkOf>,
        transport_query: Query<&TransportOf>,
        child_query: Query<(&ChildOf, &PeerAddr), With<WebTransportServerClient>>,
        mut commands: Commands,
    ) {
        if let Ok((child_of, peer_addr)) = child_query.get(trigger.entity)
            && let Ok(aeronet_link) = aeronet_query.get(child_of.parent())
        {
            // aeronet_link.0 is the WebTransportServerIo entity
            // We need to get its TransportOf to find the actual Server entity
            let transport_entity = aeronet_link.0;
            let server_entity = if let Ok(transport_of) = transport_query.get(transport_entity) {
                transport_of.server
            } else {
                warn!("WebTransportServerIo entity {:?} missing TransportOf component", transport_entity);
                return;
            };
            
            let link_entity = commands
                .spawn((
                    LinkOf { server: server_entity },
                    ViaTransport { transport: transport_entity },
                    Link::new(None),
                    PeerAddr(peer_addr.0),
                    WebTransportLinkOfIO,
                ))
                .id();
            info!(?link_entity, ?server_entity, ?transport_entity, "WebTransport client connected, spawning LinkOf");
            commands.entity(trigger.entity).insert((
                AeronetLinkOf(link_entity),
                Name::from("WebTransportClientOf"),
            ));
        }
    }
}
