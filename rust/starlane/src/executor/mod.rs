pub mod cli;
pub mod dialect;

use crate::err::ThisErr;
use crate::executor::cli::{CliExecutor, CliIn, CliOut};
use crate::executor::cli::os::{CliOsExecutor, OsExeInfo};
use crate::executor::dialect::filestore::FileStore;

#[async_trait]
pub trait Executor
{
    type In;

    type Out;

    async fn execute(&self, args: Self::In ) -> Result<Self::Out,ThisErr>;
}









