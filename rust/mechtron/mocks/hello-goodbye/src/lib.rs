#![allow(warnings)]

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate cosmic_macros;
//mod html;

use cosmic_macros::handler_sync;
use cosmic_space::err::SpaceErr;
use cosmic_space::log::{PointLogger, RootLogger};
use cosmic_space::particle::Details;
use cosmic_space::wave::core::CoreBounce;
use cosmic_space::substance::Substance;
use cosmic_space::wave::exchange::synch::{
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
        MyPlatform::new(),
    )?))
}

#[derive(Clone)]
pub struct MyPlatform;

impl Platform for MyPlatform {
    type Err = GuestErr;
    fn factories(&self) -> Result<MechtronFactories<Self>, Self::Err>
    where
        Self: Sized,
    {
        let mut factories = MechtronFactories::new();
        factories.add(HelloGoodbyeMechtronFactory::new());
        Ok(factories)
    }
}

impl MyPlatform {
    pub fn new() -> Self {
        Self {}
    }
}

pub struct HelloGoodbyeMechtronFactory {}

impl HelloGoodbyeMechtronFactory {
    pub fn new() -> Self {
        Self {}
    }
}

impl<P> MechtronFactory<P> for HelloGoodbyeMechtronFactory
where
    P: Platform + 'static,
{
    fn name(&self) -> String {
        "hello-goodbye".to_string()
    }

    fn lifecycle(&self, skel: MechtronSkel<P>) -> Result<Box<dyn MechtronLifecycle<P>>, P::Err> {
        Ok(Box::new(HelloGoodbyeMechtron::restore(skel, (), ())))
    }

    fn handler(&self, skel: MechtronSkel<P>) -> Result<Box<dyn DirectedHandler>, P::Err> {
        Ok(Box::new(HelloGoodbyeMechtron::restore(skel, (), ())))
    }

    /*
    fn handler(&self, details: &Details, transmitter: ProtoTransmitterBuilder) -> Result<Box<dyn DirectedHandler>, P::Err> {
                let phantom:PhantomData<P> = PhantomData::default();
        let skel = MechtronSkel::new(details.clone(), phantom );

        Ok(Box::new(MyApp::restore(skel,(),(),())))
    }

     */
}

pub struct HelloGoodbyeMechtron<P>
where
    P: Platform + 'static,
{
    skel: MechtronSkel<P>,
}

impl<P> Mechtron<P> for HelloGoodbyeMechtron<P>
where
    P: Platform + 'static,
{
    type Skel = MechtronSkel<P>;
    type Cache = ();
    type State = ();

    fn restore(skel: Self::Skel, _cache: Self::Cache, _state: Self::State) -> Self {
        HelloGoodbyeMechtron { skel }
    }
}

impl<P> MechtronLifecycle<P> for HelloGoodbyeMechtron<P> where P: Platform + 'static {}

#[handler_sync]
impl<P> HelloGoodbyeMechtron<P>
where
    P: Platform + 'static,
{
    #[route("Ext<Hello>")]
    pub fn hello(&self, _: InCtx<'_, ()>) -> Result<Substance, P::Err> {
        Ok(Substance::Text("Goodbye".to_string()))
    }
}

#[cfg(test)]
pub mod test {
    #[test]
    pub fn test() {}
}
