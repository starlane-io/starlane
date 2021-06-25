use crate::error::Error;
use crate::layout::{ConstellationLayout, StarLayout};
use crate::proto::ProtoStar;
use crate::star::{StarKey, StarKind};
use crate::template::{ConstellationTemplate, StarTemplate};

#[async_trait]
pub trait Provisioner {
    async fn constellation(
        &self,
        template: ConstellationTemplate,
        layout: ConstellationLayout,
    ) -> Result<(), Error>;
}
