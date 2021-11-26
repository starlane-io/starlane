use crate::cache::{ArtifactItem, ArtifactCaches};
use crate::config::mechtron::MechtronConfig;
use crate::config::wasm::Wasm;
use crate::config::bind::{BindConfig};
use crate::error::Error;
use wasm_membrane_host::membrane::WasmMembrane;
use std::sync::Arc;
use starlane_resources::message::{ResourcePortMessage, Message, ResourcePortReply};
use mesh_portal_api::message::Message;
use mechtron_common::{MechtronGuestCall, HostToGuestFrame};
use mechtron_common::version::latest::{guest, host};
use crate::mesh;
use mesh_portal_serde::version::latest;
use mesh_portal_serde::version::v0_0_1::util::ConvertFrom;
use crate::starlane::api::StarlaneApi;

use mesh_portal_serde::version::v0_0_1::id::Address;
use crate::mesh::serde::config::Info;

#[derive(Clone)]
pub struct MechtronShell {
    pub info: Info,
    pub config: ArtifactItem<MechtronConfig>,
    pub wasm: ArtifactItem<Wasm>,
    pub bind_config: ArtifactItem<BindConfig>,
    pub membrane: Arc<WasmMembrane>,
    pub api: StarlaneApi
}

impl MechtronShell {
    pub fn new( api: StarlaneApi, config: ArtifactItem<MechtronConfig>, caches: &ArtifactCaches ) -> Result<Self,Error> {

        let wasm = caches.wasms.get(&config.wasm.path).ok_or(format!("could not get referenced Wasm: {}", config.wasm.path.to_string()) )?;
        let bind_config = caches.bind_configs.get(&config.bind.path).ok_or::<Error>(format!("could not get referenced BindConfig: {}", config.wasm.path.to_string()).into() )?;

        let membrane = WasmMembrane::new_with_init(wasm.module.clone(), "mechtron_init".to_string() )?;

        membrane.init()?;

        Ok(Self{
            config,
            wasm,
            bind_config,
            membrane,
            api
        })
    }

    pub async fn handle(&self, message: Message) -> Result<Option<mesh::serde::portal::inlet::Response>,Error> {
        match message {
            Message::Request(request) => {

                let request = guest::Request{
                    to: self.info.address.clone(),
                    from: request.from,
                    entity: request.entity,
                    exchange: request.exchange
                };

                let frame = guest::Frame::Request(request);

                let frame = bincode::serialize(&rframe)?;
                let frame = self.membrane.write_buffer(&frame)?;
                match self.membrane.instance.exports.get_native_function::<i32,i32>("mechtron_guest_frame"){

                    Ok(func) => {
                        match func.call(frame)
                        {
                            Ok(reply) => {

                                if response > 0 {
                                    let response = self.membrane.consume_buffer(response).unwrap();
                                    let response :host::Response = bincode::deserialize(&response )?;

                                    latest::portal::inlet::Response  {
                                        id: latest::util::unique_id(),
                                        to: request.to,
                                        exchange: "".to_string(),
                                        entity: ()
                                    };
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
            Message::Response(_) => {}
        }
        let call = MechtronGuestCall {
            mechtron: self.config.name.clone(),
            frame: HostToGuestFrame::Message(message)
        };

        let string = serde_json::to_string(&call)?;
info!("{}",string);
        let call = self.membrane.write_string(string.as_str())?;
        info!("message delivery to mechtron complete...{}", call);
        match self.membrane.instance.exports.get_native_function::<i32,i32>("mechtron_guest_frame"){

            Ok(func) => {
                match func.call(call)
                {
                    Ok(response) => {

                        if response > 0 {
                            let response = self.membrane.consume_buffer(response).unwrap();
                            let response: latest::portal::inlet::Response = bincode::deserialize(response.as_str())?;
                            let response = ConvertFrom::convert_from(response)?;
info!("... HOST .... SENDING REPLY......");
                            Ok(Option::Some(response))
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


    /*
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

    }*/



}