/*use std::convert::{TryFrom, TryInto};
use std::sync::{Arc, mpsc};
use anyhow::anyhow;
use mesh_portal::version::latest::artifact::ArtifactRequest;
use mesh_portal::version::latest::config::{ParticleConfigBody, PointConfig};
use mesh_portal::version::latest::entity::response::RespCore;
use mesh_portal::version::latest::frame::PrimitiveFrame;
use mesh_portal::version::latest::id::Point;
use mesh_portal::version::latest::log::{LogSource, PointLogger, RootLogger};
use mesh_portal::version::latest::messaging::{ReqShell, RespShell};
use mesh_portal::version::latest::portal;
use mesh_portal::version::latest::portal::{Exchanger, initin, initout};
use mesh_portal::version::latest::portal::initin::PortalAuth;
use mesh_portal::version::latest::portal::initout::Frame;
use mesh_portal::version::latest::portal::outlet::RequestFrame;
use mesh_portal_tcp_client::{PortalClient, PortalTcpClient};
use mesh_portal_tcp_common::{FrameReader, FrameWriter, PrimitiveFrameReader, PrimitiveFrameWriter};
use crate::artifact::ArtifactRef;
use crate::config::config::ParticleConfig;
use crate::config::wasm::{Wasm, WasmCompiler};
use crate::error::Error;
use crate::mechtron::wasm::WasmMembraneExt;
use crate::particle::ArtifactSubKind;
use crate::particle::config::Parser;


pub async fn launch_mechtron_client(server: String, wasm_src: Point, point: Point) -> Result<(),Error> {
    let root_logger = RootLogger::stdout(LogSource::Core);
    let logger = root_logger.point(point.clone());
    let client = Box::new( MechtronPortalClient::new(wasm_src,point, logger ));
    let client = PortalTcpClient::new(server, client).await?;
    let mut close_rx = client.close_tx.subscribe();
    close_rx.recv().await;
    Ok(())
}

pub struct MechtronPortalClient {
    pub wasm_src: Point,
    pub point: Point,
    pub logger: PointLogger
}

impl MechtronPortalClient {
    pub fn new(wasm_src: Point, point: Point, logger: PointLogger ) -> Self {
        Self { wasm_src, point, logger  }
    }
}

#[async_trait]
impl PortalClient for MechtronPortalClient {
    fn flavor(&self) -> String {
        return "mechtron".to_string();
    }

    fn auth(&self) -> PortalAuth {
        PortalAuth {
            user: "none".to_string(),
            portal_key: Option::Some(self.wasm_src.to_string())
        }
    }


    fn logger(&self) -> PointLogger {
        self.logger.clone()
    }

    async fn init(&self, reader: &mut mesh_portal_tcp_common::FrameReader<mesh_portal::version::latest::portal::initout::Frame>, writer: &mut mesh_portal_tcp_common::FrameWriter<mesh_portal::version::latest::portal::initin::Frame>, skel: mesh_portal_api_client::PrePortalSkel) -> Result<Arc<dyn mesh_portal_api_client::ParticleCtrlFactory>, anyhow::Error> {
         let artifact = ArtifactRequest {
             point: self.wasm_src.clone(),
         };
         let request = Exchanger::new(artifact);
         writer.write(initin::Frame::Artifact(request)).await?;
 println!("client init: Artifact Requested.");

         if let initout::Frame::Artifact(response) = reader.read().await? {
 println!("client init: Artifact received.");
             let compiler = WasmCompiler::new();
             let artifact_ref = ArtifactRef{
                 point: self.wasm_src.clone(),
                 kind: ArtifactSubKind::Wasm
             };

 println!("client init:parsing wasm: ");
             let wasm = compiler.parse( artifact_ref, response.payload.clone() )?;
             let wasm_membrane_ext = WasmMembraneExt::new( wasm.module.clone(), self.wasm_src.to_string(), skel )?;
 println!("client init: wasm parsed ");
             Ok(Arc::new(MechtronResourceCtrlFactory {wasm_membrane_ext}))

         } else {
             eprintln!("was not able to exchange artifact: '{}'",self.wasm_src.to_string());
             return Err(anyhow!("was not able to exchange artifact: '{}'",self.wasm_src.to_string()).into())
         }

     }

}

pub struct MechtronResourceCtrlFactory {
    wasm_membrane_ext: WasmMembraneExt
}

impl ParticleCtrlFactory for MechtronResourceCtrlFactory {
    fn matches(&self, config: PointConfig<ParticleConfigBody>) -> bool {
        true
    }

    fn create(&self, skel: ParticleSkel) -> Result<Arc<dyn ParticleCtrl>, anyhow::Error> {

        let skel = MechtronSkel {
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
    pub membrane: WasmMembraneExt,
    pub resource_skel: ParticleSkel,
}


pub struct MechtronResourceCtrl {
    pub skel: MechtronSkel,
}

impl MechtronResourceCtrl {
    pub fn log_str( &self, message: &str) {
        self.log(message.to_string());
    }

    pub fn log( &self, message: String) {
        println!("{} => {}", self.skel.resource_skel.assign.details.stub.point.to_string(), message );
    }
}

#[async_trait]
impl ParticleCtrl for MechtronResourceCtrl {
    async fn init(&self) -> Result<(), anyhow::Error> {
        let assign = self.skel.resource_skel.assign.clone();
        let frame = mechtron_common::outlet::Frame::Assign(assign);
        self.skel.membrane.handle_outlet_frame(frame);
        Ok(())
    }

    async fn handle_request( &self, request: RequestFrame ) -> RespCore {
        let response = self.skel.membrane.handle_outlet_request(request.request).await;
        response.core
    }

}


 */
