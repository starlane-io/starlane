pub mod cli;
pub mod dialect;

use crate::err::ThisErr;
use crate::host::Host;

#[async_trait]
pub trait Executor
{
    type In;

    type Out;

    async fn execute(&self, args: Self::In ) -> Result<Self::Out,ThisErr>;
}

pub enum ExecKind {
    Host(Host)
}







