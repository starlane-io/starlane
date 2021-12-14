use crate::util::{Call, AsyncRunner, AsyncProcessor};
use crate::star::{StarSkel, StarKey};
use crate::star::core::resource::host::HostCall;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use rusqlite::{Connection, params_from_iter, Row, Transaction};
use crate::error::Error;
use crate::mesh::serde::id::{Address, Kind, Specific, AddressSegment};
use crate::mesh::serde::resource::command::common::{SetRegistry, SetProperties};
use crate::fail::{Fail, StarlaneFailure};
use crate::mesh::serde::payload::{Payload, Primitive};
use crate::mesh::serde::fail;
use crate::mesh::serde::resource::command::select::Select;
use crate::mesh::serde::resource::ResourceStub;
use crate::mesh::serde::pattern::{Hop, AddressKindPattern, AddressTksPath, AddressTksSegment};
use crate::mesh::serde::pattern::Pattern;
use crate::mesh::serde::pattern::SegmentPattern;
use crate::mesh::serde::pattern::ExactSegment;
use crate::mesh::serde::id::Version;
use mesh_portal_serde::version::latest::generic::pattern::ExactSegment;
use mesh_portal_serde::version::v0_0_1::util::ValuePattern;
use mesh_portal_serde::version::v0_0_1::pattern::SpecificPattern;
use crate::resource::{ResourceRecord, ResourceLocation};
use rusqlite::types::ValueRef;
use std::str::FromStr;
use crate::mesh::serde::generic::resource::Archetype;
use futures::SinkExt;
use mesh_portal_serde::version::v0_0_1::generic::payload::PrimitiveList;
use crate::message::{ProtoStarMessage, ProtoStarMessageTo, ReplyKind, Reply};
use crate::star::core::resource::registry::RegistryCall::SubSelect;
use crate::frame::StarMessagePayload;
use futures::future::join_all;

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

    pub async fn select( &self, select: Select, address: Address) -> Result<Payload,Fail> {
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

}

pub enum RegistryCall {
    Register{registration:Registration, tx: oneshot::Sender<Result<(),Fail>>},
    Select{select: Select, address: Address, tx: oneshot::Sender<Result<Payload,Fail>>},
    SubSelect { selector: SubSelector, tx: oneshot::Sender<Result<Vec<ResourceStub>,Fail>>},
    AddressTksPathQuery{ address: Address, tx: oneshot::Sender<Result<AddressTksPath,Fail>>},
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
            skel.core_registry_api.tx.clone(),
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
        }
    }
}

impl RegistryComponent {
    fn address_tks_path_query( &mut self, address: Address, tx: oneshot::Sender<Result<AddressTksPath,Fail>>) {
        async fn query(skel: StarSkel, trans:Transaction, address: Address) -> Result<AddressTksPath, Fail> {

            if address.segments.len() == 0 {
                return Err(Fail::Starlane(StarlaneFailure::Error("cannot address_tks_path_query on Root".to_string())));
            }
            if address.segments.len() == 1 {
                let segment = AddressTksSegment {
                    address_segment: address.last_segment().expect("expected at least one segment"),
                    tks: Kind::Space
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
                let mut record = statement.query_row(params!(parent.to_string(),address_segment), Self::process_resource_row_catch)?;
                let segment = AddressTksSegment{
                    address_segment: record.stub.address.last_segment().expect("expected at least one segment"),
                    tks: record.stub.kind
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

            fn properties( prefix: &str, properties: SetProperties, trans: &Transaction ) -> Result<(),Fail> {
                for (key, payload) in properties.iter() {
                    match payload {
                        Payload::Primitive(primitive) => {
                            match primitive {
                                Primitive::Text(text) => {
                                    trans.execute("INSERT INTO properties (address,key,value) VALUES (?1,?2,?3)", params![params.address,key.to_string(),text.to_string()])?;
                                }
                                Primitive::Address(address) => {
                                    trans.execute("INSERT INTO properties (address,key,value) VALUES (?1,?2,?3)", params![params.address,key.to_string(),address.to_string()])?;
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
                            properties( prefix.as_str(), map, &trans)?;
                        }
                        found => {
                            return Err(Fail::Fail(fail::Fail::Resource(fail::resource::Fail::Create(fail::resource::Create::InvalidProperty { expected: "Text|Address|PayloadMap".to_string(), found: found.payload_type().to_string() }))));
                        }
                    }
                }
                Ok(())
            }

            properties( "", registration.properties, &trans )?;

            trans.commit()?;
            Ok(())
        }

        tx.send(register( self, registration ));
    }

    fn select(&mut self, select: Select, address: Address, tx: oneshot::Sender<Result<Payload,Fail>>) {
        async fn sub_select(registry: &mut RegistryComponent, selector: Select ) -> Result<Payload, Fail> {
            if address != selector.pattern.query_root() {
                let fail = fail::Fail::Resource(fail::resource::Fail::Select(fail::resource::Select::WrongAddress {required:selector.pattern.query_root(), found: address }));
                return Err(Fail::Fail(fail));
            }
            let resource = registry.skel.resource_locator_api.locate(selector.pattern.query_root()).await?;
            if resource.location.host != registry.skel.info.key {
                let fail = fail::Fail::Resource(fail::resource::Fail::Select(fail::resource::Select::BadSelectRouting {required:resource.location.host.to_string(), found: registry.skel.info.key.to_string()}));
                return Err(Fail::Fail(fail));
            }

            let address_tks_path = registry.skel.core_registry_api.address_tks_path_query(address).await?;

            let sub_selector = SubSelector{
                pattern: selector.pattern.clone(),
                address,
                hops: selector.pattern.sub_select_hops(),
                address_tks_path
            };

            let stubs = registry.skel.core_registry_api.sub_select(sub_selector).await?;

            let rtn  = selector.into_payload.to_primitive(stubs)?;

            let rtn = Payload::List(rtn);

            Ok(rtn)
        }
    }



    fn sub_select(&mut self, selector: SubSelector, tx: oneshot::Sender<Result<Vec<ResourceStub>,Fail>>) {
        async fn sub_select(skel: StarSkel, transaction: Transaction,  selector: SubSelector) -> Result<Vec<ResourceStub>,Fail> {
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
                records.push(Self::process_resource_row_catch(row)?);
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
                            tks: record.stub.kind.clone()
                        });
                        let selector = SubSelector {
                            pattern: selector.pattern.clone(),
                            address,
                            hops: hops.clone(),
                            address_tks_path
                        };
                        let mut proto = ProtoStarMessage::new();
                        proto.payload = StarMessagePayload::SubSelect(selector);
                        proto.to = ProtoStarMessageTo::Star(record.location.host.clone());
                        futures.push(skel.messaging_api.exchange(proto, ReplyKind::Records, "sub-select" ));
                    }
                }
                let futures =  join_all(futures).await;

                // the records matched the present hop (which we needed for deeper searches) however
                // they may not or may not match the ENTIRE pattern therefore they must be filtered
                records.retain(|record| {
                    let address_tks_path = selector.address_tks_path.push(AddressTksSegment{
                        address_segment: record.stub.address.last_segment().expect("expecting at least one segment" ),
                        tks: record.stub.kind.clone()
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

            Ok(stubs)
        }
        match self.conn.transaction() {
            Ok(transaction) => {
                let skel = self.skel.clone();
                tokio::spawn( async move {
                    tx.send(sub_select(skel, transaction, selector).await);
                });
            }
            Err(err) => {
                tx.send( Err(Fail::Starlane(StarlaneFailure::Error("sub select could not create database transaction".to_string()))))
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




struct SubSelector {
    pub pattern: AddressKindPattern,
    pub address: Address,
    pub hops: Vec<Hop>,
    pub address_tks_path: AddressTksPath
}



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
         address TEXT NOT NULL,
         key TEXT NOT NULL,
         value TEXT NOT NULL,
         FOREIGN KEY (resource_id) REFERENCES resources (id),
         UNIQUE(address,key)
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


