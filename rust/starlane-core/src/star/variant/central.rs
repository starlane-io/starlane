use std::future::Future;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use mesh_portal::version::latest::entity::request::create::Strategy;

use tokio::sync::{mpsc, oneshot};
use mesh_portal::version::latest::cli::Transfer;
use crate::command::cli::CliServer;


use crate::error::Error;
use crate::message::StarlaneMessenger;
use crate::particle::{Kind, ParticleLocation, ParticleRecord};
use crate::star::{StarKey, StarSkel};
use crate::star::variant::{FrameVerdict, VariantCall};
use crate::starlane::api::StarlaneApi;
use crate::user::HyperUser;
use crate::util::{AsyncProcessor, AsyncRunner};

static BOOT_BUNDLE_ZIP : &'static [u8] = include_bytes!("../../../boot/bundle.zip");

pub struct CentralVariant {
    skel: StarSkel,
    initialized: bool
}

impl CentralVariant {
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<VariantCall>) {
        AsyncRunner::new(
            Box::new(Self { skel: skel.clone(),
            initialized: false}),
            skel.variant_api.tx.clone(),
            rx,
        );
    }
}

#[async_trait]
impl AsyncProcessor<VariantCall> for CentralVariant {
    async fn process(&mut self, call: VariantCall) {
        match call {
            VariantCall::Init(tx) => {
                self.init_variant(tx);
            }
            VariantCall::Frame { frame, session:_, tx } => {
                tx.send(FrameVerdict::Handle(frame));
            }
        }
    }
}


impl CentralVariant {
    fn init_variant(&mut self, tx: oneshot::Sender<Result<(), Error>>) {
        if self.initialized == true {
            tx.send(Ok(()));
            return;
        } else {
            self.initialized = true;
            let skel = self.skel.clone();
            tokio::spawn( async move {
                match skel.machine.get_starlane_api().await {
                    Ok(api) => {
                        CentralVariant::ensure(api).await;
                        tx.send(Ok(()));
                    }
                    Err(err) => {
                        error!("could not get starlane api for central init: {}", err.to_string());
                        tx.send(Err(err));
                    }
                }
            });
        }
    }
}

impl CentralVariant {

    async fn ensure(starlane_api: StarlaneApi) -> Result<(), Error> {

        let cli = starlane_api.cli();

        let session = cli.session().await?;

        session.exec("create? hyperspace<Space>" ).await;
        session.exec("create? localhost<Space>" ).await;
        session.exec("create? hyperspace:repo<Base<Repo>>").await;
        session.exec("create? hyperspace:repo:boot<ArtifactBundleSeries>").await;
        let content = Arc::new( BOOT_BUNDLE_ZIP.to_vec() );
        session.exec_with_transfers("publish? ^[ bundle.zip ]-> hyperspace:repo:boot:1.0.0", vec![Transfer::new("bundle.zip", content)]).await;
        session.exec("create? hyperspace:users<UserBase<Keycloak>>").await;
        session.exec("create? hyperspace:users:hyperuser<User>").await;

        Ok(())
    }

    /*
    async fn ensure_old(starlane_api: StarlaneApi) -> Result<(), Error> {
info!("CENTRAL ensure!");
        let mut creation = starlane_api.create_space("hyperspace").await?;
        creation.set_strategy(Strategy::Ensure);
        creation.submit().await?;

        let mut creation = starlane_api.create_space("localhost" ).await?;
        creation.set_strategy(Strategy::Ensure);
        creation.submit().await?;

        let (tx,mut rx) = CliServer::new_internal( starlane_api ).await?;

        tx.send(inlet::CliFrame::Line("? create hyperspace:repo<Base<Repo>>".to_string()) ).await?;
        tx.send(inlet::CliFrame::EndRequires ).await?;
        while let Some(frame) = rx.recv().await {
            if let outlet::Frame::End(_) = frame {
                break;
            }
        }
info!("create baes repo!");
        tx.send(inlet::CliFrame::Line("? create hyperspace:repo:boot<ArtifactBundleSeries>".to_string()) ).await?;
        tx.send(inlet::CliFrame::EndRequires ).await?;
        while let Some(frame) = rx.recv().await {
            if let outlet::Frame::End(_) = frame {
                break;
            }
        }
        info!("base repo created...!");

        tx.send(inlet::CliFrame::Line("? publish ^[ bundle.zip ]-> hyperspace:repo:boot:1.0.0".to_string()) ).await?;
        let content = Arc::new( BOOT_BUNDLE_ZIP.to_vec() );
        tx.send(inlet::CliFrame::Transfer { name: "bundle.zip".to_string(), content }).await?;
        tx.send(inlet::CliFrame::EndRequires ).await?;

        while let Some(frame) = rx.recv().await {
            if let outlet::Frame::End(_) = frame {
                break;
            }
        }

        tx.send(inlet::CliFrame::Line("? create hyperspace:users<UserBase<Keycloak>>".to_string()) ).await?;
        tx.send(inlet::CliFrame::EndRequires ).await?;
        while let Some(frame) = rx.recv().await {
            if let outlet::Frame::End(_) = frame {
                break;
            }
        }

        tx.send(inlet::CliFrame::Line("? create hyperspace:users:hyperuser<User>".to_string()) ).await?;
        tx.send(inlet::CliFrame::EndRequires ).await?;

        info!("Done with CENTRAL init");

        Ok(())
    }

     */
}
