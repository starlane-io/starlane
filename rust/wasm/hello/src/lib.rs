#![allow(warnings)]

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate cosmic_macros;

use cosmic_macros::handler_sync;
use cosmic_universe::err::UniErr;
use cosmic_universe::log::{PointLogger, RootLogger};
use cosmic_universe::particle::Details;
use cosmic_universe::wave::core::CoreBounce;
use cosmic_universe::wave::exchange::synch::{
    DirectedHandler, InCtx, ProtoTransmitter, ProtoTransmitterBuilder, RootInCtx,
};
use mechtron::err::{GuestErr, MechErr};
use mechtron::guest::GuestSkel;
use mechtron::{guest, Guest, MechtronFactories, MechtronFactory, Platform};
use mechtron::{Mechtron, MechtronLifecycle, MechtronSkel};
use std::marker::PhantomData;
use std::sync::Arc;

#[no_mangle]
pub extern "C" fn mechtron_guest(details: Details) -> Result<Arc<dyn mechtron::Guest>, GuestErr> {
    Ok(Arc::new(mechtron::guest::Guest::new(
        details,
        HelloPlatform::new(),
    )?))
}

#[derive(Clone)]
pub struct HelloPlatform;

impl Platform for HelloPlatform {
    type Err = GuestErr;
    fn factories(&self) -> Result<MechtronFactories<Self>, Self::Err>
    where
        Self: Sized,
    {
        let mut factories = MechtronFactories::new();
        factories.add(HelloFactory::new());
        Ok(factories)
    }
}

impl HelloPlatform {
    pub fn new() -> Self {
        Self {}
    }
}

pub struct HelloFactory {}

impl HelloFactory {
    pub fn new() -> Self {
        Self {}
    }
}

impl<P> MechtronFactory<P> for HelloFactory
where
    P: Platform + 'static,
{
    fn name(&self) -> String {
        "hello".to_string()
    }

    fn lifecycle(&self, skel: MechtronSkel<P>) -> Result<Box<dyn MechtronLifecycle<P>>, P::Err> {
        Ok(Box::new(HelloMechtron::restore(skel, (), ())))
    }

    fn handler(&self, skel: MechtronSkel<P>) -> Result<Box<dyn DirectedHandler>, P::Err> {
        Ok(Box::new(HelloMechtron::restore(skel, (), ())))
    }
}

pub struct HelloMechtron<P>
where
    P: Platform + 'static,
{
    skel: MechtronSkel<P>,
}

impl<P> Mechtron<P> for HelloMechtron<P>
where
    P: Platform + 'static,
{
    type Skel = MechtronSkel<P>;
    type Cache = ();
    type State = ();

    fn restore(skel: Self::Skel, _cache: Self::Cache, _state: Self::State) -> Self {
        HelloMechtron { skel }
    }
}

impl<P> MechtronLifecycle<P> for HelloMechtron<P> where P: Platform + 'static {}

#[handler_sync]
impl<P> HelloMechtron<P>
where
    P: Platform + 'static,
{
    #[route("Ext<Hello>")]
    pub fn hello(&self, _: InCtx<'_, ()>) -> Result<(), P::Err> {
        self.skel.logger.info("Hello World!");
        Ok(())
    }
}

#[cfg(test)]
pub mod test {
    #[test]
    pub fn test() {}
}
