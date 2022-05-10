use std::convert::TryInto;
use std::fmt::{Debug, Formatter};
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use futures::future::join_all;
use futures::SinkExt;
use mesh_portal::version::latest::command::common::{PropertyMod, SetProperties, SetRegistry};
use mesh_portal::version::latest::entity::request::query::{Query, QueryResult};
use mesh_portal::version::latest::entity::request::{Method, Rc};
use mesh_portal::version::latest::id::{Point, Specific, Version};
use mesh_portal::version::latest::messaging::Request;
use mesh_portal::version::latest::selector::specific::{ProductSelector, VariantSelector, VendorSelector};
use mesh_portal::version::latest::payload::{Payload, Primitive, PrimitiveList};
use mesh_portal::version::latest::particle::{Property, Stub, Status};
use mesh_portal::version::latest::util::ValuePattern;
use mesh_portal_versions::version::v0_0_1::util::unique_id;
use rusqlite::{Connection, params_from_iter, Row, Transaction};
use  rusqlite::params;
use rusqlite::types::ValueRef;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex};
use tokio::sync::oneshot;
use async_recursion::async_recursion;
use mesh_portal::version::latest::entity::request::select::Select;
use mesh_portal::version::latest::selector::{ExactSegment, GenericKindSelector, KindSelector, PointKindHierarchy, PointKindSeg, PointSegSelector};
use mesh_portal_versions::version::v0_0_1::entity::request::select::{SelectKind, SubSelect};

use crate::error::Error;
use crate::fail::{Fail, StarlaneFailure};
use crate::frame::StarMessagePayload;
use crate::logger::LogInfo;

use crate::message::{ProtoStarMessage, ProtoStarMessageTo, Reply, ReplyKind};
use crate::particle;
use crate::star::{StarKey, StarSkel};
use crate::util::{AsyncProcessor, AsyncRunner, Call};
use crate::particle::{ParticleRecord, AssignResourceStateSrc, Particle, ParticleAssign, AssignKind, ParticleLocation, KindBase, Kind};


static RESOURCE_QUERY_FIELDS: &str = "parent,point_segment,resource_type,kind,vendor,product,variant,version,version_variant,shell,status";

#[derive(Clone)]
pub struct RegistryApi {
    pub tx: mpsc::Sender<RegistryCall>,
}

impl RegistryApi {
    pub fn new(tx: mpsc::Sender<RegistryCall>) -> Self {
        Self { tx }
    }

    pub async fn register( &self, registration: Registration ) -> Result<Stub,RegError> {
        let (tx,rx) = oneshot::channel();
        self.tx.send(RegistryCall::Register {registration, tx }).await?;

        let result = rx.await;
        result?
    }

    pub async fn assign(&self, point: Point, host: StarKey) -> Result<(),Error> {
        let (tx,rx) = oneshot::channel();
        self.tx.send(RegistryCall::Assign{point, host, tx }).await?;
        rx.await?
    }


    pub async fn select(&self, select: Select) -> Result<PrimitiveList,Error> {
        let (tx,rx) = oneshot::channel();
        self.tx.send(RegistryCall::Selector {select, tx }).await;
        let mut selector = rx.await?;
        let result = selector.select().await;
        match &result {
            Ok(_) => {
                println!("Select Ok");
            }
            Err(err) => {
                println!("Select Err: {}", err.to_string());
            }
        }
        result
    }

    pub async fn query(&self, point: Point, query: Query ) -> Result<QueryResult,Error> {
        let (tx,rx) = oneshot::channel();
        self.tx.send(RegistryCall::Query {point, query, tx }).await;
        rx.await?
    }

    pub async fn set_status(&self, point: Point, status: Status ) -> Result<(),Error> {
        let (tx,rx) = oneshot::channel();
        self.tx.send(RegistryCall::SetStatus{point, status, tx }).await;
        rx.await?
    }

    pub async fn set_properties(&self, point: Point, properties: SetProperties) -> Result<(),Error> {
        let (tx,rx) = oneshot::channel();
        self.tx.send(RegistryCall::SetProperties {point, properties, tx }).await;
        rx.await?
    }

    pub async fn get_properties(&self, point: Point, keys: Vec<String>) -> Result<Vec<(String, String)>,Error> {
        let (tx,rx) = oneshot::channel();
        self.tx.send(RegistryCall::GetProperties {point, keys, tx }).await;
        rx.await?
    }

    pub async fn locate(&self, point: Point) -> Result<ParticleRecord,Error> {
        let (tx,rx) = oneshot::channel();
        self.tx.send(RegistryCall::Locate{point, tx }).await;
        rx.await?
    }

    pub async fn sequence(&self, point: Point) -> Result<u64,Error> {
        let (tx,rx) = oneshot::channel();
        self.tx.send(RegistryCall::Sequence{point, tx }).await;
        rx.await?
    }


}

pub enum RegistryCall {
    Assign{point: Point, host: StarKey, tx: oneshot::Sender<Result<(),Error>>},
    Register{registration:Registration, tx: oneshot::Sender<Result<Stub,RegError>>},
    Selector {select: Select, tx: oneshot::Sender<Selector>},
    Query { point: Point, query: Query, tx: oneshot::Sender<Result<QueryResult,Error>>},
    SetStatus { point: Point, status: Status, tx: oneshot::Sender<Result<(),Error>>},
    SetProperties { point: Point, properties: SetProperties, tx: oneshot::Sender<Result<(),Error>>},
    GetProperties { point: Point, keys: Vec<String>, tx: oneshot::Sender<Result<Vec<(String, String)>,Error>>},
    Locate{ point: Point, tx: oneshot::Sender<Result<ParticleRecord,Error>>},
    Sequence{ point: Point, tx: oneshot::Sender<Result<u64,Error>>},
}

impl Call for RegistryCall {}

pub struct RegistryComponent {
    skel: StarSkel,
    conn: Arc<Mutex<Connection>>
}

impl RegistryComponent {
    pub fn start(skel: StarSkel, mut rx: mpsc::Receiver<RegistryCall>) {
        tokio::spawn(async move {

            let mut conn = Connection::open_in_memory().expect("expected to get sqlite database connection");
            match setup(&mut conn) {
                Ok(_) => {}
                Err(err) => {
                    println!("Fatal error in setup: {}", err.to_string());
                }
            }

            let conn = Arc::new(Mutex::new(conn));

            let mut registry = RegistryComponent{
                skel,
                conn
            };

            while let Option::Some(call) = rx.recv().await {
                registry.process(call).await;
            }
        });
    }

    async fn process(&mut self, call: RegistryCall) {
        match call {
            RegistryCall::Register { registration, tx } => {
                self.register(registration,tx).await;
            }
            RegistryCall::Selector { select,  tx} => {
                let selector = Selector {
                    conn: self.conn.clone(),
                    skel: self.skel.clone(),
                    select
                };
                tx.send(selector);
            }
            RegistryCall::Query { point, query, tx } => {
                self.query(point, query, tx).await
            }
            RegistryCall::SetStatus{ point,status,tx } => {
                self.set_status(point, status, tx).await;
            }
            RegistryCall::SetProperties { point, properties, tx } => {
                self.set_properties(point, properties, tx).await;
            }
            RegistryCall::Locate { point, tx } => {
                self.locate(point,tx ).await;
            }
            RegistryCall::Sequence { point, tx } => {
                self.sequence(point,tx).await;
            }
            RegistryCall::Assign { point, host, tx } => {
                self.assign(point,host, tx).await;
            }
            RegistryCall::GetProperties { point, keys, tx } => {
                self.get_properties(point, keys, tx).await;
            }
        }
    }
}

impl RegistryComponent {

    async fn set_status(&mut self, point: Point, status: Status, tx: oneshot::Sender<Result<(),Error>>) {
        async fn process(conn: Arc<Mutex<Connection>>, point: Point, status: Status ) -> Result<(),Error> {
            let parent = point.parent().ok_or("particle must have a parent")?.to_string();
            let point_segment = point.last_segment().ok_or("particle must have a last segment")?.to_string();
            let status = status.to_string();
            let statement = "UPDATE resources SET status=?1 WHERE parent=?2 AND point_segment=?3";
            {
                let conn = conn.lock().await;
                let mut statement = conn.prepare(statement)?;
                statement.execute(params!(status,parent,point_segment))?;
            }
            Ok(())
        }
        tx.send(process(self.conn.clone(), point,status).await );
    }

    async fn get_properties(&mut self, point: Point, keys: Vec<String>, tx: oneshot::Sender<Result<Vec<(String, String)>,Error>>) {

        async fn process(conn: Arc<Mutex<Connection>>, point: Point, keys: Vec<String>) -> Result<Vec<(String, String)>,Error> {
            let conn = conn.lock().await;
            let parent = point.parent().ok_or("particle must have a parent")?.to_string();
            let point_segment = point.last_segment().ok_or("particle must have a last segment")?.to_string();

            let mut properties = vec![];
            for key in keys {
                let property = conn.query_row("SELECT key,value FROM properties WHERE resource_id=(SELECT id FROM resources WHERE parent=?1 AND point_segment=?2) AND key=?3", params![parent,point_segment,key], RegistryComponent::process_property )?;
                if property.is_some() {
                    properties.push(property.expect("property"));
                }
            }

            Ok(properties)
        }

        let result = process(self.conn.clone(), point,keys).await;

        match &result {
            Ok(_) => { }
            Err(err) => {
                eprintln!("Get Properties error: {}", err.to_string());
            }
        }

        tx.send(result);
    }


    async fn set_properties(&mut self, point: Point, properties: SetProperties, tx: oneshot::Sender<Result<(),Error>>) {

        async fn process(conn: Arc<Mutex<Connection>>, point: Point, properties: SetProperties) -> Result<(),Error> {
            let conn = conn.lock().await;
            let parent = point.parent().ok_or("particle must have a parent")?.to_string();
            let point_segment = point.last_segment().ok_or("particle must have a last segment")?.to_string();

            for (_,property_mod) in properties.iter() {
                match property_mod {
                    PropertyMod::Set { key, value ,lock } => {
                        let lock = match *lock {
                            true => 1,
                            false => 0
                        };
                        conn.execute("INSERT INTO properties (resource_id,key,value,lock) VALUES ((SELECT id FROM resources WHERE parent=?1 AND point_segment=?2),?3,?4,?5) ON CONFLICT(resource_id,key) DO UPDATE SET value=?4 WHERE lock=0", params![parent,point_segment,key.to_string(),value.to_string(),lock])?;
                    }
                    PropertyMod::UnSet(key) => {
                        conn.execute("DELETE FROM properties WHERE resource_id=(SELECT id FROM resources WHERE parent=?1 AND point_segment=?2) AND key=?3 AND lock=0", params![parent,point_segment,key.to_string()])?;
                    }
                }
            }
            Ok(())
        }

        let result = process(self.conn.clone(), point,properties).await;

        match &result {
            Ok(_) => { }
            Err(err) => {
                eprintln!("Set Properties error: {}", err.to_string());
            }
        }

        tx.send(result);
    }

    async fn locate(&mut self, point: Point, tx: oneshot::Sender<Result<ParticleRecord,Error>>) {
        tx.send(Self::locate_inner(self.conn.clone(), point).await );
    }

    async fn locate_inner(conn: Arc<Mutex<Connection>>, point: Point) -> Result<ParticleRecord,Error> {
        let conn = conn.lock().await;
        let statement = format!( "SELECT DISTINCT {} FROM resources as r WHERE parent=?1 AND point_segment=?2", RESOURCE_QUERY_FIELDS );
        let mut statement = conn.prepare(statement.as_str())?;
        let parent = point.parent().ok_or("expected a parent")?;
        let point_segment = point.last_segment().ok_or("expected last point_segment")?.to_string();
        let mut record = statement.query_row(params!(parent.to_string(),point_segment), RegistryComponent::process_resource_row_catch)?;
        let mut statement = conn.prepare("SELECT key,value,lock FROM properties WHERE resource_id=(SELECT id FROM resources WHERE parent=?1 AND point_segment=?2)")?;
        let mut rows = statement.query(params!(parent.to_string(),point_segment))?;
        while let Option::Some(row) = rows.next()? {
            let key: String = row.get(0)?;
            let value: String = row.get(1)?;
            let locked: usize= row.get(2)?;
            let locked = match locked {
                0 => false,
                _ => true
            };
            let property = Property {key,value,locked};
            record.stub.properties.insert( property.key.clone(), property );
        }

        Ok(record)
    }

    async fn sequence(&mut self, point: Point, tx: oneshot::Sender<Result<u64,Error>>) {
        async fn process(skel: StarSkel, conn:Arc<Mutex<Connection>>, point: Point) -> Result<u64, Error> {
            let conn = conn.lock().await;
            let parent = point.parent().ok_or("expecting parent since we have already established the segments are >= 2")?;
            let point_segment = point.last_segment().ok_or("expecting a last_segment since we know segments are >= 2")?;
            conn.execute("UPDATE resources SET sequence=sequence+1 WHERE parent=?1 AND point_segment=?2",params![parent.to_string(),point_segment.to_string()])?;
            Ok(conn.query_row( "SELECT DISTINCT sequence FROM resources WHERE parent=?1 AND point_segment=?2",params![parent.to_string(),point_segment.to_string()], RegistryComponent::process_sequence)?)
        }
        tx.send(process(self.skel.clone(), self.conn.clone(), point).await);
    }

    async fn assign(&mut self, point: Point, host: StarKey, tx: oneshot::Sender<Result<(),Error>>) {
        async fn process(conn:Arc<Mutex<Connection>>, point: Point, host: StarKey) -> Result<(), Error> {
            let conn = conn.lock().await;
            let parent = point.parent().ok_or("expecting parent since we have already established the segments are >= 2")?;
            let point_segment = point.last_segment().ok_or("expecting a last_segment since we know segments are >= 2")?;
            conn.execute("UPDATE resources SET shell=?1 WHERE parent=?2 AND point_segment=?3",params![host.to_string(),parent.to_string(),point_segment.to_string()])?;
            Ok(())
        }
        tx.send(process(self.conn.clone(), point, host).await);
    }


    async fn query(&mut self, point: Point, query: Query, tx: oneshot::Sender<Result<QueryResult,Error>>) {
        async fn process(skel: StarSkel, conn:Arc<Mutex<Connection>>, point: Point) -> Result<QueryResult, Error> {

            if point.segments.len() == 0 {
/*                let segment = AddressKindSegment {
                    point_segment: AddressSegment::Root,
                    kind: Kind::Root.into()
                };

 */
                return Ok(QueryResult::AddressKindPath(PointKindHierarchy::new(
                    point.route.clone(),
                    vec![]
                )));
            }
            else if point.segments.len() == 1 {
                let segment = PointKindSeg {
                    point_segment: point.last_segment().expect("expected at least one segment"),
                    kind: Kind::Space.into()
                };
                return Ok(QueryResult::AddressKindPath(PointKindHierarchy::new(
                    point.route.clone(),
                     vec![segment]
                )));
            }

            let parent = point.parent().expect("expecting parent since we have already established the segments are >= 2");
            let point_segment = point.last_segment().expect("expecting a last_segment since we know segments are >= 2");
            let request= Request::new(Method::Rc(Rc::Query(Query::AddressKindPath)).into(), skel.info.point.clone(), parent.clone() );

            let response = skel.messaging_api.request(request).await;

            let parent_kind_path = response.core.body;
            let parent_kind_path: Primitive= parent_kind_path.try_into()?;
            let parent_kind_path: String= parent_kind_path.try_into()?;

            let parent_kind_path = PointKindHierarchy::from_str(parent_kind_path.as_str())?;


            let mut record = {
                let conn = conn.lock().await;
                let statement = format!("SELECT DISTINCT {} FROM resources as r WHERE parent=?1 AND point_segment=?2", RESOURCE_QUERY_FIELDS);
                let mut statement = conn.prepare(statement.as_str())?;
                statement.query_row(params!(parent.to_string(),point_segment.to_string()), RegistryComponent::process_resource_row_catch)?
            };
                let segment = PointKindSeg {
                    point_segment: record.stub.point.last_segment().expect("expected at least one segment"),
                    kind: record.stub.kind
                };

                let path = parent_kind_path.push(segment);
                let result = QueryResult::AddressKindPath(path);

                Ok(result)
        }

                let skel = self.skel.clone();
                tx.send(process(skel, self.conn.clone(), point).await);
        }


    async fn register( &mut self, registration: Registration, tx: oneshot::Sender<Result<Stub,RegError>>) {
        fn check<'a>( registration: &Registration,  trans:&Transaction<'a>, ) -> Result<(),RegError> {
            let params = RegistryParams::from_registration(registration)?;
            let count = trans.query_row("SELECT count(*) as count from resources WHERE parent=?1 AND point_segment=?2", params![params.parent, params.point_segment], RegistryComponent::count )?;
            if count > 0 {
                Err(RegError::Dupe)
            } else {
                Ok(())
            }
        }
        fn register<'a>( registration: Registration,  trans:&Transaction<'a>,) -> Result<(),Error> {

            let params = RegistryParams::from_registration(&registration)?;
            trans.execute("INSERT INTO resources (point_segment,resource_type,kind,vendor,product,variant,version,version_variant,parent,status) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,'Pending')", params![params.point_segment,params.resource_type,params.kind,params.vendor,params.product,params.variant,params.version,params.version_variant,params.parent])?;

            fn set_properties(params: &RegistryParams, props: &SetProperties, trans: &Transaction ) -> Result<(),Error> {
                for (_,property_mod) in props.iter() {
                    match property_mod {
                        PropertyMod::Set{ key, value,lock } => {
                            let lock:usize = match lock {
                                true => 1,
                                false => 0
                            };
                            trans.execute("INSERT INTO properties (resource_id,key,value,lock) VALUES ((SELECT id FROM resources WHERE parent=?1 AND point_segment=?2),?3,?4,?5)", params![params.parent,params.point_segment,key.to_string(),value.to_string(),lock])?;
                        }
                        PropertyMod::UnSet(key) => {
                            trans.execute("DELETE FROM properties WHERE resource_id=(SELECT id FROM resources WHERE parent=?1 AND point_segment=?2) AND key=?3 AND lock=0", params![params.parent,params.point_segment,key.to_string()])?;
                        }
                    }
                }
                Ok(())
            }

            set_properties(&params, &registration.properties, &trans )?;

           Ok(())
        }

        let point = registration.point.clone();
        let result = {
            let mut conn = self.conn.lock().await;
            let mut trans = match conn.transaction() {
                Ok(trans) => trans,
                Err(error) => {
                    tx.send(Err(RegError::Error(error.into())));
                    return;
                }
            };

            match check(&registration, &trans) {
                Ok(_) => {}
                Err(error) => {
                    tx.send(Err(error));
                    return;
                }
            }

            let result = register(registration, &trans);
            match result {
                Ok(_) => {
                    trans.commit();
                }
                Err(_) => {
                    trans.rollback();
                }
            }
            result
        };
        match result {
            Ok(_) => {
                let (otx,rx) = oneshot::channel();
                self.locate(point.clone(),otx).await;
                tx.send(match rx.await {
                    Ok(Ok(record)) => {
                        Ok(record.into())
                    }
                    Ok(Err(err)) => {
                        error!("could not locate record '{}'", point.to_string());
                        Err(format!("Registry: could not locate record: {}",point.to_string()).as_str().into())
                    }

                    Err(err) => {
                        error!("could not locate record '{}'", point.to_string());
                        Err(format!("Registry: could not locate record: {}",point.to_string()).as_str().into())
                    }
                });
            }
            Err(err) => {
                error!("could not locate record '{}'", point.to_string());
                tx.send(Err(RegError::Error(err)));
            }
        }
    }

    fn process_sequence(row: &Row) -> Result<u64, rusqlite::Error> {
        let sequence: u64= row.get(0)?;
        Ok(sequence)
    }

    fn process_resource_row_catch(row: &Row) -> Result<ParticleRecord, rusqlite::Error> {
        match Self::process_resource_row(row) {
            Ok(ok) => Ok(ok),
            Err(error) => {
                error!("process_resource_rows: {}", error);
                Err(error.into())
            }
        }
    }

    fn process_property(row: &Row) -> Result<Option<(String,String)>, rusqlite::Error> {
            fn opt(row: &Row, index: usize) -> Result<Option<String>, rusqlite::Error>
            {
                if let ValueRef::Null = row.get_ref(index)? {
                   Ok(Option::None)
                } else {
                   let specific: String = row.get(index)?;
                   Ok(Option::Some(specific))
                }
            }

            let key= opt(row,0)?;
            let value= opt(row,1)?;
            if key.is_some() && value.is_some() {
                Ok(Option::Some((key.expect("key"), value.expect("value"))))
            } else {
                Ok(Option::None)
            }
    }

    fn count(row: &Row) -> Result<usize, rusqlite::Error> {
        let count: usize= row.get(0)?;
        Ok(count)
    }

    //    static RESOURCE_QUERY_FIELDS: &str = "parent,point_segment,resource_type,kind,vendor,product,variant,version,version_variant,shell,status";
    fn process_resource_row(row: &Row) -> Result<ParticleRecord, Error> {
            fn opt(row: &Row, index: usize) -> Result<Option<String>, Error>
            {
                if let ValueRef::Null = row.get_ref(index)? {
                    Ok(Option::None)
                } else {
                    let specific: String = row.get(index)?;
                    Ok(Option::Some(specific))
                }
            }

            let parent: String = row.get(0)?;
            let point_segment: String = row.get(1)?;
            let resource_type: String = row.get(2)?;
            let kind: Option<String> = opt(row,3 )?;
            let vendor: Option<String> = opt(row, 4)?;
            let product: Option<String> = opt(row, 5)?;
            let variant: Option<String> = opt(row, 6)?;
            let version: Option<String> = opt(row, 7)?;
            let version_variant: Option<String> = opt(row, 8)?;
            let host: Option<String> = opt(row, 9)?;
            let status: String = row.get(10)?;

            let point = Point::from_str(parent.as_str())?;
            let point = point.push(point_segment)?;
            let resource_type = KindBase::from_str(resource_type.as_str())?;
            let specific = if let Option::Some(vendor) = vendor {
                if let Option::Some(product) = product {
                    if let Option::Some(variant) = variant {
                        if let Option::Some(version) = version {
                            let version = if let Option::Some(version_variant) = version_variant {
                                let version = format!("{}-{}", version, version_variant);
                                Version::from_str(version.as_str())?
                            } else {
                                Version::from_str(version.as_str())?
                            };

                            Option::Some(Specific {
                                vendor,
                                product,
                                variant,
                                version
                            })
                        } else {
                            Option::None
                        }
                    } else {
                        Option::None
                    }
                } else {
                    Option::None
                }
            } else {
                Option::None
            };

            let kind = Kind::from(resource_type, kind, specific)?;
            let host = match host {
                Some(host) => {
                    ParticleLocation::Star(StarKey::from_str(host.as_str())?)
                }
                None => {
                    ParticleLocation::Unassigned
                }
            };
            let status = Status::from_str(status.as_str())?;

            let stub = Stub {
                point,
                kind: kind.into(),
                properties: Default::default(), // not implemented yet...
                status
            };

            let record = ParticleRecord {
                stub: stub,
                location: host,
            };

            Ok(record)

    }

}

/*
#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct SubSelect {
    pub pattern: AddressKindPattern,
    pub point: Address,
    pub hops: Vec<Hop>,
    pub point_tks_path: AddressTksPath
}

 */



#[derive(Clone)]
pub struct Registration {
    pub point: Point,
    pub kind: Kind,
    pub registry: SetRegistry,
    pub properties: SetProperties,
    pub owner: Point
}

pub struct RegistryParams {
    pub point: String,
    pub point_segment: String,
    pub resource_type: String,
    pub kind: Option<String>,
    pub vendor: Option<String>,
    pub product: Option<String>,
    pub variant: Option<String>,
    pub version: Option<String>,
    pub version_variant: Option<String>,
    pub parent: String,
    pub owner: Point,
}

impl RegistryParams {
    pub fn from_registration(registration: &Registration ) -> Result<Self, Error> {

        let point_segment = match registration.point.segments.last() {
            None => {"".to_string()}
            Some(segment) => {
                segment.to_string()
            }
        };
        let parent = match registration.point.parent()  {
            None => {"".to_string()}
            Some(parent) => {parent.to_string()}
        };

        let resource_type = registration.kind.kind().to_string();
        let kind = registration.kind.sub_kind();
        let vendor = match &registration.kind.specific() {
            None => Option::None,
            Some(specific) => Option::Some(specific.vendor.clone()),
        };

        let product= match &registration.kind.specific() {
            None => Option::None,
            Some(specific) => Option::Some(specific.product.clone()),
        };

        let variant = match &registration.kind.specific() {
            None => Option::None,
            Some(specific) => Option::Some(specific.variant.clone()),
        };

        let version = match &registration.kind.specific() {
            None => Option::None,
            Some(specific) =>  {
                let version = &specific.version;
                Option::Some(format!( "{}.{}.{}", version.major, version.minor, version.patch ))
            }
        };

        let version_variant = match &registration.kind.specific() {
            None => Option::None,
            Some(specific) =>  {
                let version = &specific.version;
                if version.is_prerelease() {
                    let mut pre = String::new();
                    for (i, x) in version.pre.iter().enumerate() {
                        if i != 0 {
                            pre.push_str(".");
                        }
                        pre.push_str(format!("{}", x).as_ref());
                    }
                    Option::Some(pre)
                } else {
                    Option::None
                }
            }
        };

        Ok(RegistryParams {
            point: registration.point.to_string(),
            point_segment: point_segment,
            parent,
            resource_type,
            kind,
            vendor,
            product,
            variant,
            version,
            version_variant,
            owner: registration.owner.clone()
        })
    }
}





fn setup(conn: &mut Connection) -> Result<(), Error> {


    let resources = r#"CREATE TABLE IF NOT EXISTS resources (
         id INTEGER PRIMARY KEY AUTOINCREMENT,
         point_segment TEXT NOT NULL,
         parent TEXT NOT NULL,
         resource_type TEXT NOT NULL,
         kind TEXT,
         vendor TEXT,
         product TEXT,
         variant TEXT,
         version TEXT,
         version_variant TEXT,
         shell TEXT,
         status TEXT NOT NULL,
         sequence INTEGER DEFAULT 0,
         UNIQUE(parent,point_segment)
        )"#;

    let labels = r#"
       CREATE TABLE IF NOT EXISTS labels (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
	      resource_id INTEGER NOT NULL,
	      key TEXT NOT NULL,
	      value TEXT,
          UNIQUE(key,value),
          FOREIGN KEY (resource_id) REFERENCES resources (id)
        )"#;

    /// note that a tag may reference an point NOT in this database
    /// therefore it does not have a FOREIGN KEY constraint
    let tags = r#"
       CREATE TABLE IF NOT EXISTS tags(
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          parent TEXT NOT NULL,
          tag TEXT NOT NULL,
          point TEXT NOT NULL,
          UNIQUE(tag)
        )"#;


    let properties = r#"CREATE TABLE IF NOT EXISTS properties (
         id INTEGER PRIMARY KEY AUTOINCREMENT,
	     resource_id INTEGER NOT NULL,
         key TEXT NOT NULL,
         value TEXT NOT NULL,
         lock INTEGER NOT NULL,
         FOREIGN KEY (resource_id) REFERENCES resources (id),
         UNIQUE(resource_id,key)
        )"#;

    let point_index = "CREATE UNIQUE INDEX resource_point_index ON resources(parent,point_segment)";

    let transaction = conn.transaction()?;
    transaction.execute(labels, [])?;
    transaction.execute(tags, [])?;
    transaction.execute(resources, [])?;
    transaction.execute(properties, [])?;
    transaction.execute(point_index, [])?;
    transaction.commit()?;
    Ok(())
}

pub enum RegError{
    Dupe,
    Error(Error)
}

impl ToString for RegError {
    fn to_string(&self) -> String {
        match self {
            RegError::Dupe => {
                "Dupe".to_string()
            }
            RegError::Error(error) => {
                error.to_string()
            }
        }
    }
}
impl From<sqlx::Error> for RegError {
    fn from(e: sqlx::Error) -> Self {
        RegError::Error(e.into())
    }
}


impl From<tokio::sync::oneshot::error::RecvError> for RegError {
    fn from(e: tokio::sync::oneshot::error::RecvError) -> Self {
        RegError::Error(Error::from_internal( format!("{}", e.to_string())))
    }
}
impl From<Error> for RegError {
    fn from(e: Error) -> Self {
        RegError::Error(e)
    }
}

impl From<rusqlite::Error> for RegError {
    fn from(e: rusqlite::Error) -> Self {
        RegError::Error(e.into())
    }
}

impl <T> From<mpsc::error::SendError<T>> for RegError {
    fn from(e: mpsc::error::SendError<T>) -> Self {
        RegError::Error(e.into())
    }
}

impl From<&str> for RegError {
    fn from(e: &str) -> Self {
        RegError::Error(e.into())
    }
}

#[derive(Clone)]
pub struct Selector {
    conn: Arc<Mutex<Connection>>,
    skel: StarSkel,
    select: Select
}


impl Selector {
    pub fn from( mut self, select: Select ) -> Self {
        self.select = select;
        self
    }

    async fn select(mut self) -> Result<PrimitiveList,Error> {
        println!("REG SELECT:");

        #[async_recursion]
        async fn initial(mut selector: Selector) -> Result<PrimitiveList, Error> {
            let select = selector.select;
            let point = select.pattern.query_root();
            println!("REG SELECT: initial");
            if point != select.pattern.query_root() {
                //let fail = fail::Fail::Resource(fail::particle::Fail::Select(fail::particle::Select::WrongAddress {required:select.pattern.query_root(), found: point }));
                return Err("WrongAddress".into());
            }

            println!("REG SELECT: pre query");
            let point_kind_path = selector.skel.registry_api.query( point.clone(), Query::AddressKindPath ).await?.try_into()?;
            println!("REG SELECT: post query");

            let sub_select_hops = select.pattern.sub_select_hops();
            let sub_select = select.sub_select(point.clone(), sub_select_hops, point_kind_path);

            println!("REG SELECT: pre select()...");
            let select = sub_select.into();
            selector.select = select;
            let list = selector.select().await?;

            println!("REG SELECT: post select()... {}", list.len() );
            for l in &list.list {
                if let Primitive::Stub(stub) = l {
                    println!("-> FOUND: {}", stub.point.to_string());
                }
            }

            Ok(list)
        }

        async fn sub_select(mut selector: Selector) -> Result<PrimitiveList,Error> {
            println!("REG SELECT: sub_select");
            let sub_select :SubSelect = selector.select.clone().try_into()?;
            let mut params: Vec<String> = vec![];
            let mut where_clause = String::new();
            let mut index = 1;
            where_clause.push_str( "parent=?1" );
            params.push( sub_select.point.to_string() );

            if let Option::Some(hop) = sub_select.hops.first()
            {
                match &hop.segment_selector {
                    PointSegSelector::Exact(exact) => {
                        index = index+1;
                        where_clause.push_str( format!(" AND point_segment=?{}",index).as_str() );
                        match exact {
                            ExactSegment::Address(point) => {
                                params.push( point.to_string() );
                            }
                            ExactSegment::Version(version) => {
                                params.push( version.to_string() );
                            }
                        }
                    }
                    _ => {}
                }

                match &hop.tks.resource_type {
                    GenericKindSelector::Any => {},
                    GenericKindSelector::Exact(resource_type)=> {
                        index = index+1;
                        where_clause.push_str( format!(" AND resource_type=?{}",index).as_str() );
                        params.push( resource_type.to_string() );
                    },
                }

                match &hop.tks.kind {
                    KindSelector::Any => {},
                    KindSelector::Exact(kind)=> {
                        match &kind.kind {
                            None => {}
                            Some(sub) => {
                                index = index+1;
                                where_clause.push_str(format!(" AND kind=?{}", index).as_str());
                                params.push(sub.clone() );
                            }
                        }
                    }
                }

                match &hop.tks.specific {
                    ValuePattern::Any => {}
                    ValuePattern::None => {}
                    ValuePattern::Pattern(specific) => {
                        match &specific.vendor {
                            VendorSelector::Any => {}
                            VendorSelector::Exact(vendor) => {
                                index = index+1;
                                where_clause.push_str(format!(" AND vendor=?{}", index).as_str());
                                params.push(vendor.clone() );
                            }
                        }
                        match &specific.product{
                            ProductSelector::Any => {}
                            ProductSelector::Exact(product) => {
                                index = index+1;
                                where_clause.push_str(format!(" AND product=?{}", index).as_str());
                                params.push(product.clone() );
                            }
                        }
                        match &specific.variant{
                            VariantSelector::Any => {}
                            VariantSelector::Exact(variant) => {
                                index = index+1;
                                where_clause.push_str(format!(" AND variant=?{}", index).as_str());
                                params.push(variant.clone());
                            }
                        }
                    }
                }
            }

            let statement = format!(
                "SELECT DISTINCT {} FROM resources as r WHERE {}",
                RESOURCE_QUERY_FIELDS, where_clause
            );

            let mut stubs:Vec<Stub> = vec![];
            {
                let conn = selector.conn.lock().await;
                let mut statement = conn.prepare(statement.as_str())?;
                let mut rows = statement.query(params_from_iter(params.iter()))?;

                while let Option::Some(row) = rows.next()? {
                    stubs.push(RegistryComponent::process_resource_row_catch(row)?.into() );
                }
            }

            // next IF there are more hops, must coordinate with possible other stars...
            if !sub_select.hops.is_empty() {
                let mut hops = sub_select.hops.clone();
                hops.remove(0);
                let mut futures = vec![];
                for stub in &stubs {
                    if let Option::Some(last_segment) = stub.point.last_segment() {
                        let point = sub_select.point.push_segment(last_segment.clone());
                        let point_tks_path = sub_select.point_kind_path.push(PointKindSeg {
                            segment: last_segment,
                            kind: stub.kind.clone()
                        });
                        let sub_select = selector.select.clone().sub_select(point.clone(), hops.clone(), point_tks_path);
                        let select = sub_select.into();

                        let parent = point.parent().ok_or::<Error>("expecting point to have a parent".into())?;
                        let action = Method::Rc(Rc::Select(select));
                        let core = action.into();
                        let request = Request::new(core, point.clone(), parent.clone() );
                        futures.push(selector.skel.messaging_api.request(request));
                    }
                }

                println!("JOIN ALL pre");
                let futures =  join_all(futures).await;
                println!("JOIN ALL post");

                // the records matched the present hop (which we needed for deeper searches) however
                // they may not or may not match the ENTIRE pattern therefore they must be filtered
                stubs.retain(|stub| {
                    let point_tks_path = sub_select.point_kind_path.push(PointKindSeg {
                        segment: stub.point.last_segment().expect("expecting at least one segment" ),
                        kind: stub.kind.clone()
                    });
                    sub_select.pattern.matches(&point_tks_path)
                });

                // here we already know that the child sub_select should have filtered it's
                // not matching pointes so we can add all the results
                for future in futures {
                    let response = future;
                    if let Payload::List(more_stubs) =response.core.body {
                        let mut new_stubs = vec![];
                        for stub in more_stubs.list.into_iter() {
                            if let Primitive::Stub(stub) = stub {
                                new_stubs.push(stub);
                            }
                        }
                        stubs.append(  & mut new_stubs );
                    }
                }
            }

            let stubs: Vec<Stub> = stubs.into_iter().map(|record|record.into()).collect();
            let stubs = sub_select.into_payload.to_primitive(stubs)?;

            Ok(stubs)
        }

        match &self.select.kind {
            SelectKind::Initial => {
                initial(self.clone() ).await
            }
            SelectKind::SubSelect { .. } => {
                sub_select(self.clone()).await
            }
        }

    }
}
