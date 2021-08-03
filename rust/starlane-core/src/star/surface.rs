use crate::star::StarSkel;
use crate::util::{Call, AsyncRunner, AsyncProcessor};
use tokio::sync::mpsc;
use starlane_resources::ResourceIdentifier;
use crate::resource::ResourceRecord;
use crate::message::{Fail, ProtoStarMessage};
use crate::frame::{ReplyKind, Reply};

#[derive(Clone)]
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

    pub async fn exchange(&self,proto: ProtoStarMessage, expect: ReplyKind, description: &str ) -> Result<Reply,Fail> {
        self.skel.messaging_api.exchange(proto,expect,description).await
    }

}

