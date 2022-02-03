use std::convert::TryInto;
use std::fmt::{Debug, Formatter};
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use futures::future::join_all;
use futures::SinkExt;
use mesh_portal_serde::version::latest::command::common::{SetProperties, SetRegistry};
use mesh_portal_serde::version::latest::entity::request::query::{Query, QueryResult};
use mesh_portal_serde::version::latest::entity::request::{Rc, RcCommand, ReqEntity};
use mesh_portal_serde::version::latest::entity::request::select::{Select, SelectionKind, SubSelector};
use mesh_portal_serde::version::latest::id::{Address, Specific, Version};
use mesh_portal_serde::version::latest::messaging::Request;
use mesh_portal_serde::version::latest::pattern::{AddressKindPath, AddressKindSegment, ExactSegment, KindPattern, ResourceTypePattern, SegmentPattern};
use mesh_portal_serde::version::latest::pattern::specific::{ProductPattern, VariantPattern, VendorPattern};
use mesh_portal_serde::version::latest::payload::{Payload, Primitive, PrimitiveList};
use mesh_portal_serde::version::latest::resource::{ResourceStub, Status};
use mesh_portal_serde::version::latest::util::ValuePattern;
use mesh_portal_versions::version::v0_0_1::command::common::PropertyMod;
use mesh_portal_versions::version::v0_0_1::id::AddressSegment;
use mesh_portal_versions::version::v0_0_1::util::unique_id;
use rusqlite::{Connection, params_from_iter, Row, Transaction};
use  rusqlite::params;
use rusqlite::types::ValueRef;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex};
use tokio::sync::oneshot;
use async_recursion::async_recursion;

use crate::error::Error;
use crate::fail::{Fail, StarlaneFailure};
use crate::frame::StarMessagePayload;
use crate::logger::LogInfo;

use crate::message::{ProtoStarMessage, ProtoStarMessageTo, Reply, ReplyKind};
use crate::resource;
use crate::star::{StarKey, StarSkel};
use crate::util::{AsyncProcessor, AsyncRunner, Call};
use crate::resource::{ResourceRecord, AssignResourceStateSrc, Resource, ResourceAssign, AssignKind, ResourceLocation, ResourceType, Kind};
use crate::resources::message::{ProtoRequest, MessageFrom};

use crate::security::permissions::Pattern;

static RESOURCE_QUERY_FIELDS: &str = "parent,address_segment,resource_type,kind,vendor,product,variant,version,version_variant,shell,status";

#[derive(Clone)]
pub struct RegistryApi {
    pub tx: mpsc::Sender<RegistryCall>,
}

impl RegistryApi {
    pub fn new(tx: mpsc::Sender<RegistryCall>) -> Self {
        Self { tx }
    }

    pub async fn register( &self, registration: Registration ) -> Result<ResourceStub,RegError> {
        let (tx,rx) = oneshot::channel();
        self.tx.send(RegistryCall::Register {registration, tx }).await?;

        let result = rx.await;
        result?
    }

    pub async fn assign( &self, address: Address, host: StarKey) -> Result<(),Error> {
        let (tx,rx) = oneshot::channel();
        self.tx.send(RegistryCall::Assign{address, host, tx }).await?;
        rx.await?
    }


    pub async fn select( &self, select: Select, address: Address) -> Result<PrimitiveList,Error> {
        let (tx,rx) = oneshot::channel();
        self.tx.send(RegistryCall::Select{select, address, tx }).await;
        rx.await?
    }

    pub async fn query(&self, address: Address, query: Query ) -> Result<QueryResult,Error> {
        let (tx,rx) = oneshot::channel();
        self.tx.send(RegistryCall::Query {address, query, tx }).await;
        rx.await?
    }

    pub async fn set_status(&self, address: Address, status: Status ) -> Result<(),Error> {
        let (tx,rx) = oneshot::channel();
        self.tx.send(RegistryCall::SetStatus{address, status, tx }).await;
        rx.await?
    }

    pub async fn set_location(&self, address: Address, location: ResourceLocation) -> Result<(),Error> {
        let (tx,rx) = oneshot::channel();
        self.tx.send(RegistryCall::SetLocation{address, location, tx }).await;
        rx.await?
    }

    pub async fn locate(&self, address: Address) -> Result<ResourceRecord,Error> {
        let (tx,rx) = oneshot::channel();
        self.tx.send(RegistryCall::Locate{address, tx }).await;
        rx.await?
    }

    pub async fn sequence(&self, address: Address) -> Result<u64,Error> {
        let (tx,rx) = oneshot::channel();
        self.tx.send(RegistryCall::Sequence{address, tx }).await;
        rx.await?
    }


}

pub enum RegistryCall {
    Assign{address:Address, host: StarKey, tx: oneshot::Sender<Result<(),Error>>},
    Register{registration:Registration, tx: oneshot::Sender<Result<ResourceStub,RegError>>},
    Select{select: Select, address: Address, tx: oneshot::Sender<Result<PrimitiveList,Error>>},
    Query { address: Address, query: Query, tx: oneshot::Sender<Result<QueryResult,Error>>},
    SetStatus { address: Address, status: Status, tx: oneshot::Sender<Result<(),Error>>},
    SetLocation{ address: Address, location: ResourceLocation, tx: oneshot::Sender<Result<(),Error>>},
    Locate{ address: Address, tx: oneshot::Sender<Result<ResourceRecord,Error>>},
    Sequence{ address: Address, tx: oneshot::Sender<Result<u64,Error>>},
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
            RegistryCall::Select { select, address, tx} => {
                self.select(select, address, tx).await;
            }
            RegistryCall::Query { address, query, tx } => {
                self.query(address, query, tx).await
            }
            RegistryCall::SetStatus{ address,status,tx } => {
                self.set_status(address, status, tx).await;
            }
            RegistryCall::SetLocation { address, location, tx } => {
                self.set_location(address, location, tx).await;
            }
            RegistryCall::Locate { address, tx } => {
                self.locate(address,tx ).await;
            }
            RegistryCall::Sequence { address, tx } => {
                self.sequence(address,tx).await;
            }
            RegistryCall::Assign { address, host, tx } => {
                self.assign(address,host, tx).await;
            }
        }
    }
}

impl RegistryComponent {

    async fn set_status(&mut self, address: Address, status: Status, tx: oneshot::Sender<Result<(),Error>>) {
        async fn process( conn: Arc<Mutex<Connection>>, address:Address, status: Status ) -> Result<(),Error> {
            let parent = address.parent().ok_or("resource must have a parent")?.to_string();
            let address_segment = address.last_segment().ok_or("resource must have a last segment")?.to_string();
            let status = status.to_string();
            let statement = "UPDATE resources SET status=?1 WHERE parent=?2 AND address_segment=?3";
            {
                let conn = conn.lock().await;
                let mut statement = conn.prepare(statement)?;
                statement.execute(params!(status,parent,address_segment))?;
            }
            Ok(())
        }
        tx.send(process(self.conn.clone(), address,status).await );
    }

    async fn set_location(&mut self, address: Address, location: ResourceLocation, tx: oneshot::Sender<Result<(),Error>>) {
        async fn process( conn: Arc<Mutex<Connection>>, address:Address, location: ResourceLocation) -> Result<(),Error> {
            let parent = address.parent().ok_or("resource must have a parent")?.to_string();
            let address_segment = address.last_segment().ok_or("resource must have a last segment")?.to_string();
            let host = location.ok_or()?.to_string();
            let statement = "UPDATE resources SET host=?1 WHERE parent=?2 AND address_segment=?3";
            {
                let conn = conn.lock().await;
                let mut statement = conn.prepare(statement)?;
                statement.execute(params!(host,parent,address_segment))?;
            }
            Ok(())
        }
        tx.send(process(self.conn.clone(), address,location).await );
    }

    async fn locate(&mut self, address: Address, tx: oneshot::Sender<Result<ResourceRecord,Error>>) {
println!("Locating {} star_kind {}", address.to_string(), self.skel.info.kind.to_string() );
        async fn process( conn: Arc<Mutex<Connection>>, address:Address) -> Result<ResourceRecord,Error> {
            let conn = conn.lock().await;
            let statement = format!( "SELECT DISTINCT {} FROM resources as r WHERE parent=?1 AND address_segment=?2", RESOURCE_QUERY_FIELDS );
            let mut statement = conn.prepare(statement.as_str())?;
            let parent = address.parent().ok_or("expected a parent")?;
            let address_segment = address.last_segment().ok_or("expected last address_segment")?.to_string();
            let record = statement.query_row(params!(parent.to_string(),address_segment), RegistryComponent::process_resource_row_catch)?;
            Ok(record)
        }
        tx.send(process(self.conn.clone(), address).await );
    }

    async fn sequence(&mut self, address: Address, tx: oneshot::Sender<Result<u64,Error>>) {
        async fn process(skel: StarSkel, conn:Arc<Mutex<Connection>>, address: Address) -> Result<u64, Error> {
            let conn = conn.lock().await;
            let parent = address.parent().ok_or("expecting parent since we have already established the segments are >= 2")?;
            let address_segment = address.last_segment().ok_or("expecting a last_segment since we know segments are >= 2")?;
            conn.execute("UPDATE resources SET sequence=sequence+1 WHERE parent=?1 AND address_segment=?2",params![parent.to_string(),address_segment.to_string()])?;
            Ok(conn.query_row( "SELECT DISTINCT sequence FROM resources WHERE parent=?1 AND address_segment=?2",params![parent.to_string(),address_segment.to_string()], RegistryComponent::process_sequence)?)
        }
        tx.send(process(self.skel.clone(), self.conn.clone(), address).await);
    }

    async fn assign(&mut self, address: Address, host: StarKey, tx: oneshot::Sender<Result<(),Error>>) {
        async fn process( conn:Arc<Mutex<Connection>>, address: Address, host: StarKey) -> Result<(), Error> {
            let conn = conn.lock().await;
            let parent = address.parent().ok_or("expecting parent since we have already established the segments are >= 2")?;
            let address_segment = address.last_segment().ok_or("expecting a last_segment since we know segments are >= 2")?;
            conn.execute("UPDATE resources SET shell=?1 WHERE parent=?2 AND address_segment=?3",params![host.to_string(),parent.to_string(),address_segment.to_string()])?;
            Ok(())
        }
        tx.send(process(self.conn.clone(), address, host).await);
    }


    async fn query(&mut self, address: Address, query: Query, tx: oneshot::Sender<Result<QueryResult,Error>>) {
        async fn process(skel: StarSkel, conn:Arc<Mutex<Connection>>, address: Address) -> Result<QueryResult, Error> {

            if address.segments.len() == 0 {
/*                let segment = AddressKindSegment {
                    address_segment: AddressSegment::Root,
                    kind: Kind::Root.into()
                };

 */
                return Ok(QueryResult::AddressKindPath(AddressKindPath::new(
                    address.route.clone(),
                    vec![]
                )));
            }
            else if address.segments.len() == 1 {
                let segment = AddressKindSegment {
                    address_segment: address.last_segment().expect("expected at least one segment"),
                    kind: Kind::Space.into()
                };
                return Ok(QueryResult::AddressKindPath(AddressKindPath::new(
                    address.route.clone(),
                     vec![segment]
                )));
            }

            let parent = address.parent().expect("expecting parent since we have already established the segments are >= 2");
            let address_segment = address.last_segment().expect("expecting a last_segment since we know segments are >= 2");
            let request= Request {
                id: unique_id(),
                from: skel.info.address.clone(),
                to: parent.clone(),
                entity: ReqEntity::Rc(Rc::new(Query::AddressKindPath.into() ))
            };
            let response = skel.messaging_api.exchange(request).await?;

            let parent_kind_path = response.entity.ok_or()?;
            let parent_kind_path: Primitive= parent_kind_path.try_into()?;
            let parent_kind_path: String= parent_kind_path.try_into()?;

            let parent_kind_path = AddressKindPath::from_str(parent_kind_path.as_str())?;


            let mut record = {
                let conn = conn.lock().await;
                let statement = format!("SELECT DISTINCT {} FROM resources as r WHERE parent=?1 AND address_segment=?2", RESOURCE_QUERY_FIELDS);
                let mut statement = conn.prepare(statement.as_str())?;
                statement.query_row(params!(parent.to_string(),address_segment.to_string()), RegistryComponent::process_resource_row_catch)?
            };
                let segment = AddressKindSegment {
                    address_segment: record.stub.address.last_segment().expect("expected at least one segment"),
                    kind: record.stub.kind
                };

                let path = parent_kind_path.push(segment);
                let result = QueryResult::AddressKindPath(path);

                Ok(result)
        }

                let skel = self.skel.clone();
                tx.send(process(skel, self.conn.clone(), address).await);
        }


    async fn register( &mut self, registration: Registration, tx: oneshot::Sender<Result<ResourceStub,RegError>>) {
        fn check<'a>( registration: &Registration,  trans:&Transaction<'a>, ) -> Result<(),RegError> {
            let params = RegistryParams::from_registration(registration)?;
            let count = trans.query_row("SELECT count(*) as count from resources WHERE parent=?1 AND address_segment=?2", params![params.parent, params.address_segment], RegistryComponent::count )?;
            if count > 0 {
                Err(RegError::Dupe)
            } else {
                Ok(())
            }
        }
        fn register<'a>( registration: Registration,  trans:&Transaction<'a>,) -> Result<(),Error> {
            let params = RegistryParams::from_registration(&registration)?;
            trans.execute("INSERT INTO resources (address_segment,resource_type,kind,vendor,product,variant,version,version_variant,parent,status) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,'Pending')", params![params.address_segment,params.resource_type,params.kind,params.vendor,params.product,params.variant,params.version,params.version_variant,params.parent])?;

            fn set_properties(prefix: &str, params: &RegistryParams, props: &SetProperties, trans: &Transaction ) -> Result<(),Error> {
                for property_mod in props.iter() {
                    match property_mod {
                        PropertyMod::Set{ name, value } => {
                            trans.execute("INSERT INTO properties (resource_id,key,value) VALUES ((SELECT id FROM resources WHERE parent=?1 AND address_segment=?2),?3,?4)", params![params.parent,params.address_segment,name.to_string(),value.to_string()])?;
                        }
                        PropertyMod::UnSet(name) => {
                            trans.execute("DELETE FROM properties (resource_id,key,value) WHERE parent=?1 AND address_segment=?2 AND name=?3)", params![params.parent,params.address_segment,name.to_string()])?;
                        }
                    }
                }
                Ok(())
            }

            set_properties("", &params, &registration.properties, &trans )?;

           Ok(())
        }

        let address = registration.address.clone();
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
                self.locate(address,otx).await;
                tx.send(match rx.await {
                    Ok(Ok(record)) => {
                        Ok(record.into())
                    }
                    Ok(Err(err)) => {
                        println!("~~~ could not locate record...");
                        Err("could not locate record".into())
                    }

                    Err(err) => {
                        Err("could not locate record".into())
                    }
                });
            }
            Err(err) => {
                tx.send(Err(RegError::Error(err)));
            }
        }
    }

    async fn select(&mut self, select: Select, address: Address, tx: oneshot::Sender<Result<PrimitiveList,Error>>) {
println!("REG SELECT:");
        #[async_recursion]
        async fn initial(reg: & mut RegistryComponent, skel: StarSkel, select: Select,address: Address  ) -> Result<PrimitiveList, Error> {
println!("REG SELECT: initial");
            if address != select.pattern.query_root() {
                //let fail = fail::Fail::Resource(fail::resource::Fail::Select(fail::resource::Select::WrongAddress {required:select.pattern.query_root(), found: address }));
                return Err("WrongAddress".into());
            }

println!("REG SELECT: pre query");
            let (tx,rx) = oneshot::channel();
            reg.query(address.clone(), Query::AddressKindPath,tx ).await;
            let address_kind_path = rx.await??.try_into()?;
println!("REG SELECT: post query");

            let sub_select_hops = select.pattern.sub_select_hops();
            let sub_selector = select.sub_select(address.clone(), sub_select_hops, address_kind_path);

println!("REG SELECT: pre select()...");
            let (tx,rx) = oneshot::channel();
            reg.select(sub_selector.into(),address,tx).await;
            let list = rx.await??;

println!("REG SELECT: post select()...");

            Ok(list)
        }

        async fn sub_select(skel: StarSkel, conn: Arc<Mutex<Connection>>,  selector: SubSelector) -> Result<PrimitiveList,Error> {
println!("REG SELECT: sub_select");
            let mut params: Vec<String> = vec![];
            let mut where_clause = String::new();
            let mut index = 1;
            where_clause.push_str( "parent=?1" );
            params.push( selector.address.to_string() );

            if let Option::Some(hop) = selector.hops.first()
            {
                match &hop.segment {
                    SegmentPattern::Exact(exact) => {
                        index = index+1;
                        where_clause.push_str( format!(" AND address_segment=?{}",index).as_str() );
                        match exact {
                            ExactSegment::Address(address) => {
                                params.push( address.to_string() );
                            }
                            ExactSegment::Version(version) => {
                                params.push( version.to_string() );
                            }
                        }
                    }
                    _ => {}
                }

                match &hop.tks.resource_type {
                    ResourceTypePattern::Any => {},
                    ResourceTypePattern::Exact(resource_type)=> {
                        index = index+1;
                        where_clause.push_str( format!(" AND resource_type=?{}",index).as_str() );
                        params.push( resource_type.to_string() );
                    },
                }

                match &hop.tks.kind {
                    KindPattern::Any => {},
                    KindPattern::Exact(kind)=> {
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
                            VendorPattern::Any => {}
                            VendorPattern::Exact(vendor) => {
                                index = index+1;
                                where_clause.push_str(format!(" AND vendor=?{}", index).as_str());
                                params.push(vendor.clone() );
                            }
                        }
                        match &specific.product{
                            ProductPattern::Any => {}
                            ProductPattern::Exact(product) => {
                                index = index+1;
                                where_clause.push_str(format!(" AND product=?{}", index).as_str());
                                params.push(product.clone() );
                            }
                        }
                        match &specific.variant{
                            VariantPattern::Any => {}
                            VariantPattern::Exact(variant) => {
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

            let mut records = vec![];
            {
                let conn = conn.lock().await;
                let mut statement = conn.prepare(statement.as_str())?;
                let mut rows = statement.query(params_from_iter(params.iter()))?;


                while let Option::Some(row) = rows.next()? {
                    records.push(RegistryComponent::process_resource_row_catch(row)?);
                }
            }

            // next IF there are more hops, must coordinate with possible other stars...
            if !selector.hops.is_empty() {
                let mut hops = selector.hops.clone();
                hops.remove(0);
                let mut futures = vec![];
                for record in &records {
                    if let Option::Some(last_segment) = record.stub.address.last_segment() {
                        let address = selector.address.push_segment(last_segment.clone());
                        let address_tks_path = selector.address_kind_path.push(AddressKindSegment {
                            address_segment: last_segment,
                            kind: record.stub.kind.clone()
                        });
                        let sub_selector = selector.sub_select(address.clone(),hops.clone(), address_tks_path);
                        let select = sub_selector.into();
                        let mut proto = ProtoRequest::new();

                        let parent = address.parent().ok_or::<Error>("expecting address to have a parent".into())?;
                        proto.to(address);
                        proto.from(MessageFrom::Address(parent));
                        proto.entity(ReqEntity::Rc(Rc::new(RcCommand::Select(select))));
                        let proto = proto.try_into()?;
                        futures.push(skel.messaging_api.star_exchange(proto, ReplyKind::Records, "sub-select" ));
                    }
                }

println!("JOIN ALL pre");
                let futures =  join_all(futures).await;
println!("JOIN ALL post");

                // the records matched the present hop (which we needed for deeper searches) however
                // they may not or may not match the ENTIRE pattern therefore they must be filtered
                records.retain(|record| {
                    let address_tks_path = selector.address_kind_path.push(AddressKindSegment {
                        address_segment: record.stub.address.last_segment().expect("expecting at least one segment" ),
                        kind: record.stub.kind.clone()
                    });
                    selector.pattern.matches(&address_tks_path)
                });

                // here we already know that the child sub_select should have filtered it's
                // not matching addresses so we can add all the results
                for future in futures {
                    let reply = future?;
                    if let Reply::Records(mut more_records) =reply {
                        records.append(  & mut more_records );
                    }
                }
            }

            let stubs: Vec<ResourceStub> = records.into_iter().map(|record|record.into()).collect();
            let stubs = selector.into_payload.to_primitive(stubs)?;

            Ok(stubs)
        }

            match &select.kind {
                SelectionKind::Initial => {
                    match initial(self, self.skel.clone(), select, address).await {
                        Ok(_) => {}
                        Err(error) => {
                            eprintln!("{}", error.to_string());
                        }
                    }
                }
                SelectionKind::SubSelector { .. } => {
                    match select.try_into() {
                        Ok(sub_selector) => {
                            let skel = self.skel.clone();
                            let conn = self.conn.clone();
                                tokio::spawn(async move {
                                    let result = sub_select(skel, conn, sub_selector).await;
                                    tx.send(result);
                                });
                        }
                        Err(error) => {
                            tx.send(Err(error.into()));
                        }
                    }
                }
            };

    }

    fn process_sequence(row: &Row) -> Result<u64, rusqlite::Error> {
        let sequence: u64= row.get(0)?;
        Ok(sequence)
    }

    fn process_resource_row_catch(row: &Row) -> Result<ResourceRecord, rusqlite::Error> {
        match Self::process_resource_row(row) {
            Ok(ok) => Ok(ok),
            Err(error) => {
                eprintln!("process_resource_rows: {}", error);
                Err(error.into())
            }
        }
    }
    fn count(row: &Row) -> Result<usize, rusqlite::Error> {
        let count: usize= row.get(0)?;
        Ok(count)
    }

    //    static RESOURCE_QUERY_FIELDS: &str = "parent,address_segment,resource_type,kind,vendor,product,variant,version,version_variant,shell,status";
    fn process_resource_row(row: &Row) -> Result<ResourceRecord, Error> {
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
            let address_segment: String = row.get(1)?;
            let resource_type: String = row.get(2)?;
            let kind: Option<String> = opt(row,3 )?;
            let vendor: Option<String> = opt(row, 4)?;
            let product: Option<String> = opt(row, 5)?;
            let variant: Option<String> = opt(row, 6)?;
            let version: Option<String> = opt(row, 7)?;
            let version_variant: Option<String> = opt(row, 8)?;
            let host: Option<String> = opt(row, 9)?;
            let status: String = row.get(10)?;

            let address = Address::from_str(parent.as_str())?;
            let address = address.push(address_segment)?;
            let resource_type = ResourceType::from_str(resource_type.as_str())?;
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
                    ResourceLocation::Host(StarKey::from_str(host.as_str())?)
                }
                None => {
                    ResourceLocation::Unassigned
                }
            };
            let status = Status::from_str(status.as_str())?;

            let stub = ResourceStub {
                address,
                kind: kind.into(),
                properties: Default::default(), // not implemented yet...
                status
            };

            let record = ResourceRecord {
                stub: stub,
                location: host,
            };

            Ok(record)

    }

}

/*
#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct SubSelector {
    pub pattern: AddressKindPattern,
    pub address: Address,
    pub hops: Vec<Hop>,
    pub address_tks_path: AddressTksPath
}

 */



#[derive(Clone)]
pub struct Registration {
    pub address: Address,
    pub kind: Kind,
    pub registry: SetRegistry,
    pub properties: SetProperties
}

struct RegistryParams {
    address_segment: String,
    resource_type: String,
    kind: Option<String>,
    vendor: Option<String>,
    product: Option<String>,
    variant: Option<String>,
    version: Option<String>,
    version_variant: Option<String>,
    parent: String,
}

impl RegistryParams {
    pub fn from_registration(registration: &Registration ) -> Result<Self, Error> {

        let address_segment = match registration.address.segments.last() {
            None => {"".to_string()}
            Some(segment) => {
                segment.to_string()
            }
        };
        let parent = match registration.address.parent()  {
            None => {"".to_string()}
            Some(parent) => {parent.to_string()}
        };

        let resource_type = registration.kind.resource_type().to_string();
        let kind = registration.kind.sub_string();

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
            address_segment,
            parent,
            resource_type,
            kind,
            vendor,
            product,
            variant,
            version,
            version_variant
        })
    }
}





fn setup(conn: &mut Connection) -> Result<(), Error> {


    let resources = r#"CREATE TABLE IF NOT EXISTS resources (
         id INTEGER PRIMARY KEY AUTOINCREMENT,
         address_segment TEXT NOT NULL,
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
         UNIQUE(parent,address_segment)
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

    /// note that a tag may reference an address NOT in this database
    /// therefore it does not have a FOREIGN KEY constraint
    let tags = r#"
       CREATE TABLE IF NOT EXISTS tags(
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          parent TEXT NOT NULL,
          tag TEXT NOT NULL,
          address TEXT NOT NULL,
          UNIQUE(tag)
        )"#;


    let properties = r#"CREATE TABLE IF NOT EXISTS properties (
         id INTEGER PRIMARY KEY AUTOINCREMENT,
	     resource_id INTEGER NOT NULL,
         key TEXT NOT NULL,
         value TEXT NOT NULL,
         FOREIGN KEY (resource_id) REFERENCES resources (id),
         UNIQUE(resource_id,key)
        )"#;

    let address_index = "CREATE UNIQUE INDEX resource_address_index ON resources(parent,address_segment)";

    let transaction = conn.transaction()?;
    transaction.execute(labels, [])?;
    transaction.execute(tags, [])?;
    transaction.execute(resources, [])?;
    transaction.execute(properties, [])?;
    transaction.execute(address_index, [])?;
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

impl From<tokio::sync::oneshot::error::RecvError> for RegError {
    fn from(e: tokio::sync::oneshot::error::RecvError) -> Self {
        RegError::Error(Error {
            error: format!("{}", e.to_string()),
        })
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