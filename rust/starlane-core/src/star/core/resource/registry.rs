use std::convert::TryInto;
use std::fmt::{Debug, Formatter};
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use futures::future::join_all;
use futures::SinkExt;
use mesh_portal_serde::version::latest::generic::pattern::ExactSegment;
use crate::mesh::serde::payload::PrimitiveList;
use mesh_portal_serde::version::v0_0_1::pattern::SpecificPattern;
use mesh_portal_serde::version::v0_0_1::util::ValuePattern;
use rusqlite::{Connection, params_from_iter, Row, Transaction};
use  rusqlite::params;
use rusqlite::types::ValueRef;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use crate::error::Error;
use crate::fail::{Fail, StarlaneFailure};
use crate::frame::StarMessagePayload;
use crate::logger::LogInfo;
use crate::mesh::serde::fail;
use crate::mesh::serde::generic::resource::Archetype;
use crate::mesh::serde::id::{Address, AddressSegment, Kind, Specific};
use crate::mesh::serde::id::Version;
use crate::mesh::serde::pattern::{AddressKindPattern, AddressTksPath, AddressTksSegment, Hop};
use crate::mesh::serde::pattern::Pattern;
use crate::mesh::serde::pattern::SegmentPattern;
use crate::mesh::serde::payload::{Payload, Primitive};
use crate::mesh::serde::resource::command::common::{SetProperties, SetRegistry};
use crate::mesh::serde::resource::command::create::{AddressSegmentTemplate, Create, Strategy};
use crate::mesh::serde::resource::command::select::{Select, SubSelector};
use crate::mesh::serde::resource::{ResourceStub, Status};
use crate::message::{ProtoStarMessage, ProtoStarMessageTo, Reply, ReplyKind};
use crate::resource;
use crate::star::{ StarKey, StarSkel};
use crate::star::core::resource::host::HostCall;
use crate::star::shell::pledge::ResourceHostSelector;
use crate::util::{AsyncProcessor, AsyncRunner, Call};
use crate::resource::{ResourceRecord, AssignResourceStateSrc, Resource, ResourceAssign, AssignKind, ResourceLocation, ResourceType};
use crate::resources::message::{ProtoMessage, MessageFrom};
use mesh_portal_serde::version::v0_0_1::generic::resource::command::select::SelectionKind;
use crate::mesh::serde::entity::request::{ReqEntity, Rc};
use crate::mesh::serde::generic::payload::RcCommand;

static RESOURCE_QUERY_FIELDS: &str = "parent,address_segment,resource_type,kind,vendor,product,variant,version,version_pre,host,status";

#[derive(Clone)]
pub struct RegistryApi {
    pub tx: mpsc::Sender<RegistryCall>,
}

impl RegistryApi {
    pub fn new(tx: mpsc::Sender<RegistryCall>) -> Self {
        Self { tx }
    }

    pub async fn register( &self, registration: Registration ) -> Result<(),Fail> {
        let (tx,rx) = oneshot::channel();
        self.tx.send(RegistryCall::Register {registration, tx });
        rx.await?
    }

    pub async fn select( &self, select: Select, address: Address) -> Result<PrimitiveList,Fail> {
        let (tx,rx) = oneshot::channel();
        self.tx.send(RegistryCall::Select{select, address, tx });
        rx.await?
    }

    pub async fn sub_select( &self, selector: SubSelector ) -> Result<Vec<ResourceStub>,Fail> {
        let (tx,rx) = oneshot::channel();
        self.tx.send(RegistryCall::SubSelect{selector, tx });
        rx.await?
    }

    pub async fn address_tks_path_query(&self, address: Address ) -> Result<AddressTksPath,Fail> {
        let (tx,rx) = oneshot::channel();
        self.tx.send(RegistryCall::AddressTksPathQuery{address, tx });
        rx.await?
    }

    pub async fn update_status( &self, address: Address, status: Status ) -> Result<(),Fail> {
        let (tx,rx) = oneshot::channel();
        self.tx.send(RegistryCall::UpdateStatus{address, status, tx });
        rx.await?
    }

}

pub enum RegistryCall {
    Register{registration:Registration, tx: oneshot::Sender<Result<(),Fail>>},
    Select{select: Select, address: Address, tx: oneshot::Sender<Result<PrimitiveList,Fail>>},
    AddressTksPathQuery{ address: Address, tx: oneshot::Sender<Result<AddressTksPath,Fail>>},
    UpdateStatus{ address: Address, status: Status, tx: oneshot::Sender<Result<(),Fail>>},
}

impl Call for RegistryCall {}

pub struct RegistryComponent {
    skel: StarSkel,
    conn: Connection
}

impl RegistryComponent {
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<RegistryCall>) {
        let mut conn = Connection::open_in_memory().expect("expected to get sqlite database connection");
        setup(&mut conn);

        AsyncRunner::new(
            Box::new(Self {
                skel: skel.clone(),
                conn
            }),
            skel.registry_api.tx.clone(),
            rx,
        );
    }
}

#[async_trait]
impl AsyncProcessor<RegistryCall> for RegistryComponent {
    async fn process(&mut self, call: RegistryCall) {
        match call {
            RegistryCall::Register { registration, tx } => {
                self.register(registration,tx);
            }
            RegistryCall::Select { select, address, tx} => {
                self.select(select, address, tx);
            }
            RegistryCall::SubSelect { selector, tx } => {
                self.sub_select(selector, tx);
            }
            RegistryCall::AddressTksPathQuery { address, tx } => {
                self.address_tks_path_query(address,tx)
            }
            RegistryCall::UpdateStatus { address,status,tx } => {
                self.update_status(address,status,tx)
            }
        }
    }
}

impl RegistryComponent {

    fn update_status( &mut self, address: Address, status: Status, tx: oneshot::Sender<Result<(),Fail>>) {
        fn process( conn: &Connection, address:Address, status: Status ) -> Result<(),Fail> {
            let parent = address.parent().ok_or("resource must have a parent")?.to_string();
            let address_segment = address.last_segment().ok_or("resource must have a last segment")?.to_string();
            let status = status.to_string();
            let statement = "UPDATE resources SET status=?1 WHERE parent=?2 AND address_segment=?3";
            let mut statement = conn.prepare(statement.as_str())?;
            statement.execute(params!(status,parent,address_segment))?;
            trans.commit()?;
            Ok(())
        }
        tx.send(process(&self.conn, address,status));
    }

    fn address_tks_path_query( &mut self, address: Address, tx: oneshot::Sender<Result<AddressTksPath,Fail>>) {
        async fn query(skel: StarSkel, trans:Transaction, address: Address) -> Result<AddressTksPath, Fail> {

            if address.segments.len() == 0 {
                return Err(Fail::Starlane(StarlaneFailure::Error("cannot address_tks_path_query on Root".to_string())));
            }
            if address.segments.len() == 1 {
                let segment = AddressTksSegment {
                    address_segment: address.last_segment().expect("expected at least one segment"),
                    kind: Kind::Space
                };
                return Ok(AddressTksPath{
                    segments: vec![segment]
                });
            }

            let parent = address.parent().expect("expecting parent since we have already established the segments are >= 2");
            let address_segment = address.last_segment().expect("expecting a last_segment since we know segments are >= 2");
            let mut proto = ProtoStarMessage::new();
            proto.payload = StarMessagePayload::AddressTksPathQuery(parent.clone());
            proto.to = ProtoStarMessageTo::Resource(parent.clone());
            let reply = skel.messaging_api.exchange(proto, ReplyKind::AddressTksPath, format!("getting AddressTksPath for {}",parent.to_string()).as_str()  ).await?;
            if let Reply::AddressTksPath(parent_path) = reply {
                let statement = format!( "SELECT DISTINCT {} FROM resources as r WHERE parent=?1 AND address_segment=?2", RESOURCE_QUERY_FIELDS );
                let mut statement = trans.prepare(statement.as_str())?;
                let mut record = statement.query_row(params!(parent.to_string(),address_segment), RegistryComponent::process_resource_row_catch)?;
                let segment = AddressTksSegment{
                    address_segment: record.stub.address.last_segment().expect("expected at least one segment"),
                    kind: record.stub.kind
                };

                let path = parent_path.push(segment);
                Ok(path)
            } else {
                Err(Fail::Starlane(StarlaneFailure::Error("expected AddressTksPath reply".to_string())))
            }
        }

        match self.conn.transaction() {
            Ok(transaction) => {
                let skel = self.skel.clone();
                tokio::spawn( async move {
                    tx.send(query(skel, transaction, address).await);
                });
            }
            Err(err) => {
                tx.send( Err(Fail::Starlane(StarlaneFailure::Error("address_tks_path_query could not create database transaction".to_string()))))
            }
        }

    }

    fn register( &mut self, registration: Registration, tx: oneshot::Sender<Result<(),Fail>>) {
        fn register( registry: &mut RegistryComponent, registration: Registration ) -> Result<(),Fail> {
            let params = RegistryParams::from_registration(&registration)?;
            trans.execute("INSERT INTO resources (address_segment,resource_type,kind,vendor,product,variant,version,version_pre,parent,status) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,'Pending')", params![params.address_segment,params.resource_type,params.kind,params.vendor,params.product,params.variant,params.version,params.version_pre,params.parent])?;

            fn set_properties(prefix: &str, params: &RegistryParams, props: &SetProperties, trans: &Transaction ) -> Result<(),Fail> {
                for (key, payload) in props.iter() {
                    match payload {
                        Payload::Primitive(primitive) => {
                            match primitive {
                                Primitive::Text(text) => {
                                    trans.execute("INSERT INTO properties (resource_id,key,value) VALUES ((SELECT id FROM resources WHERE parent=?1 AND address_segment=?2),?3,?4)", params![params.parent,params.address_segment,key.to_string(),text.to_string()])?;
                                }
                                Primitive::Address(address) => {
                                    trans.execute("INSERT INTO properties (resource_id,key,value) VALUES ((SELECT id FROM resources WHERE parent=?1 AND address_segment=?2),?3,?4)", params![params.parent,params.address_segment,key.to_string(),address.to_string()])?;
                                }
                                found => {
                                    return Err(Fail::Fail(fail::Fail::Resource(fail::resource::Fail::Create(fail::resource::Create::InvalidProperty { expected: "Text|Address|PayloadMap".to_string(), found: found.primitive_type().to_string() }))));
                                }
                            }
                        }
                        Payload::Map(map) => {
                            let prefix = if prefix.len() == 0 {
                                key.clone()
                            } else {
                                format!("{}.{}",prefix,key)
                            };
                            set_properties(prefix.as_str(), params, map, &trans)?;
                        }
                        found => {
                            return Err(Fail::Fail(fail::Fail::Resource(fail::resource::Fail::Create(fail::resource::Create::InvalidProperty { expected: "Text|Address|PayloadMap".to_string(), found: found.payload_type().to_string() }))));
                        }
                    }
                }
                Ok(())
            }

            set_properties("", &params, &registration.properties, &trans )?;

            trans.commit()?;
            Ok(())
        }

        tx.send(register( self, registration ));
    }

    fn select(&mut self, select: Select, address: Address, tx: oneshot::Sender<Result<PrimitiveList,Fail>>) {
        async fn initial(registry: &mut RegistryComponent, select: Select,address: Address  ) -> Result<PrimitiveList, Fail> {
            if address != selector.pattern.query_root() {
                let fail = fail::Fail::Resource(fail::resource::Fail::Select(fail::resource::Select::WrongAddress {required:selector.pattern.query_root(), found: address }));
                return Err(Fail::Fail(fail));
            }
            let resource = registry.skel.resource_locator_api.locate(selector.pattern.query_root()).await?;
            if resource.location.host != registry.skel.info.key {
                let fail = fail::Fail::Resource(fail::resource::Fail::Select(fail::resource::Select::BadSelectRouting {required:resource.location.host.to_string(), found: registry.skel.info.key.to_string()}));
                return Err(Fail::Fail(fail));
            }

            let address_tks_path = registry.skel.registry_api.address_tks_path_query(address.clone()).await?;

            let sub_selector = selector.sub_select(address,selector.pattern.sub_select_hops(), address_tks_path );

            let list = registry.skel.registry_api.select(sub_selector.into(),address.clone()).await?;

            Ok(list)
        }

        async fn sub_select(skel: StarSkel, trans: Transaction,  selector: SubSelector) -> Result<PrimitiveList,Fail> {
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
                        }
                    }
                    _ => {}
                }

                match &hop.tks.resource_type {
                    Pattern::Any => {},
                    Pattern::Exact(resource_type)=> {
                        index = index+1;
                        where_clause.push_str( format!(" AND resource_type=?{}",index).as_str() );
                        params.push( resource_type.to_string() );
                    },
                }

                match &hop.tks.kind {
                    Pattern::Any => {},
                    Pattern::Exact(kind)=> {
                        match kind.sub_string() {
                            None => {}
                            Some(sub) => {
                                index = index+1;
                                where_clause.push_str(format!(" AND kind=?{}", index).as_str());
                                params.push(sub);
                            }
                        }
                    }
                }

                match &hop.tks.specific {
                    ValuePattern::Any => {}
                    ValuePattern::None => {}
                    ValuePattern::Pattern(specific) => {
                        match &specific.vendor {
                            Pattern::Any => {}
                            Pattern::Exact(vendor) => {
                                index = index+1;
                                where_clause.push_str(format!(" AND vendor=?{}", index).as_str());
                                params.push(vendor.clone() );
                            }
                        }
                        match &specific.product{
                            Pattern::Any => {}
                            Pattern::Exact(product) => {
                                index = index+1;
                                where_clause.push_str(format!(" AND product=?{}", index).as_str());
                                params.push(product.clone() );
                            }
                        }
                        match &specific.variant{
                            Pattern::Any => {}
                            Pattern::Exact(variant) => {
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

            let mut statement = transaction.prepare(statement.as_str())?;
            let mut rows = statement.query(params_from_iter(params.iter()))?;


            let mut records = vec![];
            while let Option::Some(row) = rows.next()? {
                records.push(RegistryComponent::process_resource_row_catch(row)?);
            }

            // next IF there are more hops, must coordinate with possible other stars...
            if !selector.hops.is_empty() {
                let mut hops = selector.hops.clone();
                hops.remove(0);
                let mut futures = vec![];
                for record in records {
                    if let Option::Some(last_segment) = record.stub.address.last_segment() {
                        let address = selector.address.push_segment(last_segment.clone());
                        let address_tks_path = selector.address_tks_path.push(AddressTksSegment{
                            address_segment: last_segment,
                            kind: record.stub.kind.clone()
                        });
                        let sub_selector = selector.sub_select(address.clone(),hops.clone(), address_tks_path);
                        let select = sub_selector.into();
                        let mut proto = ProtoMessage::new();
                        let parent = address.parent()?;
                        proto.to(address);
                        proto.from(MessageFrom::Address(parent));
                        proto.entity(ReqEntity::Rc(Rc::new(RcCommand::Select(Box::new(select)), Payload::Empty )));
                        let proto = proto.try_into()?;
                        futures.push(skel.messaging_api.exchange(proto, ReplyKind::Records, "sub-select" ));
                    }
                }

                let futures =  join_all(futures).await;

                // the records matched the present hop (which we needed for deeper searches) however
                // they may not or may not match the ENTIRE pattern therefore they must be filtered
                records.retain(|record| {
                    let address_tks_path = selector.address_tks_path.push(AddressTksSegment{
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
                tx.send( initial(self,select, address,));
            }
            SelectionKind::SubSelector { .. } => {
                match select.try_into() {
                    Ok(sub_selector) => {
                        match self.conn.transaction(){
                            Ok(trans) => {
                                let skel = self.skel.clone();
                                tokio::spawn(async move {
                                    tx.send(sub_select(skel.clone(), trans, sub_selector).await);
                                });
                            }
                            Err(err) => {
                                tx.send( Err(Fail::Starlane(StarlaneFailure::Error(error.to_string()))));
                            }
                        }
                    }
                    Err(error) => {
                        tx.send( Err(Fail::Starlane(StarlaneFailure::Error(error.to_string()))));
                    }
                }
            }
        }
    }





    fn process_resource_row_catch(row: &Row) -> Result<ResourceRecord, Error> {
        match Self::process_resource_row(row) {
            Ok(ok) => Ok(ok),
            Err(error) => {
                eprintln!("process_resource_rows: {}", error);
                Err(error)
            }
        }
    }


    //    static RESOURCE_QUERY_FIELDS: &str = "parent,address_segment,resource_type,kind,vendor,product,variant,version,version_pre,host,status";
    fn process_resource_row(row: &Row) -> Result<ResourceRecord, Error> {

        fn opt( row: &Row, index: usize ) -> Result<Option<String>,Error>
        {
            if let ValueRef::Null = row.get_ref(index)? {
                Ok(Option::None)
            } else {
                let specific: String = row.get(index)?;
                Ok(Option::Some(specific))
            }
        }

        let parent : String = row.get(1)?;
        let address_segment:String = row.get(2)?;
        let resource_type:String = row.get(3)?;
        let kind: Option<String> = opt(row,4)?;
        let vendor: Option<String> = opt(row,5)?;
        let product: Option<String> = opt(row,6)?;
        let variant: Option<String> = opt(row,7)?;
        let version: Option<String> = opt(row,8)?;
        let version_pre: Option<String> = opt(row,9)?;
        let host: Option<String> = opt(row,10)?;
        let status: String = row.get(11)?;

        let address = Address::from_str(parent.as_str())?;
        let address = address.push( address_segment )?;
        let resource_type = ResourceType::from_str(resource_type);
        let specific = if let Option::Some(vendor) = vendor {
            if let Option::Some(product) = product {
                if let Option::Some(variant) = variant {
                    if let Option::Some(version) = version {
                        let version = if let Option::Some(version_pre) = version_pre{
                            let version =  format!("{}-{}",version,version_pre);
                            Version::from_str(version)?
                        } else {
                            Version::from_str(version)?
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

        let kind = Kind::from( resource_type, kind, specific)?;
        let host = StarKey::from_str(host)?;
        let status = Status::from_str(status)?;

        let stub = ResourceStub {
            address,
            kind,
            properties: Default::default(), // not implemented yet...
            status
        };

        let record = ResourceRecord {
            stub: stub,
            location: ResourceLocation { host: host },
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
    version_pre: Option<String>,
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

        let version_pre = match &registration.kind.specific() {
            None => Option::None,
            Some(specific) =>  {
                let version = &specific.version;
                if version.is_prerelease() {
                    let mut pre = String::new();
                    for (i, x) in version.pre.iter().enumerate() {
                        if i != 0 {
                            result.push_str(".");
                        }
                        result.push_str(format!("{}", x).as_ref());
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
            version_pre
        })
    }
}





pub fn setup(conn: &mut Connection) -> Result<(), Error> {


    let resources = r#"CREATE TABLE IF NOT EXISTS resources (
         id INTEGER PRIMARY KEY AUTOINCREMENT,
         address_segment TEXT NOT NULL,
         parent TEXT NOT NULL,
         resource_type TEXT NOT NULL,
         kind TEXT NOT NULL,
         vendor TEXT,
         product TEXT,
         variant TEXT,
         version TEXT,
         version_variant TEXT,
         host TEXT,
         status TEXT NOT NULL,
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

    let address_index = "CREATE UNIQUE INDEX resource_address_index ON resources(address)";

    let transaction = conn.transaction()?;
    transaction.execute(labels, [])?;
    transaction.execute(tags, [])?;
    transaction.execute(resources, [])?;
    transaction.execute(properties, [])?;
    transaction.execute(address_index, [])?;
    transaction.commit()?;

    Ok(())
}

#[derive(Clone)]
pub struct ParentCore {
    pub skel: StarSkel,
    pub stub: ResourceStub,
    pub selector: ResourceHostSelector,
    pub child_registry: Arc<dyn ResourceRegistryBacking>,
}

impl Debug for ParentCore {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ParentCore")
            .field(&self.skel)
            .field(&self.stub)
            .finish()
    }
}

pub struct Parent {
    pub core: ParentCore,
}

impl Parent {
    #[instrument]
    async fn create_child(
        core: ParentCore,
        create: Create,
        tx: oneshot::Sender<Result<ResourceStub, Fail>>,
    ) {
        let parent = match create
            .parent
            .clone()
            .key_or("expected create.parent to already be a key")
        {
            Ok(key) => key,
            Err(error) => {
                tx.send(Err(Fail::from(error)));
                return;
            }
        };

        if let Ok(reservation) = core
            .child_registry
            .reserve(resource::Registration {
                parent: parent,
                archetype: create.archetype.clone(),
                info: create.registry_info.clone(),
            })
            .await
        {
            let rx =
                ResourceCreationChamber::new(core.stub.clone(), create.clone(), core.skel.clone())
                    .await;

            tokio::spawn(async move {
                match Self::process_action(core.clone(), create.clone(), reservation, rx).await {
                    Ok(resource) => {
                        tx.send(Ok(resource.into()));
                    }
                    Err(fail) => {
                        error!("Failed to create child: FAIL: {}", fail.to_string());
                        tx.send(Err(fail.into()));
                    }
                }
            });
        } else {
            error!("ERROR: reservation failed.");

            tx.send(Err("RESERVATION FAILED!".into()));
        }
    }

    async fn process_action(
        core: ParentCore,
        create: Create,
        reservation: RegistryReservation,
        rx: oneshot::Receiver<Result<ResourceAction<AssignResourceStateSrc>, Fail>>,
    ) -> Result<ResourceRecord, Error> {
        let action = rx.await??;

        match action {
            ResourceAction::Assign(assign) => {
                let host = core
                    .selector
                    .select(create.archetype.kind.resource_type())
                    .await?;
                let record = ResourceRecord::new(assign.stub.clone(), host.star_key());
                /// need to make this so that reservation is already committed with status set to Pending
                /// at this exact point status is updated to Assigning
                /// if Assigning succeeds then the host may put it through an Initializing status (if this AssignKind is Create vs. Move)
                /// once Status is Ready the resource can receive & process requests
                host.assign(assign.clone().try_into()?).await?;
                /*               let (commit_tx, _commit_rx) = oneshot::channel();
                                reservation.commit(record.clone(), commit_tx)?;
                                host.init(assign.stub.address).await?;

                 */
                Ok(record)
            }
            ResourceAction::Update(resource) => {
                /*
                // save resource state...
                let mut proto = ProtoMessage::new();

                let update = Update{
                    address: resource.address.clone(),
                    properties: PayloadMap::default()
                };

                proto.entity(ReqEntity::Rc(Rc::new(RcCommand::Update(Box::new(update)), resource.state_src() )));
                proto.to(resource.address.clone());
                proto.from(MessageFrom::Address(core.stub.address.clone()));

                let reply = core
                    .skel
                    .messaging_api
                    .exchange(
                        proto.try_into()?,
                        ReplyKind::Empty,
                        "updating the state of a record ",
                    )
                    .await;
                match reply {
                    Ok(reply) => {
                        let record = core
                            .skel
                            .resource_locator_api
                            .locate(resource.address )
                            .await;
                        record
                    }
                    Err(err) => Err(err.into()),
                }


                 */
                //               reservation.cancel();
            }
            ResourceAction::None => {
                // do nothing
            }
        }
    }

    /*
    if let Ok(Ok(assign)) = rx.await {
    if let Ok(mut host) = core.selector.select(create.archetype.kind.resource_type()).await
    {
    let record = ResourceRecord::new(assign.stub.clone(), host.star_key());
    match host.assign(assign).await
    {
    Ok(_) => {}
    Err(err) => {
    eprintln!("host assign failed.");
    return;
    }
    }
    let (commit_tx, commit_rx) = oneshot::channel();
    match reservation.commit(record.clone(), commit_tx) {
    Ok(_) => {
    if let Ok(Ok(_)) = commit_rx.await {
    tx.send(Ok(record));
    } else {
    elog( &core, &record.stub, "create_child()", "commit failed" );
    tx.send(Err("commit failed".into()));
    }
    }
    Err(err) => {
    elog( &core, &record.stub, "create_child()", format!("ERROR: commit failed '{}'",err.to_string()).as_str() );
    tx.send(Err("commit failed".into()));
    }
    }
    } else {
    elog( &core, &assign.stub, "create_child()", "ERROR: could not select a host" );
    tx.send(Err("could not select a host".into()));
    }
    }

     */

    /*
    async fn process_create(core: ChildResourceManagerCore, create: ResourceCreate ) -> Result<ResourceRecord,Fail>{



        if !create.archetype.kind.resource_type().parent().matches(Option::Some(&core.key.resource_type())) {
            return Err(Fail::WrongParentResourceType {
                expected: HashSet::from_iter(core.key.resource_type().parent().types()),
                received: Option::Some(create.parent.resource_type())
            });
        };

        create.validate()?;

        let reservation = core.registry.reserve(ResourceNamesReservationRequest{
            parent: create.parent.clone(),
            archetype: create.archetype.clone(),
            info: create.registry_info } ).await?;

        let key = match create.key {
            KeyCreationSrc::None => {
                Address::new(core.key.clone(), ResourceId::new(&create.archetype.kind.resource_type(), core.id_seq.next() ) )?
            }
            KeyCreationSrc::Key(key) => {
                if key.parent() != Option::Some(core.key.clone()){
                    return Err("parent keys do not match".into());
                }
                key
            }
        };

        let address = match create.address{
            AddressCreationSrc::None => {
                let address = format!("{}:{}", core.address.to_parts_string(), key.generate_address_tail()? );
                create.archetype.kind.resource_type().address_structure().from_str(address.as_str())?
            }
            AddressCreationSrc::Append(tail) => {
                create.archetype.kind.resource_type().append_address(core.address.clone(), tail )?
            }
            AddressCreationSrc::Space(space_name) => {
                if core.key.resource_type() != ResourceType::Nothing{
                    return Err(format!("Space creation can only be used at top level (Nothing) not by {}",core.key.resource_type().to_string()).into());
                }
                ResourceAddress::for_space(space_name.as_str())?
            }
        };

        let stub = ResourceStub {
            key: key,
            address: address.clone(),
            archetype: create.archetype.clone(),
            owner: None
        };


        let assign = ResourceAssign {
            stub: stub.clone(),
            state_src: create.src.clone(),
        };

        let mut host = core.selector.select(create.archetype.kind.resource_type() ).await?;
        host.assign(assign).await?;
        let record = ResourceRecord::new( stub, host.star_key() );
        let (tx,rx) = oneshot::channel();
        reservation.commit( record.clone(), tx )?;

        Ok(record)
    }

     */
}

#[async_trait]
impl ResourceManager for Parent {
    async fn create(
        &self,
        create: Create,
    ) -> oneshot::Receiver<Result<ResourceStub, Fail>> {
        let (tx, rx) = oneshot::channel();

        let core = self.core.clone();
        tokio::spawn(async move {
            Parent::create_child(core, create, tx).await;
        });
        rx
    }
}


pub struct ResourceCreationChamber {
    parent: ResourceStub,
    create: Create,
    skel: StarSkel,
    tx: oneshot::Sender<Result<ResourceAction<AssignResourceStateSrc>, Fail>>,
}

impl ResourceCreationChamber {
    pub async fn new(
        parent: ResourceStub,
        create: Create,
        skel: StarSkel,
    ) -> oneshot::Receiver<Result<ResourceAction<AssignResourceStateSrc>, Fail>> {
        let (tx, rx) = oneshot::channel();
        let chamber = ResourceCreationChamber {
            parent: parent,
            create: create,
            skel: skel,
            tx: tx,
        };
        chamber.run().await;
        rx
    }

    async fn run(self) {
        tokio::spawn(async move {
           async fn create( chamber: &ResourceCreationChamber) -> Result<ResourceAction<AssignResourceStateSrc>,Fail> {

               let address = match &chamber.create.address_template.child_segment_template {
                   AddressSegmentTemplate::Exact(segment) => {
                       chamber.create.address_template.parent.push(segment.clone())?
                   }
               };

               let record = chamber
                   .skel
                   .resource_locator_api
                   .locate(address.clone().into())
                   .await;

               match record {
                   Ok(record) => {
                       match chamber.create.strategy {
                           Strategy::Create => {
                               let fail = Fail::Fail(fail::Fail::Resource(fail::resource::Fail::Create(fail::resource::Create::AddressAlreadyInUse(address.to_string()))));
                               return Err(fail);
                           }
                           Strategy::Ensure => {
                               // we've ensured that it's here, now we can go home
                               return Ok(ResourceAction::None);
                           }
                           Strategy::CreateOrUpdate => {
                               if record.stub.archetype != chamber.create.archetype {
                                   let fail = Fail::Fail(fail::Fail::Resource(fail::resource::Fail::Create(fail::resource::Create::CannotUpdateArchetype)));
                                   return Err(fail);
                               }
                               match &chamber.create.state_src {
                                   AssignResourceStateSrc::Stateless => {
                                       // nothing left to do...
                                       return Ok(ResourceAction::None);
                                   }
                                   AssignResourceStateSrc::Direct(state) => {
                                       let resource = Resource::new(
                                           record.stub.address,
                                           record.stub.archetype,
                                           state.clone(),
                                       );
                                       return Ok(ResourceAction::Update(resource));
                                   }
                               }
                           }
                       }
                   }
                   Err(_) => {
                       // maybe this should be Option since using an error to signal not found
                       // might get confused with error for actual failure
                       let assign = ResourceAssign::new(AssignKind::Create, stub, chamber.create.state_src.clone() );
                       return Ok(ResourceAction::Assign(assign));
                   }
               }
           }

           let result = create(&self).await;
           self.tx.send(result);

        });
    }

    async fn finalize_create(
        &self,
        key: Address,
        address: Address,
    ) -> Result<ResourceAction<AssignResourceStateSrc>, Fail> {
        let stub = ResourceStub {
            address: address.clone(),
            archetype: self.create.archetype.clone(),
        };

        let assign = ResourceAssign {
            kind: AssignKind::Create,
            stub: stub,
            state_src: self.create.state_src.clone(),
        };
        Ok(ResourceAction::Assign(assign))
    }
}


