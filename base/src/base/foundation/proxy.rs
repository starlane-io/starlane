use crate::base::config::Config;
use crate::base::config::ProviderConfig;
use crate::base::err::BaseErr;
use crate::base::foundation;
use crate::base::foundation::kind::FoundationKind;
use crate::base::foundation::proxy::sealed::ProxySealed;
use crate::base::foundation::status::Status;
use crate::base::kind::DependencyKind;
use starlane_hyperspace::reg::Registry;
use crate::space::progress::Progress;
use std::ops::Deref;
use std::sync::Arc;
use tokio::sync::watch::Receiver;

pub trait Proxy<T>: Deref<Target=T> {}

pub struct Foundation<F>
where
    F: foundation::Foundation,
{
    orig: Box<dyn foundation::Foundation<Config=F::Config, Dependency=F::Dependency, Provider=F::Provider>>,
}

impl<F> Deref for Foundation<F>
where
    F: foundation::Foundation,
{
    type Target = Arc<dyn foundation::Foundation<Config=F::Config, Dependency=F::Dependency, Provider=F::Provider>>;

    fn deref(&self) -> &Self::Target {
        &self.orig
    }
}


impl<F> foundation::Foundation for Foundation<F>
where
    F: foundation::Foundation,
{
    type Config = F::Config;
    type Dependency = F::Dependency;
    type Provider = F::Provider;

    fn kind(&self) -> FoundationKind {}

    fn config(&self) -> Self::Config {
        todo!()
    }

    fn status(&self) -> Status {
        todo!()
    }

    fn status_watcher(&self) -> Arc<Receiver<Status>> {
        todo!()
    }

    async fn synchronize(&self, progress: Progress) -> Result<Status, BaseErr> {
        todo!()
    }

    async fn install(&self, progress: Progress) -> Result<(), BaseErr> {
        todo!()
    }

    fn dependency(&self, kind: &DependencyKind) -> Result<Option<Self::Dependency>, BaseErr> {
        todo!()
    }

    fn registry(&self) -> Result<Registry, BaseErr> {
        todo!()
    }
}


pub(crate) mod sealed {
    pub trait ProxySealed<T> {
        fn get(&self) -> &T;
    }
}

