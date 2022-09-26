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
use mechtron::{Mechtron, MechtronLifecycle};
use mechtron::{guest, Guest, MechtronFactories, MechtronFactory, Platform};
use std::sync::Arc;
use cosmic_universe::wave::core::CoreBounce;
use cosmic_universe::wave::exchange::synch::{DirectedHandler, RootInCtx};
use cosmic_macros::handler_sync;

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
        "my-mechtron".to_string()
    }

    fn create(&self, details: Details ) -> Result<Box<dyn MechtronLifecycle<P>>, P::Err> {
        Ok(Box::new(MyApp::new()))
    }
}


pub struct MyApp<P> where P: Platform {
    phantom: PhantomData<P>
}

impl <P> MyApp<P> where P: Platform {
    pub fn new()->Self{
        Self{
            phantom: Default::default()
        }

    }
}

impl <P> Mechtron<P> for MyApp<P> where P: Platform+'static {
    type Skel = ();
    type Ctx = ();
    type Cache = ();
    type State = ();

    fn restore(_skel: Self::Skel, _ctx: Self::Ctx, _cache: Self::Cache, _state: Self::State) -> Self {
        MyApp::new()
    }

    fn cache(ctx: Self::Ctx) -> Self::Cache {
        ()
    }
}

impl <P> MechtronLifecycle<P> for MyApp<P> where P: Platform+'static {

}

#[handler_sync]
impl <P> MyApp<P> where P:Platform {

}


#[cfg(test)]
pub mod test {
    #[test]
    pub fn test () {

    }
}
