use std::str::FromStr;
use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};


use crate::error::Error;
use crate::resource::{Kind, ResourceRecord,  ResourceLocation};
use crate::star::{StarKey, StarSkel};
use crate::star::variant::{FrameVerdict, VariantCall};
use crate::starlane::api::StarlaneApi;
use crate::util::{AsyncProcessor, AsyncRunner};
use crate::mesh::serde::generic::resource::ResourceStub;
use crate::mesh::serde::id::Address;
use crate::mesh::serde::resource::command::create::Strategy;
use crate::mesh::serde::resource::Status;

pub struct CentralVariant {
    skel: StarSkel,
}

impl CentralVariant {
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<VariantCall>) {
        AsyncRunner::new(
            Box::new(Self { skel: skel.clone() }),
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
                self.init(tx);
            }
            VariantCall::Frame { frame, session:_, tx } => {
                tx.send(FrameVerdict::Handle(frame));
            }
        }
    }
}


impl CentralVariant {
    fn init(&self, tx: oneshot::Sender<Result<(), Error>>) {

        let skel = self.skel.clone();

        tokio::spawn(async move {
            let starlane_api = StarlaneApi::new(skel.surface_api.clone());
            let result = Self::ensure(starlane_api).await;
            if let Result::Err(error) = result.as_ref() {
                error!("Central Init Error: {}", error.to_string());
            }
            tx.send(result);
        });
    }
}

impl CentralVariant {
    async fn ensure(starlane_api: StarlaneApi) -> Result<(), Error> {

        let mut creation = starlane_api.create_space("space", "Space").await?;
        creation.set_strategy(Strategy::Ensure);
        creation.submit().await?;

        Ok(())
    }
}
