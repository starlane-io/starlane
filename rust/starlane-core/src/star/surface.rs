use crate::star::StarSkel;
use crate::util::{Call, AsyncRunner, AsyncProcessor};
use tokio::sync::mpsc;
use starlane_resources::ResourceIdentifier;
use crate::resource::ResourceRecord;
use crate::message::Fail;

pub struct SurfaceApi {
  skel: StarSkel
}

impl SurfaceApi {

    pub fn new(skel: StarSkel) -> Self {
      Self {
          skel
      }
    }

    pub async fn locate(&self, identifier: ResourceIdentifier ) -> Result<ResourceRecord,Fail> {
        self.skel.resource_locator_api.locate(identifier).await
    }
}

