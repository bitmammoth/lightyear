//! Plugin to register and handle user inputs.
use crate::action_state::ActionState;
#[cfg(any(feature = "client", feature = "server"))]
use crate::input_message::NativeStateSequence;
use bevy_app::{App, Plugin};
use bevy_ecs::entity::MapEntities;
use bevy_reflect::{FromReflect, Reflectable};
use core::fmt::Debug;
use lightyear_inputs::config::InputConfig;
use lightyear_inputs::input_buffer::InputBuffer;
use serde::Serialize;
use serde::de::DeserializeOwned;

#[derive(Default)]
pub struct InputPlugin<A> {
    pub config: InputConfig<A>,
}

impl<
    A: Serialize
        + DeserializeOwned
        + Clone
        + PartialEq
        + Send
        + Sync
        + Debug
        + Default
        + 'static
        + MapEntities
        + Reflectable
        + FromReflect,
> Plugin for InputPlugin<A>
{
    fn build(&self, app: &mut App) {
        tracing::info!("🔧 InputPlugin<{}> build called", core::any::type_name::<A>());
        app.register_type::<InputBuffer<ActionState<A>, A>>();
        app.register_type::<ActionState<A>>();

        // TODO: for simplicity, we currently register both client and server input plugins if both features are enabled
        #[cfg(feature = "client")]
        {
            tracing::info!("🔧 Adding ClientInputPlugin<NativeStateSequence<{}>>", core::any::type_name::<A>());
            use lightyear_inputs::client::ClientInputPlugin;
            app.add_plugins(ClientInputPlugin::<NativeStateSequence<A>>::new(
                self.config,
            ));
        }
        #[cfg(not(feature = "client"))]
        {
            tracing::warn!("⚠️ ClientInputPlugin NOT added - client feature disabled");
        }

        #[cfg(feature = "server")]
        {
            tracing::info!("🔧 Adding ServerInputPlugin<NativeStateSequence<{}>>", core::any::type_name::<A>());
            use lightyear_inputs::server::ServerInputPlugin;
            app.add_plugins(ServerInputPlugin::<NativeStateSequence<A>> {
                rebroadcast_inputs: self.config.rebroadcast_inputs,
                marker: core::marker::PhantomData,
            });
        }
        #[cfg(not(feature = "server"))]
        {
            tracing::warn!("⚠️ ServerInputPlugin NOT added - server feature disabled");
        }
    }
}
