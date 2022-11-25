use std::sync::Arc;
use cosmic_space::wave::exchange::synch::InCtx;
use cosmic_space::substance::Substance;
use handlebars::Handlebars;
use mechtron::{Mechtron, MechtronLifecycle, MechtronSkel, Platform};
use serde_json::json;
use crate::err::MyErr;

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
skel.logger.info("restore mechtron");
        MyMechtron { skel, cache }
    }

    fn cache(skel: Self::Skel) -> Result<Option<Self::Cache>, P::Err> {
skel.logger.info("caching mechtron");
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
    #[route("Http<Get>/.*")]
    pub fn hello(&self, ctx: InCtx<'_, ()>) -> Result<Substance, MyErr> {
self.skel.logger.info("mechtron RPC");
        let render = self.cache.render("template", &json!({"title": "Welcome", "message": "Hello World"}) )?;

        Ok(Substance::Text(render))
    }
}
