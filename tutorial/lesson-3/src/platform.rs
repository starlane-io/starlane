use mechtron::{MechtronFactories, Platform};
use crate::err::MyErr;
use crate::factory::MyMechtronFactory;

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

#[derive(Clone)]
pub struct MyPlatform;

impl MyPlatform {
    pub fn new() -> Self {
        Self {}
    }
}
