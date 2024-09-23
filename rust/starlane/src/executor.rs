use crate::hyperspace::err::HyperErr;

#[async_trait]
pub trait Executor
where
    Self::Err: HyperErr,
{
    type Args;
    type Err;
    type Spawn;
    async fn execute(&self, args: Self::Args) -> Self::Spawn;
}