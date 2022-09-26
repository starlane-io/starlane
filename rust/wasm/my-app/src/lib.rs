#![allow(warnings)]

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate cosmic_macros;
//mod html;

use std::marker::PhantomData;
use cosmic_universe::err::UniErr;
use cosmic_universe::particle::Details;
use mechtron::err::{GuestErr, MechErr};
use mechtron::{Mechtron, MechtronLifecycle, MechtronSkel};
use mechtron::{guest, Guest, MechtronFactories, MechtronFactory, Platform};
use std::sync::Arc;
use cosmic_universe::wave::core::CoreBounce;
use cosmic_universe::wave::exchange::synch::{DirectedHandler, InCtx, ProtoTransmitter, ProtoTransmitterBuilder, RootInCtx};
use cosmic_macros::handler_sync;
use cosmic_universe::log::{PointLogger, RootLogger};
use mechtron::guest::GuestSkel;

#[no_mangle]
pub extern "C" fn mechtron_guest(details: Details) -> Result<Arc<dyn mechtron::Guest>, GuestErr> {
    Ok(Arc::new(mechtron::guest::Guest::new(
        details,
        MyAppPlatform::new(),
    )?))
}

#[derive(Clone)]
pub struct MyAppPlatform;

impl Platform for MyAppPlatform {
    type Err = GuestErr;
    fn factories(&self) -> Result<MechtronFactories<Self>, Self::Err>
    where
        Self: Sized,
    {
        let mut factories = MechtronFactories::new();
        factories.add( MyAppFactory::new() );
        Ok(factories)
    }
}

impl MyAppPlatform {
    pub fn new() -> Self {
        Self {}
    }
}


pub struct MyAppFactory { }

impl MyAppFactory {
    pub fn new() -> Self {
        Self{}
    }
}

impl <P> MechtronFactory<P> for MyAppFactory where P: Platform+'static{
    fn name(&self) -> String {
        "my-app".to_string()
    }

    fn lifecycle(&self, skel: MechtronSkel<P> ) -> Result<Box<dyn MechtronLifecycle<P>>, P::Err> {
        Ok(Box::new(MyApp::restore(skel,(),())))
    }

    fn handler(&self, skel: MechtronSkel<P>) -> Result<Box<dyn DirectedHandler>, P::Err> {
        Ok(Box::new(MyApp::restore(skel,(),())))
    }

    /*
    fn handler(&self, details: &Details, transmitter: ProtoTransmitterBuilder) -> Result<Box<dyn DirectedHandler>, P::Err> {
                let phantom:PhantomData<P> = PhantomData::default();
        let skel = MechtronSkel::new(details.clone(), phantom );

        Ok(Box::new(MyApp::restore(skel,(),(),())))
    }

     */
}




pub struct MyApp<P> where P: Platform + 'static{
    skel: MechtronSkel<P>
}


impl <P> Mechtron<P> for MyApp<P> where P: Platform+'static {
    type Skel = MechtronSkel<P>;
    type Cache = ();
    type State = ();

    fn restore(skel: Self::Skel, _cache: Self::Cache, _state: Self::State) -> Self {
        MyApp {
            skel
        }
    }
}


impl <P> MechtronLifecycle<P> for MyApp<P> where P: Platform+'static {

}

#[handler_sync]
impl <P> MyApp<P> where P: Platform+'static {
    #[route("Ext<Check>")]
    pub fn check(&self, _: InCtx<'_,()> ) -> Result<(),P::Err> {
        self.skel.logger.info("CHECK MECHTRON REACHED!");
        Ok(())
    }
}


#[cfg(test)]
pub mod test {
    #[test]
    pub fn test () {

    }
}
