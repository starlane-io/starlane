use cosmic_space::wave::exchange::synch::DirectedHandler;
use std::collections::HashMap;
use cosmic_space::point::Point;
use std::sync::Arc;
use handlebars::Handlebars;
use mechtron::{Mechtron, MechtronFactory, MechtronLifecycle, MechtronSkel, Platform};
use crate::mechtron::MyMechtron;

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
