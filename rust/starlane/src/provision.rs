use crate::star::{StarKey, StarKind};
use crate::error::Error;
use crate::layout::{StarLayout, ConstellationLayout};
use crate::proto::ProtoStar;
use crate::template::{StarTemplate, ConstellationTemplate};

#[async_trait]
pub trait Provisioner
{
  async fn constellation(&self, template: ConstellationTemplate, layout: ConstellationLayout ) -> Result<(),Error>;
}

