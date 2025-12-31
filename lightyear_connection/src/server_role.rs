//! ServerRole Resource
//!
//! This module introduces the concept of a "logical server" that exists independently
//! of the IO transport layer. This allows a single server application to have multiple
//! transport types (UDP, WebTransport, Steam, etc.) while maintaining a unified server identity.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    ServerRole (Resource)                     │
//! │  - Single logical server identity                           │
//! │  - Owns shared state (timeline, replication config, etc.)   │
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!          ┌───────────────────┼───────────────────┐
//!          │                   │                   │
//!          ▼                   ▼                   ▼
//!    ┌──────────┐        ┌──────────┐        ┌──────────┐
//!    │ IoServer │        │ IoServer │        │ IoServer │
//!    │  (UDP)   │        │ (Steam)  │        │  (WT)    │
//!    │ + Server │        │ + Server │        │ + Server │
//!    └──────────┘        └──────────┘        └──────────┘
//!          │                   │                   │
//!          ▼                   ▼                   ▼
//!    ┌──────────┐        ┌──────────┐        ┌──────────┐
//!    │ LinkOf   │        │ LinkOf   │        │ LinkOf   │
//!    │ (client) │        │ (client) │        │ (client) │
//!    └──────────┘        └──────────┘        └──────────┘
//! ```
//!
//! # Usage
//!
//! The `ServerRole` resource is automatically inserted when any `Server` component is added
//! if it doesn't already exist. Systems that need server-level state should query the
//! `ServerRole` resource rather than individual `Server` entities.
//!
//! For IO-specific behavior, query the `Server` entities directly with their transport-specific
//! marker components (e.g., `UdpServerIo`, `SteamServerIo`).

use bevy_ecs::prelude::*;
use bevy_reflect::Reflect;

/// The logical server role for the application.
///
/// This resource represents the "server" from a game logic perspective, independent
/// of how many transport types are being used. There should only ever be one `ServerRole`
/// per application.
///
/// # When to use ServerRole vs Server entity
///
/// - Use `ServerRole` (this resource) when you need:
///   - Server-wide configuration
///   - To check if the app is running as a server
///   - Server-level statistics aggregated across all transports
///
/// - Use `Server` entities when you need:
///   - To iterate over all connected clients (via `LinkOf` relationships)
///   - Transport-specific configuration
///   - To start/stop specific transports
#[derive(Resource, Debug, Default, Reflect)]
pub struct ServerRole {
    /// The current state of the server
    pub state: ServerRoleState,
}

/// The state of the logical server
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum ServerRoleState {
    /// Server is not running (no transports started)
    #[default]
    Stopped,
    /// Server is starting (at least one transport is starting)
    Starting,
    /// Server is running (at least one transport is started)
    Running,
    /// Server is stopping (all transports are stopping)
    Stopping,
}

impl ServerRole {
    /// Create a new ServerRole
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if the server is running
    pub fn is_running(&self) -> bool {
        self.state == ServerRoleState::Running
    }

    /// Check if the server is stopped
    pub fn is_stopped(&self) -> bool {
        self.state == ServerRoleState::Stopped
    }
}

/// Marker component to identify an entity as an IO transport server.
///
/// This is separate from the `Server` component which manages the `LinkOf` relationships.
/// `IoServer` indicates that this entity is responsible for a specific transport type.
///
/// # Example
///
/// ```ignore
/// // A UDP server entity would have:
/// // - Server (manages LinkOf relationships)
/// // - IoServer (marks it as an IO transport)
/// // - UdpServerIo (UDP-specific configuration)
/// // - Started/Starting/Stopped (connection state)
/// ```
#[derive(Component, Debug, Default, Reflect)]
pub struct IoServer {
    /// Human-readable name for this transport (e.g., "UDP", "Steam", "WebTransport")
    pub transport_name: &'static str,
}

impl IoServer {
    pub fn new(transport_name: &'static str) -> Self {
        Self { transport_name }
    }

    pub fn udp() -> Self {
        Self::new("UDP")
    }

    pub fn steam() -> Self {
        Self::new("Steam")
    }

    pub fn webtransport() -> Self {
        Self::new("WebTransport")
    }

    pub fn websocket() -> Self {
        Self::new("WebSocket")
    }

    pub fn crossbeam() -> Self {
        Self::new("Crossbeam")
    }
}

/// Plugin that manages the ServerRole resource
pub struct ServerRolePlugin;

impl bevy_app::Plugin for ServerRolePlugin {
    fn build(&self, app: &mut bevy_app::App) {
        // Initialize the resource if it doesn't exist
        app.init_resource::<ServerRole>();

        // Update ServerRole state based on Server entity states
        app.add_observer(Self::on_server_starting);
        app.add_observer(Self::on_server_started);
        app.add_observer(Self::on_server_stopping);
        app.add_observer(Self::on_server_stopped);
    }
}

impl ServerRolePlugin {
    /// When any Server entity starts starting, update ServerRole
    fn on_server_starting(
        _trigger: On<Add, super::server::Starting>,
        mut server_role: ResMut<ServerRole>,
        query: Query<(), (With<lightyear_link::prelude::Server>, With<super::server::Started>)>,
    ) {
        // If no servers are already running, set to Starting
        if query.is_empty() && server_role.state == ServerRoleState::Stopped {
            server_role.state = ServerRoleState::Starting;
        }
    }

    /// When any Server entity becomes Started, update ServerRole to Running
    fn on_server_started(
        _trigger: On<Add, super::server::Started>,
        mut server_role: ResMut<ServerRole>,
    ) {
        server_role.state = ServerRoleState::Running;
    }

    /// When a Server entity starts stopping
    fn on_server_stopping(
        _trigger: On<Add, super::server::Stopping>,
        mut server_role: ResMut<ServerRole>,
        started_query: Query<
            (),
            (
                With<lightyear_link::prelude::Server>,
                With<super::server::Started>,
            ),
        >,
    ) {
        // Only set to Stopping if no other servers are still running
        if started_query.is_empty() {
            server_role.state = ServerRoleState::Stopping;
        }
    }

    /// When a Server entity becomes Stopped
    fn on_server_stopped(
        _trigger: On<Add, super::server::Stopped>,
        mut server_role: ResMut<ServerRole>,
        running_query: Query<
            (),
            (
                With<lightyear_link::prelude::Server>,
                Or<(
                    With<super::server::Started>,
                    With<super::server::Starting>,
                )>,
            ),
        >,
    ) {
        // Only set to Stopped if no other servers are running or starting
        if running_query.is_empty() {
            server_role.state = ServerRoleState::Stopped;
        }
    }
}

/// Run condition: returns true if the application has a ServerRole and it's running
pub fn is_server_running(server_role: Option<Res<ServerRole>>) -> bool {
    server_role.map(|r| r.is_running()).unwrap_or(false)
}

/// Run condition: returns true if there is a ServerRole resource
pub fn has_server_role(server_role: Option<Res<ServerRole>>) -> bool {
    server_role.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_role_default() {
        let role = ServerRole::default();
        assert_eq!(role.state, ServerRoleState::Stopped);
        assert!(role.is_stopped());
        assert!(!role.is_running());
    }

    #[test]
    fn test_io_server_constructors() {
        assert_eq!(IoServer::udp().transport_name, "UDP");
        assert_eq!(IoServer::steam().transport_name, "Steam");
        assert_eq!(IoServer::webtransport().transport_name, "WebTransport");
    }
}
