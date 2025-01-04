use crate::provider::ProviderKind;
use crate::registry::err::RegErr;
use async_trait::async_trait;
use indexmap::IndexMap;
use starlane_space::particle::Details;
use starlane_space::selector::KindSelector;

#[async_trait]
pub trait ProviderContext: Send + Sync {
    async fn select<'a>(
        &'a self,
        select: &'a mut KindSelector,
    ) -> Result<IndexMap<ProviderKind, Details>, RegErr>;
}
