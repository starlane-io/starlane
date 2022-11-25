#![allow(warnings)]

pub mod err;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate cosmic_macros;

#[macro_use]
extern crate cosmic_macros_primitive;


use cosmic_macros::handler_sync;
use cosmic_space::err::SpaceErr;
use cosmic_space::point::Point;
use cosmic_space::log::{PointLogger, RootLogger};
use cosmic_space::particle::Details;
use cosmic_space::substance::Substance;
use cosmic_space::wave::core::CoreBounce;
use cosmic_space::wave::exchange::synch::{
    DirectedHandler, InCtx, ProtoTransmitter, ProtoTransmitterBuilder, RootInCtx,
};
use handlebars::{Handlebars, Renderable, Template};
use mechtron::err::{GuestErr, MechErr};
use mechtron::guest::{GuestCtx, GuestSkel};
use mechtron::{guest, Guest, MechtronFactories, MechtronFactory, Platform};
use mechtron::{Mechtron, MechtronLifecycle, MechtronSkel};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;
use crate::err::MyErr;
use serde_json::json;

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
    type Err = MyErr;
    fn factories(&self) -> Result<MechtronFactories<Self>, Self::Err>
    where
        Self: Sized,
    {
        let mut factories = MechtronFactories::new();
        factories.add(MyMechtronFactory::new());
        Ok(factories)
    }
}

impl MyPlatform {
    pub fn new() -> Self {
        Self {}
    }
}

pub struct MyMechtronFactory
{
    pub cache: HashMap<Point,Arc<Handlebars<'static>>>,
}

impl MyMechtronFactory
{
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }
}

impl<P> MechtronFactory<P> for MyMechtronFactory
where
    P: Platform + 'static,
{
    fn name(&self) -> String {
        "hello".to_string()
    }

    fn new(&mut self, skel: MechtronSkel<P>) -> Result<(), P::Err> {
        let template = MyMechtron::cache(skel.clone())?.ok_or("expected template")?;
        self.cache.insert(skel.details.stub.point.clone(), template);
        Ok(())
    }

    fn lifecycle(&self, skel: MechtronSkel<P>) -> Result<Box<dyn MechtronLifecycle<P>>, P::Err> {
        let cache = self
            .cache
            .get(&skel.details.stub.point)
            .ok_or("expecting template")?
            .clone();
        Ok(Box::new(MyMechtron::restore(skel, cache, ())))
    }

    fn handler(&self, skel: MechtronSkel<P>) -> Result<Box<dyn DirectedHandler>, P::Err> {
        let cache = self
            .cache
            .get(&skel.details.stub.point)
            .ok_or("expecting template")?
            .clone();
        Ok(Box::new(MyMechtron::restore(skel, cache, ())))
    }
}

pub struct MyMechtron<P>
where
    P: Platform + 'static,
{
    skel: MechtronSkel<P>,
    cache: Arc<Handlebars<'static>>,
}

impl<P> Mechtron<P> for MyMechtron<P>
where
    P: Platform + 'static,
{
    type Skel = MechtronSkel<P>;
    type Cache = Arc<Handlebars<'static>>;
    type State = ();

    fn restore(skel: Self::Skel, cache: Self::Cache, _state: Self::State) -> Self {
        MyMechtron { skel, cache }
    }

    fn cache(skel: Self::Skel) -> Result<Option<Self::Cache>, P::Err> {
        let template = skel.raw_from_bundle("template/index.html")?;
        let template = String::from_utf8((**template).clone())?;

        let mut handlebars = Handlebars::new();

        handlebars.register_template_string("template", template);

        Ok(Some(Arc::new(handlebars)))
    }
}

impl<P> MechtronLifecycle<P> for MyMechtron<P> where P: Platform + 'static {}

#[handler_sync]
impl <P> MyMechtron<P>
where
    P: Platform + 'static,
{
    #[route("Http<Get>")]
    pub fn hello(&self, ctx: InCtx<'_, ()>) -> Result<Substance, MyErr> {
        let render = self.cache.render("template", &json!({"title": "My Mechtron", "message": "Hello World"}) )?;

        Ok(Substance::Text(render))
    }
}

#[cfg(test)]
pub mod test {
    #[test]
    pub fn test() {}
}
