use std::convert::{TryFrom, TryInto};
use std::sync::Arc;
use mesh_portal_api_client::{ResourceCtrl, ResourceCtrlFactory, ResourceSkel};
use mesh_portal_serde::error::Error;
use mesh_portal_serde::version::latest::artifact::ArtifactRequest;
use mesh_portal_serde::version::latest::config::{Config, ResourceConfigBody};
use mesh_portal_serde::version::latest::frame::PrimitiveFrame;
use mesh_portal_serde::version::latest::id::Address;
use mesh_portal_serde::version::latest::messaging::Response;
use mesh_portal_serde::version::latest::portal;
use mesh_portal_serde::version::latest::portal::{Exchanger, initin, initout};
use mesh_portal_tcp_client::PortalClient;
use mesh_portal_tcp_common::{FrameReader, FrameWriter, PrimitiveFrameReader, PrimitiveFrameWriter};
use mesh_portal_tcp_server::ClientIdent;
use tokio::sync::mpsc;
use crate::artifact::ArtifactRef;
use crate::config::wasm::{Wasm, WasmCompiler};
use crate::mechtron::wasm::{Call, WasmMembraneExt};
use crate::mechtron::WasmMembraneExt;
use crate::resource::ArtifactKind;
use crate::resource::config::Parser;

pub struct MechtronPortalClient {
    pub wasm_address: Address,
}

impl MechtronPortalClient {
    pub fn new(wasm_address: Address) -> Self {
        Self { wasm_address }
    }
}

#[async_trait]
impl PortalClient for MechtronPortalClient {
    fn flavor(&self) -> String {
        return "mechtron".to_string();
    }

    async fn auth(
        &self,
        reader: &mut PrimitiveFrameReader,
        writer: &mut PrimitiveFrameWriter,
    ) -> Result<(), anyhow::Error> {
        let client_ident = ClientIdent {
            user: "none".to_string(),
            portal_key: Option::Some(self.wasm_address.to_string())
        };
        let frame = bincode::serialize(&client_ident )?;
        let frame : PrimitiveFrame = From::from(frame);
        writer.write(frame).await?;
        Ok(())
    }


    fn logger(&self) -> fn(m: &str) {
        fn logger(message: &str) {
            println!("{}", message);
        }
        logger
    }

    async fn init( &self, reader: & mut FrameReader<initout::Frame>, writer: & mut FrameWriter<initin::Frame> ) -> Result<Arc< dyn ResourceCtrlFactory >,Error> {
        let artifact = ArtifactRequest {
            address: self.wasm_address.clone(),
        };
        let request = Exchanger::new(artifact);
        writer.write(initin::Frame::Artifact(request)).await;

        if let initout::Frame::Artifact(response) = reader.read().await? {
            let compiler = WasmCompiler::new();
            let artifact_ref = ArtifactRef{
                address: self.wasm_address.clone(),
                kind: ArtifactKind::Wasm
            };

            let wasm = compiler.parse( artifact_ref, response.payload.bin.clone() )?;
            let wasm_membrane_ext = WasmMembraneExt::new( wasm.module.clone() )?;
            Ok(Arc::new(MechtronResourceCtrlFactory {wasm_membrane_ext}))

        } else {
            return Err(format!("was not able to exchange artifact: '{}'",self.wasm_address.to_string()).into())
        }

    }

}

pub struct MechtronResourceCtrlFactory {
    wasm_membrane_ext: WasmMembraneExt
}

impl ResourceCtrlFactory for MechtronResourceCtrlFactory {
    fn matches(&self, config: Config<ResourceConfigBody>) -> bool {
        true
    }

    fn create(&self, skel: ResourceSkel) -> Result<Arc<dyn ResourceCtrl>, Error> {

        let (tx,rx) = mpsc::channel(1024);
        let skel = MechtronSkel {
            tx ,
            membrane: self.wasm_membrane_ext.clone(),
            resource_skel: skel
        };

        Ok(Arc::new(MechtronResourceCtrl {
            skel,
        }))
    }
}

#[derive(Clone)]
pub struct MechtronSkel {
    pub tx: mpsc::Sender<Call>,
    pub membrane: WasmMembraneExt,
    pub resource_skel: ResourceSkel,
}


pub struct MechtronResourceCtrl {
    pub skel: MechtronSkel,
}

impl MechtronResourceCtrl {
    pub fn log_str( &self, message: &str) {
        self.log(message.to_string());
    }

    pub fn log( &self, message: String) {
        println!("{} => {}", self.name, message );
    }
}

#[async_trait]
impl ResourceCtrl for MechtronResourceCtrl {
    async fn init(&self) -> Result<(), Error> {
        Ok(())
    }

    async fn handle(&self, frame: portal::outlet::Frame ) -> Result<Option<Response>,Error> {
        let frame: mechtron_common::outlet::Frame = TryFrom::try_from(frame)?;
        if let portal::outlet::Frame::Request(request) = frame {
            Ok(Option::Some(self.skel.membrane.handle_request(request)).await)
        } else {
            self.skel.membrane.handle_frame(frame.try_into()?)
        }

        Ok(Option::None)
    }
}
