use mesh_portal::version::latest::id::Point;
use mesh_portal::version::latest::messaging::{Request, Response};
use mesh_portal::version::latest::msg::MsgMethod;
use mesh_portal::version::latest::payload::{Payload, PayloadType};
use mesh_portal_versions::version::v0_0_1::command::Command;
use mesh_portal_versions::version::v0_0_1::id::id::Port;
use mesh_portal_versions::version::v0_0_1::messaging::Method;
use mesh_portal_versions::version::v0_0_1::service::Global;
use crate::error::Error;
use crate::registry::RegistryApi;

lazy_static! {
    static ref COMMAND_SERVICE_PORT: Port = Point::from_str("GLOBAL::command-service").unwrap().to_port();
}

#[derive(Clone)]
pub struct GlobalApi {
    registry: RegistryApi
}

#[async_trait]
impl Global for GlobalApi {
    async fn handle(&self, request: Request) -> Response {
        if request.to == *COMMAND_SERVICE_PORT {
            self.handle_command_service_request(request).await
        } else {
            request.not_found()
        }
    }
}
impl GlobalApi {

    pub fn new( registry: RegistryApi ) -> Self {
        Self {
            registry
        }
    }

    async fn handle_command_service_request(&self, request: Request) -> Response {
        async fn handle(global: &GlobalApi, request: &Request) -> Result<Response,Error> {
            match &request.core.method {
                Method::Msg(method) if method.as_str() == "Command" && request.core.body.payload_type() == PayloadType::Command => {
                    if let Payload::Command(command) = &request.core.body {
                        match &**command {
                            Command::Create(create) => {
                                let stub = global.registry.create(create).await?.stub;
                                Ok(request.ok_payload(Payload::Stub(stub)))
                            }
                            Command::Delete(delete) => {
                                let list = global.registry.delete(delete).await?;
                                Ok(request.ok_payload(Payload::List(list)))
                            }
                            Command::Select(select) => {
                                let list = global.registry.select(select).await?;
                                Ok(request.ok_payload(Payload::List(list)))
                            }
                            Command::Publish(create) => {
                                let stub = global.registry.create(create).await?.stub;
                                Ok(request.ok_payload(Payload::Stub(stub)))
                            }
                            Command::Set(set) => {
                                global.registry.set(set).await?;
                                Ok(request.ok())
                            }
                            Command::Get(get) => {
                                let payload= global.registry.get(get).await?;
                                Ok(request.ok_payload(payload))
                            }
                        }
                        Ok(request.ok())
                    } else {
                        request.fail("unexpected command body mismatch.  expected Payload(Command)")
                    }
                }
                _ => {
                    request.fail("command service expecting Msg request with method 'Command' and body payload type 'Command'")
                }
            }
        }

        match handle(self, &request).await {
            Ok(response) => response,
            Err(error) => request.fail(error.to_string().as_str() )
        }
    }
}
