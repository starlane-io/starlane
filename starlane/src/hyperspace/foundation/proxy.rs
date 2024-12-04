use std::ops::Deref;
use std::sync::Arc;
use tokio::sync::watch::Receiver;
use crate::hyperspace::foundation;
use crate::hyperspace::foundation::config::{Config, ProviderConfig};
use crate::hyperspace::foundation::err::FoundationErr;
use crate::hyperspace::foundation::kind::{DependencyKind, FoundationKind};
use crate::hyperspace::foundation::proxy::sealed::ProxySealed;
use crate::hyperspace::foundation::status::Status;
use crate::hyperspace::reg::Registry;
use crate::space::progress::Progress;

pub trait Proxy<T>: Deref<Target = T> { }

pub struct Foundation<F> where F: foundation::Foundation{
    orig: Box<dyn foundation::Foundation<Config=F::Config, Dependency=F::Dependency, Provider=F::Provider>>
}

impl <F> Deref for Foundation<F> where F: foundation::Foundation {
    type Target = Arc<dyn foundation::Foundation<Config=F::Config, Dependency=F::Dependency, Provider=F::Provider>>;

    fn deref(&self) -> &Self::Target {
        &self.orig
    }
}

impl <F> Proxy<F> for Foundation<F> where F: foundation::Foundation{

}

impl <F> foundation::Foundation for Foundation<F> where F: foundation::Foundation {
    type Config = F::Config;
    type Dependency = F::Dependency;
    type Provider = F::Provider;

    fn kind(&self) -> FoundationKind {
    }

    fn config(&self) -> Self::Config {
        todo!()
    }

    fn status(&self) -> Status {
        todo!()
    }

    fn status_watcher(&self) -> Arc<Receiver<Status>> {
        todo!()
    }

    async fn synchronize(&self, progress: Progress) -> Result<Status, FoundationErr> {
        todo!()
    }

    async fn install(&self, progress: Progress) -> Result<(), FoundationErr> {
        todo!()
    }

    fn dependency(&self, kind: &DependencyKind) -> Result<Option<Self::Dependency>, FoundationErr> {
        todo!()
    }

    fn registry(&self) -> Result<Registry, FoundationErr> {
        todo!()
    }
}



pub(super) mod sealed {
    pub trait ProxySealed<T> {
        fn get(&self) -> &T;
    }
}

