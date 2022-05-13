use std::convert::{TryFrom, TryInto};
use std::marker::PhantomData;
use std::str::FromStr;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use futures::FutureExt;
use semver::Version;
use tokio::runtime::Handle;
use tokio::sync::{mpsc, oneshot};
use tokio::sync::oneshot::error::RecvError;
use tokio::time::error::Elapsed;

use crate::cache::{ArtifactCaches, ProtoArtifactCachesFactory};
use crate::error::Error;
use crate::frame::{StarPattern, TraversalAction, ResourceRegistryRequest, StarMessagePayload};
use crate::particle::{Kind, KindBase, AssignParticleStateSrc, ParticleRecord};
use crate::particle::FileSubKind;
use crate::star::{StarCommand, StarKind, StarSkel, StarKey};
use crate::star::shell::search::{SearchInit, SearchHits};
use crate::star::surface::SurfaceApi;
use crate::starlane::StarlaneCommand;
use crate::watch::{WatchResourceSelector, Watcher};
use crate::message::{ProtoStarMessage, ProtoStarMessageTo, ReplyKind, Reply};
use crate::artifact::{ArtifactBundle, ArtifactRef};
use kube::ResourceExt;
use mesh_portal::error;
use mesh_portal::version::latest::command::common::{SetLabel, StateSrc};
use mesh_portal::version::latest::entity::request::create::{PointSegFactory, PointTemplate, Create, KindTemplate, Strategy, Template};
use mesh_portal::version::latest::entity::request::{Method, Rc};
use mesh_portal::version::latest::entity::request::get::Get;
use mesh_portal::version::latest::id::Point;
use mesh_portal::version::latest::messaging::{Message, Request, Response};
use mesh_portal::version::latest::payload::{Payload, PayloadMap, Primitive};
use mesh_portal::version::latest::particle::Stub;
use mesh_portal::version::latest::command::common::{PropertyMod, SetProperties};
use mesh_portal::version::latest::entity::request::get::GetOp;
use mesh_portal::version::latest::id::Tks;

use crate::fail::{Fail, StarlaneFailure};

#[derive(Clone)]
pub struct StarlaneApi {
    surface_api: SurfaceApi,
    starlane_tx: Option<mpsc::Sender<StarlaneCommand>>,
    pub agent: Point
}

impl StarlaneApi {

}




impl StarlaneApi {
    pub fn new(surface_api: SurfaceApi, agent: Point) -> Self {
        Self::new_with_options(surface_api, Option::None, agent)
    }

    fn new_with_options(
        surface_api: SurfaceApi,
        starlane_tx: Option<mpsc::Sender<StarlaneCommand>>,
        agent: Point
    ) -> Self {
        Self {
            surface_api,
            starlane_tx,
            agent
        }
    }

    pub async fn exchange( &self, request: Request ) -> Response {
        self.surface_api.exchange(request).await
    }

    pub fn shutdown(&self) -> Result<(), Error> {
        self.starlane_tx.as_ref().ok_or("this api does not have access to the StarlaneMachine and therefore cannot do a shutdown")?.try_send(StarlaneCommand::Shutdown);
        Ok(())
    }

    pub async fn timeout<T>(
        rx: tokio::sync::oneshot::Receiver<Result<T, Error>>,
    ) -> Result<T, Error> {
        match tokio::time::timeout(Duration::from_secs(15), rx).await {
            Ok(result) => match result {
                Ok(result) => result,
                Err(_err) => Err("recv error".into()),
            },
            Err(err) => {
                eprintln!("elapsed error: {}", err);
                Err("timeout".into())
            }
        }
    }

    pub async fn fetch_resource_record(
        &self,
        point: Point,
    ) -> Result<ParticleRecord, Error> {
        self.surface_api.locate(point).await
    }


    pub async fn cache( &self, artifact: &ArtifactRef)  -> Result<ArtifactCaches,Error> {
        let mut cache = self.get_caches().await?.create();
        cache.cache(vec![artifact.clone()]).await?;
        Ok(cache.to_caches().await?)
    }

    pub async fn get_caches(&self) -> Result<Arc<ProtoArtifactCachesFactory>, Error> {
        Ok(self.surface_api.get_caches().await?)
    }

    pub async fn create(&self, create: Create) -> Result<Stub, Error> {
        let action = Method::Cmd(Rc::Create(create.clone()));
        let core = action.into();
        let request = Request::new(core, self.agent.clone(), create.template.point.parent.clone() );
        let response = self.surface_api.exchange(request).await.ok_or()?;
        if let Payload::Stub(stub) =  &response.core.body {
            Ok(stub.clone())
        } else {
            Err("unexpected response".into())
        }
    }

    pub async fn create_sys_resource( &self, template: Template, messenger_tx: mpsc::Sender<Message> ) -> Result<Stub,Error> {
       self.surface_api.create_sys_resource(template,messenger_tx).await
    }

    pub async fn star_search(
        &self,
        star_pattern: StarPattern
    ) -> Result<SearchHits, Error> {

        let hits = self.surface_api.star_search(star_pattern).await?;
        Ok(hits)
    }


    pub async fn watch(
        &self,
        selector: WatchResourceSelector,
    ) -> Result<Watcher, Error> {
        self.surface_api.watch( selector ).await
    }


    pub async fn create_api<API>(&self, create: Create) -> Result<API, Error>
    where
        API: TryFrom<ResourceApi>,
    {
        let resource_api = ResourceApi {
            stub: self.create(create).await?,
            surface_api: self.surface_api.clone(),
            agent: self.agent.clone()
        };

        let api = API::try_from(resource_api);

        match api {
            Ok(api) => Ok(api),
            Err(error) => Err("catastrophic conversion error when attempting to try_convert api".into()),
        }
    }

    pub async fn create_space(&self, name: &str) -> Result<Creation<SpaceApi>, Error> {
        let point_template = PointTemplate {
            parent: Point::root(),
            child_segment_template: PointSegFactory::Exact(name.to_string())
        };

        let kind_template = KindTemplate {
            kind: "Space".to_string(),
            sub_kind: None,
            specific: None
        };

        let template = Template::new(point_template,kind_template );
        let mut properties = SetProperties::new();
        let create = Create {
            template,
            state: StateSrc::Stateless,
            properties,
            strategy: Strategy::Create,
            registry: Default::default()
        };

        let creation = Creation::new( self.clone(), create );
        Ok(creation)
    }

    pub async fn get_space(&self, point: Point) -> Result<SpaceApi, Error> {
        let record = self.fetch_resource_record(point).await?;
        Ok(SpaceApi::new(self.surface_api.clone(), record.details.stub, self.agent.clone() )?)
    }

    pub async fn get_state(&self, point: Point) -> Result<Payload,Error> {
        let get = Get {
            point: point.clone(),
            op: GetOp::State
        };
        let action = Method::Cmd(Rc::Get(get));
        let core = action.into();
        let request = Request::new(core, self.agent.clone(), point.clone());
        let response = self.surface_api.exchange(request).await;
        match response.core.is_ok() {
            true => Ok(response.core.body),
            false => Err(format!("could not get state for: '{}'",point.to_string()).into())
        }
    }
}

pub struct SpaceApi {
    stub: Stub,
    surface_api: SurfaceApi,
    agent: Point
}

impl SpaceApi {

    pub fn point(&self) -> Point {
        self.stub.point.clone()
    }

    pub fn new(surface_api: SurfaceApi, stub: Stub, agent: Point) -> Result<Self, Error> {
        if stub.kind.kind != KindBase::Space.to_string() {
            return Err(format!(
                "wrong kind particle type for SpaceApi: {}",
                stub.kind.kind.to_string()
            )
            .into());
        }

        Ok(SpaceApi { stub, surface_api, agent })
    }

    pub fn starlane_api(&self) -> StarlaneApi {
        StarlaneApi::new(self.surface_api.clone(), self.agent.clone() )
    }

}




pub struct Creation<API>
where
    API: TryFrom<ResourceApi>,
{
    api: StarlaneApi,
    create: Create,
    phantom: PhantomData<API>,
}

impl<API> Creation<API>
where
    API: TryFrom<ResourceApi>,
{
    pub fn new(api: StarlaneApi, create: Create ) -> Self {
        Self {
            api,
            create,
            phantom: PhantomData {},
        }
    }

    pub async fn submit(self) -> Result<API, Error> {
        self.api.create_api(self.create).await
    }

    pub fn set_strategy(&mut self, strategy: Strategy) {
        self.create.strategy = strategy;
    }

    pub fn set_state(&mut self, payload: Payload ) {
        self.create.state = StateSrc::StatefulDirect(payload);
    }

    pub fn set_label(&mut self, key: &str) {
        let key = key.to_string();
        self.create.registry.push(SetLabel::Set(key));
    }

    pub fn set_label_with_value(&mut self, key: &str, value: &str) {
        let key = key.to_string();
        let value = value.to_string();
        self.create.registry.push(SetLabel::SetValue {key,value} );
    }

    pub fn set_property( &mut self, key:&str, value: &str) {
       self.create.properties.push(PropertyMod::Set{ key: key.to_string(), value: value.to_string(), lock: false });
    }
}

pub struct ResourceApi {
    stub: Stub,
    surface_api: SurfaceApi,
    agent: Point
}




impl TryFrom<ResourceApi> for SpaceApi {
    type Error = Error;

    fn try_from(value: ResourceApi) -> Result<Self, Self::Error> {
        Ok(Self::new(value.surface_api, value.stub, value.agent.clone())?)
    }
}


#[derive(Debug)]
pub enum StarlaneAction {
    GetState {
        point: Point,
        tx: tokio::sync::oneshot::Sender<Result<Payload, Error>>,
    },
}

pub struct StarlaneApiRunner {
    api: StarlaneApi,
    rx: tokio::sync::mpsc::Receiver<StarlaneAction>,
}

impl StarlaneApiRunner {
    pub fn new(api: StarlaneApi) -> tokio::sync::mpsc::Sender<StarlaneAction> {
        let (tx, rx) = tokio::sync::mpsc::channel(16);

        let runner = StarlaneApiRunner { api: api, rx: rx };

        runner.run();

        tx
    }

    fn run(mut self) {
        thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async move {
                while let Option::Some(action) = self.rx.recv().await {
                    self.process(action).await;
                }
            });
        });
    }

    async fn process(&self, action: StarlaneAction) {
        match action {
            StarlaneAction::GetState { point: identifier, tx } => {
                tx.send(self.api.get_state(identifier).await );
            }
        }
    }
}

#[derive(Clone)]
pub struct StarlaneApiRelay {
    tx: tokio::sync::mpsc::Sender<StarlaneAction>,
}

impl StarlaneApiRelay {
    /*
    pub async fn get_resource_state(
        &self,
        identifier: ResourceIdentifier,
    ) -> Result<DataSet<BinSrc>, Error> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx
            .send(StarlaneAction::GetState {
                point: identifier,
                tx: tx,
            })
            .await;
        rx.await?
    }
     */
}

impl Into<StarlaneApiRelay> for StarlaneApi {
    fn into(self) -> StarlaneApiRelay {
        StarlaneApiRelay {
            tx: StarlaneApiRunner::new(self),
        }
    }
}
