use crate::base::config::{BaseConfig, BaseSubConfig};
use crate::registry::exchange::MuxedRequest;
use async_trait::async_trait;
use serde_derive::{Deserialize, Serialize};
use starlane_space::command::direct::delete::Delete;
use starlane_space::command::direct::query::{Query, QueryResult};
use starlane_space::command::direct::select::{Select, SubSelect};
use starlane_space::err::{HyperSpatialError, ParseErrs0, SpaceErr, SpatialError};
use starlane_space::hyper::{ParticleLocation, ParticleRecord};
use starlane_space::kind::Kind;
use starlane_space::particle::{Details, Properties, Status, Stub};
use starlane_space::point::Point;
use starlane_space::security::{Access, AccessGrant, IndexedAccessGrant};
use starlane_space::selector::Selector;
use starlane_space::substance::SubstanceList;
use starlane_space::types::property::SetProperties;
use starlane_space::{SetRegistry, Strategy};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::oneshot::error::RecvError;

pub type Registry = Arc<dyn RegistryApi>;

pub trait RegistryConfig: BaseSubConfig {}

#[async_trait]
pub trait RegistryApi: Send + Sync {
    async fn scorch(&self) -> Result<(), RegErr>;

    async fn register<'a>(&'a self, registration: &'a Registration) -> Result<(), RegErr>;

    async fn assign_star<'a>(&'a self, point: &'a Point, star: &'a Point) -> Result<(), RegErr>;

    async fn assign_host<'a>(&'a self, point: &'a Point, host: &'a Point) -> Result<(), RegErr>;

    async fn set_status<'a>(&'a self, point: &'a Point, status: &'a Status) -> Result<(), RegErr>;

    async fn set_properties<'a>(
        &'a self,
        point: &'a Point,
        properties: &'a SetProperties,
    ) -> Result<(), RegErr>;

    async fn sequence<'a>(&'a self, point: &'a Point) -> Result<u64, RegErr>;

    async fn get_properties<'a>(&'a self, point: &'a Point) -> Result<Properties, RegErr>;

    async fn record<'a>(&'a self, point: &'a Point) -> Result<ParticleRecord, RegErr>;

    async fn query<'a>(&'a self, point: &'a Point, query: &'a Query)
        -> Result<QueryResult, RegErr>;

    async fn delete<'a>(&'a self, delete: &'a Delete) -> Result<SubstanceList, RegErr>;

    //    async fn select<'a>(&'a self, select: &'a mut Select) -> Result<SubstanceList, RegErr>;

    async fn select<'a>(&'a self, select: &'a mut Select) -> Result<SubstanceList, RegErr>;

    //async fn sub_select<'a>(&'a self, sub_select: &'a SubSelect) -> Result<Vec<Stub>, RegErr>;

    async fn grant<'a>(&'a self, access_grant: &'a AccessGrant) -> Result<(), RegErr>;

    async fn access<'a>(&'a self, to: &'a Point, on: &'a Point) -> Result<Access, RegErr>;

    async fn chown<'a>(
        &'a self,
        on: &'a Selector,
        owner: &'a Point,
        by: &'a Point,
    ) -> Result<(), RegErr>;

    async fn list_access<'a>(
        &'a self,
        to: &'a Option<&'a Point>,
        on: &'a Selector,
    ) -> Result<Vec<IndexedAccessGrant>, RegErr>;

    async fn remove_access<'a>(&'a self, id: i32, to: &'a Point) -> Result<(), RegErr>;
}

pub struct RegistryWrapper {
    registry: Registry,
}

impl RegistryWrapper {
    pub fn new(registry: Registry) -> Self {
        Self { registry }
    }
}

#[async_trait]
trait RegistrySubSelect: RegistryApi {
    async fn sub_select<'a>(&'a self, sub_select: &'a SubSelect) -> Result<Vec<Stub>, RegErr>;
}

async fn select<'a>(
    registry: &'a impl RegistrySubSelect,
    select: &'a mut Select,
) -> Result<SubstanceList, RegErr> {
    let point = select.pattern.query_root();

    let hierarchy = registry
        .query(&point, &Query::PointHierarchy)
        .await?
        .try_into()?;

    let sub_select_hops = select.pattern.sub_select_hops();
    let sub_select = select
        .clone()
        .sub_select(point.clone(), sub_select_hops, hierarchy);
    let mut list = registry.sub_select(&sub_select).await?;
    if select.pattern.matches_root() {
        list.push(Stub {
            point: Point::root(),
            kind: Kind::Root,
            status: Status::Ready,
        });
    }

    let list = sub_select.into_payload.to_primitive(list)?;

    Ok(list)
}

#[async_trait]
impl RegistryApi for RegistryWrapper {
    async fn scorch<'a>(&'a self) -> Result<(), RegErr> {
        self.registry.scorch().await
    }

    async fn register<'a>(&'a self, registration: &'a Registration) -> Result<(), RegErr> {
        self.registry.register(registration).await
    }

    async fn assign_star<'a>(&'a self, point: &'a Point, star: &'a Point) -> Result<(), RegErr> {
        self.registry.assign_star(point, star).await
    }

    async fn assign_host<'a>(&'a self, point: &'a Point, host: &'a Point) -> Result<(), RegErr> {
        self.registry.assign_host(point, host).await
    }

    async fn set_status<'a>(&'a self, point: &'a Point, status: &'a Status) -> Result<(), RegErr> {
        self.registry.set_status(point, status).await
    }

    async fn set_properties<'a>(
        &'a self,
        point: &'a Point,
        properties: &'a SetProperties,
    ) -> Result<(), RegErr> {
        self.registry.set_properties(point, properties).await
    }

    async fn sequence<'a>(&'a self, point: &'a Point) -> Result<u64, RegErr> {
        self.registry.sequence(point).await
    }

    async fn get_properties<'a>(&'a self, point: &'a Point) -> Result<Properties, RegErr> {
        self.registry.get_properties(point).await
    }

    async fn record<'a>(&'a self, point: &'a Point) -> Result<ParticleRecord, RegErr> {
        if point.is_global() {
            let location = ParticleLocation::new(Some(Point::local_star()), None);
            let record = ParticleRecord {
                details: Details {
                    stub: Stub {
                        point: point.clone(),
                        kind: Kind::Global,
                        status: Status::Ready,
                    },
                    properties: Properties::default(),
                },
                location,
            };

            Ok(record)
        } else {
            self.registry.record(point).await
        }
    }

    async fn query<'a>(
        &'a self,
        point: &'a Point,
        query: &'a Query,
    ) -> Result<QueryResult, RegErr> {
        self.registry.query(point, query).await
    }

    async fn delete<'a>(&'a self, delete: &'a Delete) -> Result<SubstanceList, RegErr> {
        self.registry.delete(delete).await
    }

    async fn select<'a>(&'a self, select: &'a mut Select) -> Result<SubstanceList, RegErr> {
        self.registry.select(select).await
    }

    async fn grant<'a>(&'a self, access_grant: &'a AccessGrant) -> Result<(), RegErr> {
        self.registry.grant(access_grant).await
    }

    async fn access<'a>(&'a self, to: &'a Point, on: &'a Point) -> Result<Access, RegErr> {
        self.registry.access(to, on).await
    }

    async fn chown<'a>(
        &'a self,
        on: &'a Selector,
        owner: &'a Point,
        by: &'a Point,
    ) -> Result<(), RegErr> {
        self.registry.chown(on, owner, by).await
    }

    async fn list_access<'a>(
        &'a self,
        to: &'a Option<&'a Point>,
        on: &'a Selector,
    ) -> Result<Vec<IndexedAccessGrant>, RegErr> {
        self.registry.list_access(to, on).await
    }

    async fn remove_access<'a>(&'a self, id: i32, to: &'a Point) -> Result<(), RegErr> {
        self.registry.remove_access(id, to).await
    }
}

#[derive(Error, Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum RegErr {
    #[error(transparent)]
    Parse(#[from] ParseErrs0),
    #[error("duplicate error")]
    Dupe,
    #[error("particle not found: '{0}'")]
    NotFound(Point),

    #[error(transparent)]
    SpaceErr(#[from] SpaceErr),

    #[error("expected parent for point `{0}'")]
    ExpectedParent(Point),
    #[error("Registry does not handle GetOp::State operations")]
    NoGetOpStateOperations,
    #[error("Database Setup Failed")]
    RegistrySetupFailed,
    #[error("Point '{point}' registry error: {message}")]
    Point { point: Point, message: String },
    #[error("{0}")]
    Msg(String),

    #[error("postgres registry db connection pool '{0}' not found")]
    PoolNotFound(String),

    #[error("{0}")]
    IoErr(String),
    #[error("database has scorch guard enabled.  To change this: 'INSERT INTO reset_mode VALUES ('Scorch')'"
    )]
    NoScorch,
    #[error("expected an embedded postgres registry but received configuration for a remote postgres registry"
    )]
    ExpectedEmbeddedRegistry,
    #[error("Registry is not reachable")]
    Unreachable,
    #[error("Registry exchanger error")]
    ExchangeErr,
}

impl<T> From<tokio::sync::mpsc::error::SendError<T>> for RegErr {
    fn from(value: SendError<T>) -> Self {
        Self::ExchangeErr
    }
}

impl From<tokio::sync::oneshot::error::RecvError> for RegErr {
    fn from(value: tokio::sync::oneshot::error::RecvError) -> Self {
        Self::ExchangeErr
    }
}

/*impl From<tokio::sync::mpsc::error::SendError<RegistryRequest>> for RegErr {
    fn from(value: SendError<RegistryRequest>) -> Self {
        RegErr::ExchangeErr
    }
}

 */

impl From<std::io::Error> for RegErr {
    fn from(value: std::io::Error) -> Self {
        Self::IoErr(value.to_string())
    }
}

impl SpatialError for RegErr {}

impl HyperSpatialError for RegErr {}

impl From<&str> for RegErr {
    fn from(err: &str) -> Self {
        Self::Msg(err.to_string())
    }
}

impl From<&String> for RegErr {
    fn from(err: &String) -> Self {
        Self::Msg(err.to_string())
    }
}

impl RegErr {
    pub fn dupe() -> Self {
        Self::Dupe
    }

    pub fn point<S>(point: Point, message: S) -> RegErr
    where
        S: ToString,
    {
        let message = message.to_string();
        RegErr::Point { point, message }
    }

    pub fn pool_not_found<S: ToString>(key: S) -> Self {
        Self::PoolNotFound(key.to_string())
    }

    pub fn expected_parent(point: &Point) -> Self {
        Self::ExpectedParent(point.clone())
    }

    pub fn msg<M>(msg: M) -> RegErr
    where
        M: ToString,
    {
        Self::Msg(msg.to_string())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct Registration {
    pub point: Point,
    pub kind: Kind,
    pub registry: SetRegistry,
    pub properties: SetProperties,
    pub owner: Point,
    pub strategy: Strategy,
    pub status: Status,
}

impl Registration {
    pub fn mock() -> Self {
        Self {
            point: Point::root(),
            kind: Kind::Root,
            registry: Default::default(),
            properties: Default::default(),
            owner: Point::hyper_user(),
            strategy: Strategy::Commit,
            status: Status::Unknown,
        }
    }
}

#[derive(
    Clone,
    Debug,
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    strum_macros::Display,
    strum_macros::EnumDiscriminants,
)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(RegistryRequestType))]
#[strum_discriminants(derive(Hash, Serialize, Deserialize, strum_macros::Display))]
pub enum RegistryRequest {
    Scorch,
    Register(Registration),
    AssignStar {
        point: Point,
        star: Point,
    },
    AssignHost {
        point: Point,
        host: Point,
    },
    SetStatus {
        point: Point,
        status: Status,
    },
    SetProperties {
        point: Point,
        properties: SetProperties,
    },
    Sequence(Point),
    GetProperties(Point),
    Record(Point),
    Query {
        point: Point,
        query: Query,
    },
    Delete(Delete),
    Select(Select),

    Grant(AccessGrant),
    Access {
        to: Point,
        on: Point,
    },
    Chown {
        on: Selector,
        owner: Point,
        by: Point,
    },
    ListAccess {
        to: Option<Point>,
        on: Selector,
    },
    RemoveAccess {
        id: i32,
        to: Point,
    },
}

#[derive(
    Clone,
    Debug,
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    strum_macros::Display,
    strum_macros::EnumDiscriminants,
)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(RegistryResponseType))]
#[strum_discriminants(derive(Hash, Serialize, Deserialize, strum_macros::Display))]
pub enum RegistryResponse {
    Scorch,
    Register,
    AssignStar,
    AssignHost,
    SetStatus,
    SetProperties,
    Sequence(u64),
    GetProperties(Properties),
    Record(ParticleRecord),
    Query(QueryResult),
    Delete(SubstanceList),
    Select(SubstanceList),
    Grant,
    Access(Access),
    Chown,
    ListAccess(Vec<IndexedAccessGrant>),
    RemoveAccess,
}

impl TryInto<Result<(), RegErr>> for RegistryResponse {
    type Error = RegErr;

    fn try_into(self) -> Result<Result<(), RegErr>, Self::Error> {
        match self {
            RegistryResponse::Scorch => Ok(Ok(())),
            RegistryResponse::Register => Ok(Ok(())),
            RegistryResponse::AssignStar => Ok(Ok(())),
            RegistryResponse::AssignHost => Ok(Ok(())),
            RegistryResponse::SetStatus => Ok(Ok(())),
            RegistryResponse::SetProperties => Ok(Ok(())),
            RegistryResponse::Grant => Ok(Ok(())),
            RegistryResponse::Chown => Ok(Ok(())),
            RegistryResponse::RemoveAccess => Ok(Ok(())),
            _ => Err(RegErr::msg(
                "Invalid RegistryResponse variant for Result<(), RegErr>",
            )),
        }
    }
}

impl TryInto<Result<u64, RegErr>> for RegistryResponse {
    type Error = RegErr;

    fn try_into(self) -> Result<Result<u64, RegErr>, Self::Error> {
        match self {
            RegistryResponse::Sequence(value) => Ok(Ok(value)),
            _ => Err(RegErr::msg(
                "Invalid RegistryResponse variant for Result<u64, RegErr>",
            )),
        }
    }
}

impl TryInto<Result<Properties, RegErr>> for RegistryResponse {
    type Error = RegErr;

    fn try_into(self) -> Result<Result<Properties, RegErr>, Self::Error> {
        match self {
            RegistryResponse::GetProperties(value) => Ok(Ok(value)),
            _ => Err(RegErr::msg(
                "Invalid RegistryResponse variant for Result<Properties, RegErr>",
            )),
        }
    }
}

impl TryInto<Result<ParticleRecord, RegErr>> for RegistryResponse {
    type Error = RegErr;

    fn try_into(self) -> Result<Result<ParticleRecord, RegErr>, Self::Error> {
        match self {
            RegistryResponse::Record(value) => Ok(Ok(value)),
            _ => Err(RegErr::msg(
                "Invalid RegistryResponse variant for Result<ParticleRecord, RegErr>",
            )),
        }
    }
}

impl TryInto<Result<QueryResult, RegErr>> for RegistryResponse {
    type Error = RegErr;

    fn try_into(self) -> Result<Result<QueryResult, RegErr>, Self::Error> {
        match self {
            RegistryResponse::Query(value) => Ok(Ok(value)),
            _ => Err(RegErr::msg(
                "Invalid RegistryResponse variant for Result<QueryResult, RegErr>",
            )),
        }
    }
}

impl TryInto<Result<SubstanceList, RegErr>> for RegistryResponse {
    type Error = RegErr;

    fn try_into(self) -> Result<Result<SubstanceList, RegErr>, Self::Error> {
        match self {
            RegistryResponse::Delete(value) => Ok(Ok(value)),
            RegistryResponse::Select(value) => Ok(Ok(value)),
            _ => Err(RegErr::msg(
                "Invalid RegistryResponse variant for Result<SubstanceList, RegErr>",
            )),
        }
    }
}

impl TryInto<Result<Access, RegErr>> for RegistryResponse {
    type Error = RegErr;

    fn try_into(self) -> Result<Result<Access, RegErr>, Self::Error> {
        match self {
            RegistryResponse::Access(value) => Ok(Ok(value)),
            _ => Err(RegErr::msg(
                "Invalid RegistryResponse variant for Result<Access, RegErr>",
            )),
        }
    }
}

impl TryInto<Result<Vec<IndexedAccessGrant>, RegErr>> for RegistryResponse {
    type Error = RegErr;

    fn try_into(self) -> Result<Result<Vec<IndexedAccessGrant>, RegErr>, Self::Error> {
        match self {
            RegistryResponse::ListAccess(value) => Ok(Ok(value)),
            _ => Err(RegErr::msg(
                "Invalid RegistryResponse variant for Result<Vec<IndexedAccessGrant>, RegErr>",
            )),
        }
    }
}

pub mod exchange {
    use crate::registry::{RegErr, Registration, RegistryApi, RegistryRequest, RegistryResponse};
    use anyhow::anyhow;
    use async_trait::async_trait;
    use dashmap::DashMap;
    use futures::{Sink, SinkExt, Stream, StreamExt};
    use itertools::Itertools;
    use mockall::PredicateBoxExt;
    use serde_derive::{Deserialize, Serialize};
    use starlane_space::types::registry::Registry;
    use starlane_space::wave::exchange::asynch::Exchanger;
    use starlane_space::{
        Access, AccessGrant, Delete, IndexedAccessGrant, ParticleRecord, Point, Properties, Query,
        QueryResult, Select, Selector, SetProperties, Status, Stub, SubSelect, SubstanceList,
    };
    use std::collections::HashMap;
    use std::fmt::Display;
    use std::io;
    use std::io::Write;
    use std::marker::PhantomData;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::task::{Context, Poll};
    use std::time::Duration;
    use nom::AsBytes;
    use tokio::sync::mpsc::error::SendError;
    use tokio_util::bytes::{BufMut, BytesMut};
    use tokio_util::codec::{
        Decoder, Encoder, Framed, FramedRead, FramedWrite, LengthDelimitedCodec, LinesCodec,
    };

    mod transform {
        use super::*;

        pub fn scorch(result: RegistryResponse) -> Result<(), RegErr> {
            if let RegistryResponse::Scorch = result {
                Ok(())
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        pub fn register(res: RegistryResponse) -> Result<(), RegErr> {
            if let RegistryResponse::Register = res {
                Ok(())
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        pub fn assign_star(res: RegistryResponse) -> Result<(), RegErr> {
            if let RegistryResponse::AssignStar = res {
                Ok(())
            } else {
                Err(RegErr::ExchangeErr)
            }
        }
        pub fn assign_host(res: RegistryResponse) -> Result<(), RegErr> {
            if let RegistryResponse::AssignHost = res {
                Ok(())
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        pub fn set_status(res: RegistryResponse) -> Result<(), RegErr> {
            if let RegistryResponse::SetStatus = res {
                Ok(())
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        pub fn set_properties(res: RegistryResponse) -> Result<(), RegErr> {
            if let RegistryResponse::SetProperties = res {
                Ok(())
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        pub fn sequence(res: RegistryResponse) -> Result<u64, RegErr> {
            if let RegistryResponse::Sequence(ret) = res {
                Ok(ret)
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        pub fn get_properties(res: RegistryResponse) -> Result<Properties, RegErr> {
            if let RegistryResponse::GetProperties(ret) = res {
                Ok(ret)
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        pub fn record(res: RegistryResponse) -> Result<ParticleRecord, RegErr> {
            if let RegistryResponse::Record(ret) = res {
                Ok(ret)
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        pub fn query(res: RegistryResponse) -> Result<QueryResult, RegErr> {
            if let RegistryResponse::Query(ret) = res {
                Ok(ret)
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        pub fn delete(res: RegistryResponse) -> Result<SubstanceList, RegErr> {
            if let RegistryResponse::Delete(ret) = res {
                Ok(ret)
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        pub fn select(res: RegistryResponse) -> Result<SubstanceList, RegErr> {
            if let RegistryResponse::Select(ret) = res {
                Ok(ret)
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        pub fn grant(res: RegistryResponse) -> Result<(), RegErr> {
            if let RegistryResponse::Grant = res {
                Ok(())
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        pub fn access(res: RegistryResponse) -> Result<Access, RegErr> {
            if let RegistryResponse::Access(ret) = res {
                Ok(ret)
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        pub fn chown(res: RegistryResponse) -> Result<(), RegErr> {
            if let RegistryResponse::Chown = res {
                Ok(())
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        pub fn list_access(res: RegistryResponse) -> Result<Vec<IndexedAccessGrant>, RegErr> {
            if let RegistryResponse::ListAccess(ret) = res {
                Ok(ret)
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        pub fn remove_access(res: RegistryResponse) -> Result<(), RegErr> {
            if let RegistryResponse::RemoveAccess = res {
                Ok(())
            } else {
                Err(RegErr::ExchangeErr)
            }
        }
    }

    struct MuxFramedWriter<T, S>
    where
        T: Send + Sync,
        S: SinkExt<T> + Sink<T, Error = anyhow::Error> + Send + Sync + std::marker::Unpin,
    {
        rx: tokio::sync::mpsc::Receiver<T>,
        sink: S,
    }

    impl<T, S> MuxFramedWriter<T, S>
    where
        T: Send + Sync + 'static,
        S: SinkExt<T> + Sink<T, Error = anyhow::Error> + Send + Sync + std::marker::Unpin + 'static,
    {
        pub fn new(sink: S) -> tokio::sync::mpsc::Sender<T> {
            let (tx, rx) = tokio::sync::mpsc::channel(128);
            let writer = Self { rx, sink };
            tokio::spawn(async move { writer.start().await });
            tx
        }

        async fn start(mut self) {
            while let Some(frame) = self.rx.recv().await {
                if let Err(err) = self.sink.send(frame).await {
                    println!("error sending frame: {}", err);
                    break;
                }
            }
        }
    }

    struct MuxFramedReader<T, S>
    where
        T: Display + Send + Sync + 'static,
        S: StreamExt<Item = Result<T, anyhow::Error>> + Send + Sync + std::marker::Unpin + 'static,
    {
        tx: tokio::sync::mpsc::Sender<T>,
        stream: S,
    }

    impl<T, S> MuxFramedReader<T, S>
    where
        T: Send + Sync + Display,
        S: StreamExt<Item = Result<T, anyhow::Error>> + Send + Sync + std::marker::Unpin + 'static,
    {
        pub fn new(stream: S) -> tokio::sync::mpsc::Receiver<T> {
            let (tx, rx) = tokio::sync::mpsc::channel(128);
            let reader = Self { tx, stream };
            tokio::spawn(async move { reader.start().await });
            rx
        }

        async fn start(mut self) {
            while let Some(Ok(t)) = self.stream.next().await {
                if let Err(err) = self.tx.send(t).await {
                    println!("read send err...{}", err);
                    break;
                }
            }
        }
    }

    #[tokio::test]
    pub async fn test_mux_framed_writer() {
        let x = LengthDelimitedCodec::new();

        let (read, write) = tokio_pipe::pipe().unwrap();

        let REQUEST: MuxedRequest = MuxedRequest::new(RegistryRequest::Scorch, 1_u64);

        let tx = {
            let write = FramedWrite::new(write, MuxedRequestCodec::default());
            MuxFramedWriter::new(write)
        };

        let mut read = FramedRead::new(read, MuxedRequestCodec::default());

        let mut read = MuxFramedReader::new(read);

        tx.send(REQUEST.clone()).await.unwrap();

        let from_read = tokio::time::timeout(Duration::from_secs(1), read.recv())
            .await
            .unwrap()
            .unwrap();

        println!("Received FROM! {}", from_read.request.to_string());

        assert_eq!(REQUEST, from_read);
    }

    pub struct MuxRegistryClient {
        sequence: AtomicU64,
        map: Arc<DashMap<u64, tokio::sync::oneshot::Sender<Result<RegistryResponse, RegErr>>>>,
        sink: tokio::sync::mpsc::Sender<MuxedRequest>,
    }

    impl MuxRegistryClient {
        pub fn local(registry: Arc<dyn RegistryApi>) -> Self {
            let (client_request_sink, client_request_stream) = tokio::sync::mpsc::channel(128);

            let (server_response_sink, server_response_stream) = tokio::sync::mpsc::channel(128);

            MuxRegistryServer::new(registry, client_request_stream, server_response_sink);

            Self::new(client_request_sink, server_response_stream)
        }

        pub fn new(
            sink: tokio::sync::mpsc::Sender<MuxedRequest>,
            stream: tokio::sync::mpsc::Receiver<MuxedResult>,
        ) -> Self {
            /// start the receiver
            let map = Arc::new(DashMap::new());
            MuxResultReceiver::new(stream, map.clone());

            let sequence = AtomicU64::new(0u64);
            Self {
                sequence,
                map,
                sink,
            }
        }
    }

    #[async_trait]
    impl Sender for MuxRegistryClient {
        async fn send<R, F>(&self, request: RegistryRequest, expect: F) -> Result<R, RegErr>
        where
            F: Fn(RegistryResponse) -> Result<R, RegErr> + Send + Sync,
        {
            let id = self.sequence.fetch_add(1u64, Ordering::Relaxed);
            let request = MuxedRequest::new(request, id);
            let (res_tx, res_rx) = tokio::sync::oneshot::channel();
            self.map.insert(id, res_tx);
            self.sink.send(request).await?;
            let response = res_rx.await??;
            expect(response)
        }
    }

    struct MuxResultReceiver {
        stream: tokio::sync::mpsc::Receiver<MuxedResult>,
        map: Arc<DashMap<u64, tokio::sync::oneshot::Sender<Result<RegistryResponse, RegErr>>>>,
    }

    impl MuxResultReceiver {
        pub fn new(
            stream: tokio::sync::mpsc::Receiver<MuxedResult>,
            map: Arc<DashMap<u64, tokio::sync::oneshot::Sender<Result<RegistryResponse, RegErr>>>>,
        ) {
            let mut runner = Self { stream, map };

            tokio::spawn(async move { runner.start().await });
        }

        async fn start(mut self) {
            while let Some(res) = self.stream.recv().await {
                if let Some((_, tx)) = self.map.remove(&res.id) {
                    tx.send(res.result.into());
                }
            }
        }
    }

    pub struct MuxRegistryServer {
        sink_tx: tokio::sync::mpsc::Sender<MuxedResult>,
        stream: tokio::sync::mpsc::Receiver<MuxedRequest>,
        tx: tokio::sync::mpsc::Sender<Exchange>,
    }

    impl MuxRegistryServer {
        pub fn new(
            registry: Arc<dyn RegistryApi>,
            stream: tokio::sync::mpsc::Receiver<MuxedRequest>,
            sink: tokio::sync::mpsc::Sender<MuxedResult>,
        ) {
            let tx = ExchangeRunner::new(registry);
            let (sink_tx, mut sink_rx) = tokio::sync::mpsc::channel(100);
            tokio::spawn(async move {
                while let Some(frame) = sink_rx.recv().await {
                    if let Err(_) = sink.send(frame).await {
                        break;
                    }
                }
            });
            let mut server = Self {
                stream,
                sink_tx,
                tx,
            };
            tokio::spawn(async move { server.start().await });
        }

        async fn start(mut self) {
            while let Some(req) = self.stream.recv().await {
                let tx = self.tx.clone();
                let sink = self.sink_tx.clone();
                tokio::spawn(async move {
                    let res = RegistryResult::from(send(tx, req.request).await);
                    let res = MuxedResult::new(res, req.id);
                    sink.send(res).await;

                    async fn send(
                        tx: tokio::sync::mpsc::Sender<Exchange>,
                        req: RegistryRequest,
                    ) -> Result<RegistryResponse, RegErr> {
                        let (exchange, mut rx) = Exchange::new(req);
                        tx.send(exchange).await?;
                        rx.await?
                    }
                });
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
    pub struct MuxedRequest {
        id: u64,
        request: RegistryRequest,
    }

    impl Display for MuxedRequest {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{} -> {}", self.id, self.request)
        }
    }

    impl MuxedRequest {
        pub fn new(request: RegistryRequest, id: u64) -> Self {
            Self { id, request }
        }
    }

    #[derive(Serialize, Deserialize)]
    pub struct MuxedResult {
        id: u64,
        result: RegistryResult,
    }

    impl MuxedResult {
        pub fn new(result: RegistryResult, id: u64) -> Self {
            Self { id, result }
        }
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub enum RegistryResult {
        Ok(RegistryResponse),
        Err(RegErr),
    }

    impl From<Result<RegistryResponse, RegErr>> for RegistryResult {
        fn from(result: Result<RegistryResponse, RegErr>) -> Self {
            match result {
                Ok(result) => Self::Ok(result),
                Err(err) => Self::Err(err),
            }
        }
    }

    impl Into<Result<RegistryResponse, RegErr>> for RegistryResult {
        fn into(self) -> Result<RegistryResponse, RegErr> {
            match self {
                RegistryResult::Ok(value) => Ok(value),
                RegistryResult::Err(value) => Err(value),
            }
        }
    }

    #[derive(Default)]
    struct MuxedRequestCodec(LengthDelimitedCodec);
    #[derive(Default)]
    struct MuxedResultCodec;

    impl Decoder for MuxedRequestCodec {
        type Item = MuxedRequest;
        type Error = anyhow::Error;

        fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
            use tokio_util::bytes::Buf;
            let mut data = self.0.decode(src)?.ok_or(anyhow::Error::msg("No data"))?;
            let read = bincode::deserialize(& mut data)?;
            Ok(Some(read))
        }
    }

    impl Encoder<MuxedRequest> for MuxedRequestCodec {
        type Error = anyhow::Error;

        fn encode(&mut self, item: MuxedRequest, dst: &mut BytesMut) -> Result<(), Self::Error> {
            let data = bincode::serialize(& item)?;
            self.0.encode(data.into(),dst)?;
            Ok(())
        }
    }
    impl Decoder for MuxedResultCodec {
        type Item = MuxedResult;
        type Error = anyhow::Error;

        fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
            Ok(Some(bincode::deserialize_from(&**src)?))
        }
    }

    impl Encoder<MuxedResult> for MuxedResultCodec {
        type Error = anyhow::Error;

        fn encode(&mut self, item: MuxedResult, dst: &mut BytesMut) -> Result<(), Self::Error> {
            bincode::serialize_into(&mut **dst, &item)?;
            Ok(())
        }
    }

    type MuxedRequestFrameWrite<T> = FramedWrite<T, MuxedRequestCodec>;
    type MuxedRequestFrameRead<T> = FramedRead<T, MuxedRequestCodec>;

    type MuxedResultFrameWrite<T> = FramedWrite<T, MuxedResultCodec>;
    type MuxedResultFrameRead<T> = FramedRead<T, MuxedResultCodec>;

    struct FramedSink<T, S, C>
    where
        C: Encoder<T>,
    {
        writer: FramedWrite<S, C>,
        _phantom: PhantomData<T>,
    }

    #[async_trait]
    trait Sender: Send + Sync {
        async fn send<R, F>(&self, request: RegistryRequest, expect: F) -> Result<R, RegErr>
        where
            F: Fn(RegistryResponse) -> Result<R, RegErr> + Send + Sync;
    }

    #[async_trait]
    impl<S: Sender> RegistryApi for S {
        async fn scorch<'a>(&'a self) -> Result<(), RegErr> {
            self.send(RegistryRequest::Scorch, transform::scorch).await
        }

        async fn register<'a>(&'a self, registration: &'a Registration) -> Result<(), RegErr> {
            let registration = registration.clone();
            self.send(RegistryRequest::Register(registration), transform::register)
                .await
        }

        async fn assign_star<'a>(
            &'a self,
            point: &'a Point,
            star: &'a Point,
        ) -> Result<(), RegErr> {
            let point = point.clone();
            let star = star.clone();
            self.send(
                RegistryRequest::AssignStar { point, star },
                transform::assign_star,
            )
            .await
        }

        async fn assign_host<'a>(
            &'a self,
            point: &'a Point,
            host: &'a Point,
        ) -> Result<(), RegErr> {
            let point = point.clone();
            let host = host.clone();
            self.send(
                RegistryRequest::AssignHost { point, host },
                transform::assign_host,
            )
            .await
        }

        async fn set_status<'a>(
            &'a self,
            point: &'a Point,
            status: &'a Status,
        ) -> Result<(), RegErr> {
            let point = point.clone();
            let status = status.clone();
            self.send(
                RegistryRequest::SetStatus { point, status },
                transform::set_status,
            )
            .await
        }

        async fn set_properties<'a>(
            &'a self,
            point: &'a Point,
            properties: &'a SetProperties,
        ) -> Result<(), RegErr> {
            let point = point.clone();
            let properties = properties.clone();
            let request = RegistryRequest::SetProperties { point, properties };
            self.send(request, transform::set_properties).await
        }

        async fn sequence<'a>(&'a self, point: &'a Point) -> Result<u64, RegErr> {
            let point = point.clone();
            let request = RegistryRequest::Sequence(point);
            self.send(request, transform::sequence).await
        }

        async fn get_properties<'a>(&'a self, point: &'a Point) -> Result<Properties, RegErr> {
            let point = point.clone();
            let request = RegistryRequest::GetProperties(point);
            self.send(request, transform::get_properties).await
        }

        async fn record<'a>(&'a self, point: &'a Point) -> Result<ParticleRecord, RegErr> {
            let point = point.clone();
            let request = RegistryRequest::Record(point);
            self.send(request, transform::record).await
        }

        async fn query<'a>(
            &'a self,
            point: &'a Point,
            query: &'a Query,
        ) -> Result<QueryResult, RegErr> {
            let point = point.clone();
            let query = query.clone();
            let request = RegistryRequest::Query { point, query };
            self.send(request, transform::query).await
        }

        async fn delete<'a>(&'a self, delete: &'a Delete) -> Result<SubstanceList, RegErr> {
            let delete = delete.clone();
            let request = RegistryRequest::Delete(delete);
            self.send(request, transform::delete).await
        }

        async fn select<'a>(&'a self, select: &'a mut Select) -> Result<SubstanceList, RegErr> {
            let select = select.clone();
            let request = RegistryRequest::Select(select);
            self.send(request, transform::select).await
        }

        async fn grant<'a>(&'a self, access_grant: &'a AccessGrant) -> Result<(), RegErr> {
            let access_grant = access_grant.clone();
            let request = RegistryRequest::Grant(access_grant);
            self.send(request, transform::grant).await
        }

        async fn access<'a>(&'a self, to: &'a Point, on: &'a Point) -> Result<Access, RegErr> {
            let to = to.clone();
            let on = on.clone();
            let request = RegistryRequest::Access { to, on };
            self.send(request, transform::access).await
        }

        async fn chown<'a>(
            &'a self,
            on: &'a Selector,
            owner: &'a Point,
            by: &'a Point,
        ) -> Result<(), RegErr> {
            let on = on.clone();
            let owner = owner.clone();
            let by = by.clone();
            let request = RegistryRequest::Chown { on, owner, by };
            self.send(request, transform::chown).await
        }

        async fn list_access<'a>(
            &'a self,
            to: &'a Option<&'a Point>,
            on: &'a Selector,
        ) -> Result<Vec<IndexedAccessGrant>, RegErr> {
            let to = to.map(|p| p.clone());
            let on = on.clone();
            let request = RegistryRequest::ListAccess { to, on };
            self.send(request, transform::list_access).await
        }

        async fn remove_access<'a>(&'a self, id: i32, to: &'a Point) -> Result<(), RegErr> {
            let to = to.clone();
            let request = RegistryRequest::RemoveAccess { id, to };
            self.send(request, transform::remove_access).await
        }
    }

    pub struct RegistryExchanger {
        tx: tokio::sync::mpsc::Sender<Exchange>,
    }

    impl RegistryExchanger {
        pub fn new(registry: Arc<dyn RegistryApi>) -> Self {
            let tx = ExchangeRunner::new(registry);
            Self { tx }
        }

        async fn xsend<R>(
            &self,
            request: RegistryRequest,
            expect: impl Fn(RegistryResponse) -> Result<R, RegErr>,
        ) -> Result<R, RegErr> {
            let (exchange, mut rx) = Exchange::new(request);
            self.tx.send(exchange).await?;
            let result = rx.await??;
            expect(result)
        }
    }

    #[async_trait]
    impl Sender for RegistryExchanger {
        async fn send<R, F>(&self, request: RegistryRequest, expect: F) -> Result<R, RegErr>
        where
            F: Fn(RegistryResponse) -> Result<R, RegErr> + Send + Sync,
        {
            let (exchange, mut rx) = Exchange::new(request);
            self.tx.send(exchange).await?;
            let result = rx.await??;
            expect(result)
        }
    }

    struct Exchange {
        pub request: RegistryRequest,
        pub tx: tokio::sync::oneshot::Sender<Result<RegistryResponse, RegErr>>,
    }

    impl Exchange {
        pub fn new(
            request: RegistryRequest,
        ) -> (
            Self,
            tokio::sync::oneshot::Receiver<Result<RegistryResponse, RegErr>>,
        ) {
            let (tx, rx) = tokio::sync::oneshot::channel();
            (Self { request, tx }, rx)
        }
    }

    struct ExchangeRunner {
        rx: tokio::sync::mpsc::Receiver<Exchange>,
        registry: Arc<dyn RegistryApi>,
    }

    impl ExchangeRunner {
        pub fn new(registry: Arc<dyn RegistryApi>) -> tokio::sync::mpsc::Sender<Exchange> {
            let (tx, rx) = tokio::sync::mpsc::channel(128);
            let mut runner = Self { rx, registry };
            tokio::spawn(async move {
                runner.start().await;
            });
            tx
        }
        pub async fn start(mut self) {
            while let Some(x) = self.rx.recv().await {
                let registry = self.registry.clone();
                tokio::spawn(async move {
                    let result: Result<RegistryResponse, RegErr> = match x.request {
                        RegistryRequest::Scorch => {
                            registry.scorch().await.map(|_| RegistryResponse::Scorch)
                        }
                        RegistryRequest::Register(registration) => registry
                            .register(&registration)
                            .await
                            .map(|_| RegistryResponse::Register),
                        RegistryRequest::AssignStar { point, star } => registry
                            .assign_star(&point, &star)
                            .await
                            .map(|_| RegistryResponse::AssignStar),
                        RegistryRequest::AssignHost { point, host } => registry
                            .assign_host(&point, &host)
                            .await
                            .map(|_| RegistryResponse::AssignHost),
                        RegistryRequest::SetStatus { point, status } => registry
                            .set_status(&point, &status)
                            .await
                            .map(|_| RegistryResponse::SetStatus),
                        RegistryRequest::SetProperties { point, properties } => registry
                            .set_properties(&point, &properties)
                            .await
                            .map(|_| RegistryResponse::SetProperties),
                        RegistryRequest::Sequence(point) => registry
                            .sequence(&point)
                            .await
                            .map(|value| RegistryResponse::Sequence(value)),
                        RegistryRequest::GetProperties(point) => registry
                            .get_properties(&point)
                            .await
                            .map(|value| RegistryResponse::GetProperties(value)),
                        RegistryRequest::Record(point) => registry
                            .record(&point)
                            .await
                            .map(|value| RegistryResponse::Record(value)),
                        RegistryRequest::Query { point, query } => registry
                            .query(&point, &query)
                            .await
                            .map(|value| RegistryResponse::Query(value)),
                        RegistryRequest::Delete(delete) => registry
                            .delete(&delete)
                            .await
                            .map(|value| RegistryResponse::Delete(value)),
                        RegistryRequest::Select(mut select) => registry
                            .select(&mut select)
                            .await
                            .map(|value| RegistryResponse::Select(value)),
                        RegistryRequest::Grant(access_grant) => registry
                            .grant(&access_grant)
                            .await
                            .map(|_| RegistryResponse::Grant),
                        RegistryRequest::Access { to, on } => registry
                            .access(&to, &on)
                            .await
                            .map(|value| RegistryResponse::Access(value)),
                        RegistryRequest::Chown { on, owner, by } => registry
                            .chown(&on, &owner, &by)
                            .await
                            .map(|_| RegistryResponse::Chown),
                        RegistryRequest::ListAccess { to, on } => registry
                            .list_access(&to.as_ref(), &on)
                            .await
                            .map(|value| RegistryResponse::ListAccess(value)),
                        RegistryRequest::RemoveAccess { id, to } => registry
                            .remove_access(id, &to)
                            .await
                            .map(|_| RegistryResponse::RemoveAccess),
                    };
                    x.tx.send(result).unwrap();
                });
            }
        }
    }
}

#[cfg(test)]
pub mod test {
    use super::*;
    use crate::hyperlane::HyperwayKind::Mount;
    use crate::registry::exchange::{MuxRegistryClient, RegistryExchanger};
    use mockall::mock;
    use starlane_space::wave::exchange::asynch::Exchanger;

    mock! {
        pub Registry{
        }

        #[async_trait]
        impl RegistryApi for Registry{
        async fn scorch<'a>(&'a self) -> Result<(), RegErr>;

        async fn register<'a>(&'a self, registration: &'a Registration) -> Result<(), RegErr>;

        async fn assign_star<'a>(&'a self, point: &'a Point, star: &'a Point) -> Result<(), RegErr>;

        async fn assign_host<'a>(&'a self, point: &'a Point, host: &'a Point) -> Result<(), RegErr>;

        async fn set_status<'a>(&'a self, point: &'a Point, status: &'a Status) -> Result<(), RegErr>;

        async fn set_properties<'a>(
            &'a self,
            point: &'a Point,
            properties: &'a SetProperties,
        ) -> Result<(), RegErr>;

        async fn sequence<'a>(&'a self, point: &'a Point) -> Result<u64, RegErr>;

        async fn get_properties<'a>(&'a self, point: &'a Point) -> Result<Properties, RegErr>;

        async fn record<'a>(&'a self, point: &'a Point) -> Result<ParticleRecord, RegErr>;

        async fn query<'a>(&'a self, point: &'a Point, query: &'a Query)
            -> Result<QueryResult, RegErr>;

        async fn delete<'a>(&'a self, d: &'a Delete) -> Result<SubstanceList, RegErr>;

        //    async fn select<'a>(&'a self, select: &'a mut Select) -> Result<SubstanceList, RegErr>;

        async fn select<'a>(&'a self, select: &'a mut Select) -> Result<SubstanceList, RegErr>;

        //async fn sub_select<'a>(&'a self, sub_select: &'a SubSelect) -> Result<Vec<Stub>, RegErr>;

        async fn grant<'a>(&'a self, access_grant: &'a AccessGrant) -> Result<(), RegErr>;

        async fn access<'a>(&'a self, to: &'a Point, on: &'a Point) -> Result<Access, RegErr>;

        async fn chown<'a>(
            &'a self,
            on: &'a Selector,
            owner: &'a Point,
            by: &'a Point,
        ) -> Result<(), RegErr>;

        async fn list_access<'a>(
            &'a self,
            to: &'a Option<&'a Point>,
            on: &'a Selector,
        ) -> Result<Vec<IndexedAccessGrant>, RegErr>;

        async fn remove_access<'a>(&'a self, id: i32, to: &'a Point) -> Result<(), RegErr>;
        }
    }
    #[tokio::test]
    pub async fn test_mock() {
        let mock = mock();
        //        let registry = RegistryExchanger::new(mock);
        test_registry(mock).await.unwrap();
    }

    #[tokio::test]
    pub async fn test_exchanger() {
        let mock = Arc::new(mock());
        let registry = RegistryExchanger::new(mock.clone());
        test_registry(registry).await.unwrap();
        drop(mock);
    }

    #[tokio::test]
    pub async fn test_muxer() {
        let blah = Arc::new(1u64);

        let mock = Arc::new(mock());
        let registry = MuxRegistryClient::local(mock.clone());
        test_registry(registry).await.unwrap();
    }

    pub fn mock() -> MockRegistry {
        let mut mock = MockRegistry::new();
        mock.expect_scorch().times(1).returning(|| Ok(()));
        mock.expect_register().times(1).returning(|x| Ok(()));

        mock.expect_assign_host().times(1).returning(|_, _| Ok(()));
        mock.expect_assign_star().times(1).returning(|_, _| Ok(()));
        mock.expect_delete()
            .times(1)
            .returning(|_| Ok(SubstanceList::default()));
        mock.expect_select()
            .times(1)
            .returning(|_| Ok(SubstanceList::default()));
        mock.expect_query()
            .times(1)
            .returning(|_, _| Ok(QueryResult::mock()));
        mock.expect_get_properties()
            .times(1)
            .returning(|_| Ok(Properties::new()));
        mock.expect_sequence().times(1).returning(|_| Ok(64u64));
        mock.expect_set_properties()
            .times(1)
            .returning(|_, _| Ok(()));
        mock.expect_set_status().times(1).returning(|_, _| Ok(()));
        mock.expect_record()
            .times(1)
            .returning(|_| Ok(ParticleRecord::mock()));

        mock
    }

    async fn test_registry(mock: impl RegistryApi + Send + Sync + 'static) -> anyhow::Result<()> {
        mock.scorch().await.unwrap();
        mock.register(&Registration::mock()).await.unwrap();
        mock.assign_host(&Point::root(), &Point::root())
            .await
            .unwrap();
        mock.assign_star(&Point::root(), &Point::root())
            .await
            .unwrap();
        mock.delete(&Delete::mock()).await.unwrap();
        mock.select(&mut Select::mock()).await.unwrap();
        mock.query(&Point::root(), &Query::mock()).await.unwrap();
        mock.get_properties(&Point::root()).await.unwrap();
        mock.sequence(&Point::root()).await.unwrap();
        mock.set_properties(&Point::root(), &SetProperties::new())
            .await
            .unwrap();
        mock.set_status(&Point::hyper_user(), &Status::Ready)
            .await
            .unwrap();
        mock.record(&Point::global_depedencies()).await.unwrap();
        Ok(())
    }
}
