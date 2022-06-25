use std::str::FromStr;
use std::sync::Arc;
use mesh_portal::version::latest::id::Point;
use mesh_portal::version::latest::messaging::{Agent, ReqProto, ReqShell, RespShell};
use mesh_portal::version::latest::msg::MsgMethod;
use mesh_portal::version::latest::payload::{Substance, PayloadType};
use mesh_portal_versions::version::v0_0_1::command::Command;
use mesh_portal_versions::version::v0_0_1::id::id::Port;
use mesh_portal_versions::version::v0_0_1::id::id::ToPort;
use mesh_portal_versions::version::v0_0_1::wave::{AsyncTransmitter, AsyncTransmitterWithAgent,  Method};
use mesh_portal_versions::version::v0_0_1::service::Global;
use crate::error::Error;
use crate::registry::RegistryApi;

lazy_static! {
    static ref COMMAND_SERVICE_PORT: Port = Point::from_str("GLOBAL::command-service").unwrap().to_port();
}

#[derive(Clone)]
pub struct GlobalApi {
    registry: RegistryApi,
    transmitter: Arc<dyn AsyncTransmitter>
}

#[async_trait]
impl Global for GlobalApi {
    async fn handle(&self, request: ReqShell) -> RespShell {
        if request.to == *COMMAND_SERVICE_PORT {
            self.handle_command_service_request(request).await
        } else {
            request.not_found()
        }
    }
}
impl GlobalApi {

    pub fn new(registry: RegistryApi, transmitter: Arc<dyn AsyncTransmitter> ) -> Self {
        //let transmitter = AsyncTransmitterWithAgent::new( Agent::Point(Point::global_executor()), Point::global_executor().to_port(), transmitter );
        Self {
            registry,
            transmitter
        }
    }

    async fn handle_command_service_request(&self, request: ReqShell) -> RespShell {
        async fn handle(global: &GlobalApi, request: ReqShell) -> Result<RespShell,Error> {
            match &request.core.method {
                Method::Msg(method) if method.as_str() == "Command" && request.core.body.kind() == PayloadType::Command => {
                    if let Substance::Command(command) = &request.core.body {
                        match &**command {
                            Command::Create(create) => {
                                let mut response = {
                                    let mut request = request.clone();
                                    request.to = create.template.point.parent.clone().to_port();
                                    global.transmitter.send(request).await
                                };
                                response.from = Point::global_executor().to_port();
                                Ok(response)
                            }
                            Command::Delete(delete) => {
                                let list = global.registry.delete(delete).await?;
                                Ok(request.ok_body(Substance::List(list)))
                            }
                            Command::Select(select) => {
                                let list = global.registry.select(select).await?;
                                Ok(request.ok_body(Substance::List(list)))
                            }
                            Command::Set(set) => {
                                global.registry.set(set).await?;
                                Ok(request.ok())
                            }
                            Command::Get(get) => {
                                let payload= global.registry.get(get).await?;
                                Ok(request.ok_body(payload))
                            }
                            Command::Update(_) => {
                                Ok(request.status(400))
                            }
                            Command::Read(_) => {
                                Ok(request.status(400))
                            }
                        }
                    } else {
                        Ok(request.fail("unexpected command body mismatch.  expected Payload(Command)"))
                    }
                }
                _ => {
                    Ok(request.fail("command service expecting Msg request with method 'Command' and body payload type 'Command'"))
                }
            }
        }

        match handle(self, request.clone()).await {
            Ok(response) => response,
            Err(error) => request.fail(error.to_string().as_str() )
        }
    }
}
