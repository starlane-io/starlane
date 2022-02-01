use std::sync::Arc;
use anyhow::Error;
use mesh_portal_api_client::{ResourceCtrl, ResourceCtrlFactory, ResourceSkel};
use mesh_portal_serde::version::latest::config::{Config, ResourceConfigBody};
use mesh_portal_serde::version::latest::messaging::Response;
use mesh_portal_serde::version::latest::portal::outlet;
use mesh_portal_tcp_client::{PortalClient, PortalTcpClient};
use mesh_portal_tcp_common::{PrimitiveFrameReader, PrimitiveFrameWriter};

pub async fn connect(host: String) -> Result<PortalTcpClient,Error>{
    let client = Box::new(ControlPortalClient::new("hyperuser".to_string()));
    let client = PortalTcpClient::new(host, client).await?;
    Ok(client)
}

pub struct ControlPortalClient {
    pub user: String,
}

impl ControlPortalClient {
    pub fn new(user: String) -> Self {
        Self { user}
    }
}

#[async_trait]
impl PortalClient for ControlPortalClient {
    fn flavor(&self) -> String {
        return "starlane".to_string();
    }

    async fn auth(
        &self,
        reader: &mut PrimitiveFrameReader,
        writer: &mut PrimitiveFrameWriter,
    ) -> Result<(), Error> {
        writer.write_string(self.user.clone()).await?;
        Ok(())
    }


    fn resource_ctrl_factory(&self) ->Arc<dyn ResourceCtrlFactory> {
        Arc::new(ControlCtrlFactory {name: self.user.clone()})
    }

    fn logger(&self) -> fn(m: &str) {
        fn logger(message: &str) {
            println!("{}", message);
        }
        logger
    }
}

pub struct ControlCtrlFactory {
    pub name: String
}

impl ResourceCtrlFactory for ControlCtrlFactory {
    fn matches(&self, config: Config<ResourceConfigBody>) -> bool {
        true
    }

    fn create(&self, skel: ResourceSkel) -> Result<Arc<dyn ResourceCtrl>, Error> {
        Ok(Arc::new(Control {
            name: self.name.clone(),
            skel
        }))
    }
}

pub struct Control {
    pub name: String,
    pub skel: ResourceSkel
}

impl Control {
    pub fn log_str( &self, message: &str) {
        self.log(message.to_string());
    }

    pub fn log( &self, message: String) {
        println!("{} => {}", self.name, message );
    }

}

#[async_trait]
impl ResourceCtrl for Control {
    async fn init(&self) -> Result<(), Error> {
        Ok(())
    }

    async fn outlet_frame(&self, frame: outlet::Frame ) -> Result<Option<Response>,Error> {
        Ok(Option::None)
    }

}