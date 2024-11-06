use crate::starlane_hyperspace::hyperspace::executor::Executor;
pub mod filestore;

#[derive(Clone, Hash, Eq, PartialEq)]
pub enum ExecutorDialect {
    FileStore
}




