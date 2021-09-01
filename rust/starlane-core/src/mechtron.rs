use crate::cache::{ArtifactItem, ArtifactCaches};
use crate::config::mechtron::MechtronConfig;
use crate::config::wasm::Wasm;
use crate::config::bind::BindConfig;
use crate::error::Error;
use wasm_membrane_host::membrane::WasmMembrane;
use std::sync::Arc;
use starlane_resources::message::{ResourcePortMessage, Message};
use mechtron_common::{MechtronCall, MechtronCommand};

#[derive(Clone)]
pub struct Mechtron {
    pub config: ArtifactItem<MechtronConfig>,
    pub wasm: ArtifactItem<Wasm>,
    pub bind_config: ArtifactItem<BindConfig>,
    pub membrane: Arc<WasmMembrane>
}

impl Mechtron {
    pub fn new( config: ArtifactItem<MechtronConfig>, caches: &ArtifactCaches ) -> Result<Self,Error> {

        let wasm = caches.wasms.get(&config.wasm.address ).ok_or(format!("could not get referenced Wasm: {}", config.wasm.address.to_string()) )?;
        let bind_config = caches.bind_configs.get(&config.bind.address ).ok_or::<Error>(format!("could not get referenced BindConfig: {}", config.wasm.address.to_string()).into() )?;

        let membrane = WasmMembrane::new_with_init(wasm.module.clone(), "mechtron_init".to_string() )?;

        membrane.init()?;

        Ok(Self{
            config,
            wasm,
            bind_config,
            membrane
        })
    }

    pub async fn message( &self, message: Message<ResourcePortMessage>) -> Result<(),Error> {
        let call = MechtronCall {
            mechtron: self.config.name.clone(),
            command: MechtronCommand::Message(message)
        };

        let string = serde_json::to_string(&call)?;
info!("{}",string);
        let call = self.membrane.write_string(string.as_str())?;
        info!("message delivery to mechtron complete...{}", call);
        match self.membrane.instance.exports.get_native_function::<i32,()>("mechtron_call"){

            Ok(func) => {
                match func.call(call)
                {
                    Ok(_) => {
                    }
                    Err(error) => {
                        error!("wasm runtime error: {}",error );
                    }
                }

            }
            Err(error) => {
                error!("error when exporting function: mechtron_call" );
            }
        }
        Ok(())
    }
}