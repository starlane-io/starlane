use crate::cache::{ArtifactItem, ArtifactCaches};
use crate::config::mechtron::MechtronConfig;
use crate::config::wasm::Wasm;
use crate::config::bind::BindConfig;
use crate::error::Error;
use wasm_membrane_host::membrane::WasmMembrane;
use std::sync::Arc;
use starlane_resources::message::{ResourcePortMessage, Message, ResourcePortReply};
use mechtron_common::{MechtronCall, MechtronCommand, MechtronResponse};
use starlane_resources::http::{HttpRequest, HttpResponse};

#[derive(Clone)]
pub struct Mechtron {
    pub config: ArtifactItem<MechtronConfig>,
    pub wasm: ArtifactItem<Wasm>,
    pub bind_config: ArtifactItem<BindConfig>,
    pub membrane: Arc<WasmMembrane>
}

impl Mechtron {
    pub fn new( config: ArtifactItem<MechtronConfig>, caches: &ArtifactCaches ) -> Result<Self,Error> {

        let wasm = caches.wasms.get(&config.wasm.path).ok_or(format!("could not get referenced Wasm: {}", config.wasm.path.to_string()) )?;
        let bind_config = caches.bind_configs.get(&config.bind.path).ok_or::<Error>(format!("could not get referenced BindConfig: {}", config.wasm.path.to_string()).into() )?;

        let membrane = WasmMembrane::new_with_init(wasm.module.clone(), "mechtron_init".to_string() )?;

        membrane.init()?;

        Ok(Self{
            config,
            wasm,
            bind_config,
            membrane
        })
    }

    pub async fn message( &self, message: Message<ResourcePortMessage>) -> Result<Option<ResourcePortReply>,Error> {
        let call = MechtronCall {
            mechtron: self.config.name.clone(),
            command: MechtronCommand::Message(message)
        };

        let string = serde_json::to_string(&call)?;
info!("{}",string);
        let call = self.membrane.write_string(string.as_str())?;
        info!("message delivery to mechtron complete...{}", call);
        match self.membrane.instance.exports.get_native_function::<i32,i32>("mechtron_call"){

            Ok(func) => {
                match func.call(call)
                {
                    Ok(reply) => {

                        if reply > 0 {
                            let reply_json = self.membrane.consume_string(reply).unwrap();
                            let reply:ResourcePortReply = serde_json::from_str(reply_json.as_str())?;
info!("... HOST .... SENDING REPLY......");
                            Ok(Option::Some(reply))
                        }
                        else {
                            Ok(Option::None)
                        }

                    }
                    Err(error) => {
                        error!("wasm runtime error: {}",error );
                        Err("wasm runtime error".into())
                    }
                }
            }
            Err(error) => {
                error!("error when exporting function: mechtron_call" );
                Err("wasm export error".into())
            }
        }

    }


    pub async fn http_request( &self, message: Message<HttpRequest>) -> Result<Option<HttpResponse>,Error> {
        let call = MechtronCall {
            mechtron: self.config.name.clone(),
            command: MechtronCommand::HttpRequest(message)
        };

        let string = serde_json::to_string(&call)?;
        info!("{}",string);
        let call = self.membrane.write_string(string.as_str())?;
        info!("message delivery to mechtron complete...{}", call);
        match self.membrane.instance.exports.get_native_function::<i32,i32>("mechtron_call"){

            Ok(func) => {
                match func.call(call)
                {
                    Ok(reply) => {

                        if reply > 0 {
                            let reply_json = self.membrane.consume_string(reply).unwrap();
                            let reply:MechtronResponse = serde_json::from_str(reply_json.as_str())?;
                            if let MechtronResponse::HttpResponse(reply)= reply {
                                info!("... HOST .... SENDING REPLY......");
                                Ok(Option::Some(reply))
                            }
                            else {
                                error!("MechtronResponse::PortReply not expected!");
                                Ok(Option::None)
                            }
                        }
                        else {
                            Ok(Option::None)
                        }

                    }
                    Err(error) => {
                        error!("wasm runtime error: {}",error );
                        Err("wasm runtime error".into())
                    }
                }
            }
            Err(error) => {
                error!("error when exporting function: mechtron_call" );
                Err("wasm export error".into())
            }
        }

    }



}