use crate::base::provider::ProviderKindDisc;
use crate::registry::err::RegErr;
use async_trait::async_trait;
use indexmap::IndexMap;
use starlane_space::particle::Details;
use starlane_space::selector::KindSelector;
use crate::base::Foundation;

#[derive(Clone,Debug)]
pub struct FoundationContext<F> where F: Foundation {
    foundation: F
}

#[async_trait]
pub trait ProviderContext: Send + Sync {
    async fn select<'a>(
        &'a self,
        select: &'a mut KindSelector,
    ) -> Result<IndexMap<ProviderKindDisc, Details>, RegErr>;
}
